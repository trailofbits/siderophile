#![forbid(unsafe_code)]

use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    string::FromUtf8Error,
};

use quote::ToTokens;

use syn::{
    punctuated::Punctuated, visit, Attribute, Expr, GenericArgument, ImplItemMethod, ItemFn,
    ItemImpl, ItemMod, ItemTrait, PathArguments, TraitItemMethod,
};

/// A formatted list of Rust items that are unsafe
pub struct UnsafeItems(pub(crate) Vec<String>);

#[derive(Debug)]
pub enum ScanFileError {
    Io(io::Error, PathBuf),
    Utf8(FromUtf8Error, PathBuf),
    Syn(syn::Error, PathBuf),
}

impl Error for ScanFileError {}

/// Forward Display to Debug. See the crate root documentation.
impl fmt::Display for ScanFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

struct SiderophileSynVisitor {
    /// Where we log all the findings
    buf: Vec<String>,

    /// Keeps track of what the current module path is (this includes trait defs and impls)
    cur_mod_path: VecDeque<String>,

    /// Count unsafe usage inside tests
    include_tests: bool,
}

impl SiderophileSynVisitor {
    fn new(prefix: String, include_tests: bool) -> Self {
        let mut cur_mod_path = VecDeque::new();
        cur_mod_path.push_back(prefix);
        let buf = Vec::new();

        Self {
            buf,
            cur_mod_path,
            include_tests,
        }
    }
}

/// Will return true for #[cfg(test)] decodated modules.
///
/// This function is a somewhat of a hack and will probably misinterpret more
/// advanced cfg expressions. A better way to do this would be to let rustc emit
/// every single source file path and span within each source file and use that
/// as a general filter for included code.
/// TODO: Investigate if the needed information can be emitted by rustc today.
fn is_test_mod(i: &ItemMod) -> bool {
    use syn::Meta;
    i.attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .any(|m| match m {
            Meta::List(ml) => meta_list_is_cfg_test(&ml),
            _ => false,
        })
}

// MetaList {
//     ident: Ident(
//         cfg
//     ),
//     paren_token: Paren,
//     nested: [
//         Meta(
//             Word(
//                 Ident(
//                     test
//                 )
//             )
//         )
//     ]
// }
fn meta_list_is_cfg_test(ml: &syn::MetaList) -> bool {
    use syn::NestedMeta;
    if ml.path.get_ident().map(ToString::to_string) != Some("cfg".to_string()) {
        return false;
    }
    ml.nested.iter().any(|n| match n {
        NestedMeta::Meta(meta) => meta_is_word_test(meta),
        NestedMeta::Lit(_) => false,
    })
}

fn meta_is_word_test(m: &syn::Meta) -> bool {
    use syn::Meta;
    match m {
        Meta::Path(p) => p.get_ident().map(ToString::to_string) == Some("test".to_string()),
        Meta::List(_) | Meta::NameValue(_) => false,
    }
}

fn is_test_fn(i: &ItemFn) -> bool {
    i.attrs
        .iter()
        .flat_map(Attribute::parse_meta)
        .any(|m| meta_is_word_test(&m))
}

impl<'ast> visit::Visit<'ast> for SiderophileSynVisitor {
    fn visit_file(&mut self, i: &'ast syn::File) {
        syn::visit::visit_file(self, i);
    }

    /// Free-standing functions
    fn visit_item_fn(&mut self, i: &ItemFn) {
        // Exclude #[test] functions if not explicitly allowed
        if !self.include_tests && is_test_fn(i) {
            return;
        }

        self.cur_mod_path.push_back(i.sig.ident.to_string());

        // See if this function is marked unsafe
        if i.sig.unsafety.is_some() {
            let pp = fmt_mod_path(&self.cur_mod_path);
            self.buf.push(pp);
        }

        trace!("entering function {:?}", i.sig.ident);
        visit::visit_item_fn(self, i);

        self.cur_mod_path.pop_back();
    }

    fn visit_expr(&mut self, i: &Expr) {
        match i {
            Expr::Unsafe(i) => {
                let pp = fmt_mod_path(&self.cur_mod_path);
                self.buf.push(pp);
                visit::visit_expr_unsafe(self, i);
            }
            Expr::Closure(expr_closure) => {
                self.cur_mod_path.push_back("{{closure}}".to_string());
                visit::visit_expr_closure(self, expr_closure);
                self.cur_mod_path.pop_back();
            }
            Expr::Path(_) | Expr::Lit(_) => {
                // Do not count. The expression `f(x)` should count as one
                // expression, not three.
            }
            other => {
                visit::visit_expr(self, other);
            }
        }
    }

    fn visit_item_mod(&mut self, i: &ItemMod) {
        if !self.include_tests && is_test_mod(i) {
            return;
        }

        self.cur_mod_path.push_back(i.ident.to_string());
        visit::visit_item_mod(self, i);
        self.cur_mod_path.pop_back();
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        // unsafe trait impl's
        if let syn::Type::Path(ref for_path) = &*i.self_ty {
            let for_path = fmt_syn_path(for_path.path.clone());
            if let Some((_, ref trait_path, _)) = i.trait_ {
                let trait_path = fmt_syn_path(trait_path.clone());
                // Save the old path. We heavily modify the path for trait impls
                let old_cur_mod_path = self.cur_mod_path.clone();

                // We want a trait impl to look like
                // `<parking_lot_core::util::Option<T> as UncheckedOptionExt<T>>::unchecked_unwrap`
                self.cur_mod_path.push_back(for_path);
                let fmt_cur_mod_path = fmt_mod_path(&self.cur_mod_path);
                let full_impl_path = format!("<{} as {}>", fmt_cur_mod_path, trait_path);

                trace!("entering trait impl {}", trait_path);
                // The new path is just one component long, the whole thing in angled brackets
                self.cur_mod_path.clear();
                self.cur_mod_path.push_back(full_impl_path);

                // Recurse
                visit::visit_item_impl(self, i);

                // Restore the old path
                self.cur_mod_path = old_cur_mod_path;
                trace!("exiting trait impl {}", trait_path);
            } else {
                // Regular impls look like `parking_lot::raw_mutex::RawMutex::unlock_slow`
                trace!("entering impl {}", for_path);
                self.cur_mod_path.push_back(for_path.clone());

                visit::visit_item_impl(self, i);

                self.cur_mod_path.pop_back();
                trace!("exiting impl {}", for_path);
            }
        } else {
            // I don't know what this case represents
            visit::visit_item_impl(self, i);
        }
    }

    fn visit_item_trait(&mut self, i: &ItemTrait) {
        // Unsafe traits
        self.cur_mod_path.push_back(i.ident.to_string());
        visit::visit_item_trait(self, i);
        self.cur_mod_path.pop_back();
    }

    fn visit_trait_item_method(&mut self, i: &TraitItemMethod) {
        // Unsafe default-implemented trait methods
        self.cur_mod_path.push_back(i.sig.ident.to_string());
        visit::visit_trait_item_method(self, i);
        self.cur_mod_path.pop_back();
    }

    fn visit_impl_item_method(&mut self, i: &ImplItemMethod) {
        self.cur_mod_path.push_back(i.sig.ident.to_string());

        // See if this method is unsafe
        if i.sig.unsafety.is_some() {
            let pp = fmt_mod_path(&self.cur_mod_path);
            self.buf.push(pp);
        }

        trace!("entering method {:?}", i.sig.ident);
        visit::visit_impl_item_method(self, i);

        self.cur_mod_path.pop_back();
    }
}

// LLVM callgraphs don't have lifetimes, so neither do we. This removes the 'a in things like
// <lock_api::mutex::MutexGuard<'a,R,T> as DerefMut>::deref_mut
fn without_lifetimes(mut path: syn::Path) -> syn::Path {
    for seg in path.segments.iter_mut() {
        if let PathArguments::AngleBracketed(ref mut generic_args) = seg.arguments {
            // First remove all the lifetime arguments from this path
            let non_lifetime_args = generic_args
                .args
                .iter()
                .filter(|a| !matches!(a, GenericArgument::Lifetime(_)));

            // Now go into every type in the generic arguments and remove their lifetimes too. This
            // handles examples like <http::header::name::HeaderName as From<HdrName<'a>>>::from
            let stripped_args = non_lifetime_args.cloned().map(|a| {
                if let GenericArgument::Type(syn::Type::Path(mut ty_path)) = a {
                    // Recurse into the path in the type parameter of the given path
                    let stripped_path = without_lifetimes(ty_path.path);

                    ty_path.path = stripped_path;
                    GenericArgument::Type(syn::Type::Path(ty_path))
                } else {
                    a
                }
            });

            generic_args.args = stripped_args.collect::<Punctuated<_, _>>();

            // Check if the new arglist is empty. If it is, remove the arglist, otherwise we get
            // things like http::header::name::HdrName<>
            if generic_args.args.is_empty() {
                seg.arguments = PathArguments::None;
            }
        }
    }

    path
}

// Formats a Rust path represented by a syn::Path object
fn fmt_syn_path(path: syn::Path) -> String {
    let stripped_path = without_lifetimes(path);
    let token_trees = stripped_path.into_token_stream().into_iter();
    let fmt_components: Vec<String> = token_trees.map(|t| format!("{}", t)).collect();

    fmt_components.join("")
}

// Formats a module path represented by an ordered list of path components
fn fmt_mod_path(mod_path: &VecDeque<String>) -> String {
    let submods = mod_path.iter().cloned().collect::<Vec<String>>();
    submods.as_slice().join("::")
}

/// Scan a single file for `unsafe` usage.
pub fn find_unsafe_in_file(
    crate_name: &str,
    file_to_scan: &Path,
    include_tests: bool,
) -> Result<UnsafeItems, ScanFileError> {
    use syn::visit::Visit;
    trace!("in crate {}", crate_name);
    trace!("in file {:?}", file_to_scan);
    let src = std::ffi::OsString::from("src");
    let src_cpt = std::path::Component::Normal(&src);

    // Get the module path of the file we're in right now
    let prefix_module_path = if file_to_scan.components().any(|c| c == src_cpt) {
        let mut mods: Vec<String> = file_to_scan
            .components()
            .rev()
            .take_while(|c| c != &src_cpt)
            .map(|c| c.as_os_str().to_os_string().into_string().unwrap())
            .map(|c| c.replace("-", "_"))
            .filter(|c| c != "lib.rs" && c != "mod.rs")
            .map(|mut c| {
                if let Some(i) = c.find('.') {
                    c.truncate(i);
                }
                c
            })
            .collect();
        mods.reverse();
        mods.join("::")
    } else {
        String::new()
    };

    // This looks like `parking_lot_core::thread_parker::unix`
    let full_prefix = if prefix_module_path.is_empty() {
        crate_name.to_string()
    } else {
        [crate_name, &prefix_module_path].join("::")
    };

    let mut in_file =
        File::open(file_to_scan).map_err(|e| ScanFileError::Io(e, file_to_scan.to_path_buf()))?;
    let mut src = vec![];
    in_file
        .read_to_end(&mut src)
        .map_err(|e| ScanFileError::Io(e, file_to_scan.to_path_buf()))?;
    let src =
        String::from_utf8(src).map_err(|e| ScanFileError::Utf8(e, file_to_scan.to_path_buf()))?;
    let syntax =
        syn::parse_file(&src).map_err(|e| ScanFileError::Syn(e, file_to_scan.to_path_buf()))?;

    let mut vis = SiderophileSynVisitor::new(full_prefix, include_tests);
    vis.visit_file(&syntax);

    Ok(UnsafeItems(vis.buf))
}
