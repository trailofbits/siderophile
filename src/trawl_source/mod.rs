mod ast_walker;

use anyhow::{anyhow, Context};
use cargo::{
    core::{
        compiler::{CompileMode, Executor, Unit},
        manifest::TargetKind,
        package::PackageSet,
        Package, PackageId, Target, Workspace,
    },
    ops::CompileOptions,
};
use cargo_util::{paths, CargoResult, ProcessBuilder};
use std::{
    collections::{HashMap, HashSet},
    env::set_var,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use walkdir::{self, WalkDir};

#[derive(Debug)]
pub enum RsResolveError {
    Walkdir(walkdir::Error),

    /// Like io::Error but with the related path.
    Io(io::Error, PathBuf),

    /// Would like cargo::Error here, but it's private, why?
    /// This is still way better than a panic though.
    Cargo(String),

    /// This should not happen unless incorrect assumptions have been made in
    /// `siderophile` about how the cargo API works.
    ArcUnwrap(),

    /// Failed to get the inner context out of the mutex.
    InnerContextMutex(String),

    /// Failed to parse a .dep file.
    DepParse(String, PathBuf),
}

impl Error for RsResolveError {}

/// Forward Display to Debug.
impl fmt::Display for RsResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl From<PoisonError<CustomExecutorInnerContext>> for RsResolveError {
    fn from(e: PoisonError<CustomExecutorInnerContext>) -> Self {
        Self::InnerContextMutex(e.to_string())
    }
}

fn is_file_with_ext(entry: &walkdir::DirEntry, file_ext: &str) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }
    let p = entry.path();
    let ext = match p.extension() {
        Some(e) => e,
        None => return false,
    };
    // to_string_lossy is ok since we only want to match against an ASCII
    // compatible extension and we do not keep the possibly lossy result
    // around.
    ext.to_string_lossy() == file_ext
}

// TODO: Make a wrapper type for canonical paths and hide all mutable access.

/// Provides information needed to scan for crate root
/// `#![forbid(unsafe_code)]`.
/// The wrapped `PathBuf`s are canonicalized.
enum RsFile {
    /// Library entry point source file, usually src/lib.rs
    LibRoot(PathBuf),

    /// Executable entry point source file, usually src/main.rs
    BinRoot(PathBuf),

    /// Not sure if this is relevant but let's be conservative for now.
    CustomBuildRoot(PathBuf),

    /// All other .rs files.
    Other(PathBuf),
}

impl RsFile {
    const fn as_path_buf(&self) -> &PathBuf {
        match self {
            RsFile::LibRoot(ref pb)
            | RsFile::BinRoot(ref pb)
            | RsFile::CustomBuildRoot(ref pb)
            | RsFile::Other(ref pb) => pb,
        }
    }
}

#[allow(clippy::expect_used)]
pub fn find_rs_files_in_dir(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let walker = WalkDir::new(dir).into_iter();
    walker.filter_map(|entry| {
        let entry = entry.expect("walkdir error."); // TODO: Return result.
        if !is_file_with_ext(&entry, "rs") {
            return None;
        }
        Some(
            entry
                .path()
                .canonicalize()
                .expect("Error converting to canonical path"),
        ) // TODO: Return result.
    })
}

#[allow(clippy::expect_used, clippy::unwrap_used)]
fn find_rs_files_in_package(pack: &Package) -> Vec<RsFile> {
    // Find all build target entry point source files.
    let mut canon_targets = HashMap::new();
    for t in pack.targets() {
        let path = match t.src_path().path() {
            Some(p) => p,
            None => continue,
        };
        if !path.exists() {
            // A package published to crates.io is not required to include
            // everything. We have to skip this build target.
            continue;
        }
        let canon = path
            .canonicalize() // will Err on non-existing paths.
            .expect("canonicalize for build target path failed."); // FIXME
        let targets = canon_targets.entry(canon).or_insert_with(Vec::new);
        targets.push(t);
    }
    let mut out = Vec::new();
    for p in find_rs_files_in_dir(pack.root()) {
        if !canon_targets.contains_key(&p) {
            out.push(RsFile::Other(p));
        }
    }
    for (k, v) in canon_targets {
        for target in v {
            out.push(into_rs_code_file(target.kind(), k.clone()));
        }
    }
    out
}

const fn into_rs_code_file(kind: &TargetKind, path: PathBuf) -> RsFile {
    match kind {
        TargetKind::Lib(_) => RsFile::LibRoot(path),
        TargetKind::Bin => RsFile::BinRoot(path),
        TargetKind::Test
        | TargetKind::Bench
        | TargetKind::ExampleLib(_)
        | TargetKind::ExampleBin => RsFile::Other(path),
        TargetKind::CustomBuild => RsFile::CustomBuildRoot(path),
    }
}

fn find_rs_files_in_packages<'a>(
    packs: &'a [&Package],
) -> impl Iterator<Item = (PackageId, RsFile)> + 'a {
    packs.iter().flat_map(|pack| {
        find_rs_files_in_package(pack)
            .into_iter()
            .map(move |path| (pack.package_id(), path))
    })
}

/// This is mostly `PackageSet::get_many`. The only difference is that we don't panic when
/// downloads fail
#[allow(clippy::unwrap_used)]
fn get_many<'a>(
    packs: &'a PackageSet,
    ids: impl IntoIterator<Item = PackageId>,
) -> Vec<&'a Package> {
    let mut pkgs = Vec::new();
    let mut downloads = packs.enable_download().unwrap();
    for id in ids {
        match downloads.start(id) {
            // This might not return `Some` right away. It's still downloading.
            Ok(pkg_opt) => pkgs.extend(pkg_opt),
            Err(e) => warn!("Could not begin downloading {:?}, {:?}", id, e),
        }
    }
    while downloads.remaining() > 0 {
        // Packages whose `.start()` returned an `Ok(None)` earlier will return now
        match downloads.wait() {
            Ok(pkg) => pkgs.push(pkg),
            Err(e) => warn!("Failed to download package, {:?}", e),
        }
    }
    pkgs
}

/// Finds and outputs all unsafe things to the given file
#[allow(clippy::panic)]
pub fn find_unsafe_in_packages(
    packs: &PackageSet,
    mut rs_files_used: HashMap<PathBuf, u32>,
    allow_partial_results: bool,
    include_tests: bool,
) -> (HashMap<PathBuf, u32>, Vec<String>) {
    let packs = get_many(packs, packs.package_ids());
    let pack_code_files = find_rs_files_in_packages(&packs);
    let mut tainted_things = vec![];
    for (pack_id, rs_code_file) in pack_code_files {
        let p = rs_code_file.as_path_buf();

        // This .rs file path was found by intercepting rustc arguments or by parsing the .d files
        // produced by rustc. Here we increase the counter for this path to mark that this file has
        // been scanned. Warnings will be printed for .rs files in this collection with a count of
        // 0 (has not been scanned). If this happens, it could indicate a logic error or some
        // incorrect assumption in siderophile.
        if let Some(c) = rs_files_used.get_mut(p) {
            *c += 1;
        }

        let crate_name = pack_id.name().as_str().replace("-", "_");
        match ast_walker::find_unsafe_in_file(&crate_name, p, include_tests) {
            Ok(ast_walker::UnsafeItems(items)) => {
                // Output unsafe items as we go
                tainted_things.extend(items);
            }
            Err(e) => {
                if allow_partial_results {
                    warn!(
                        "Failed to parse file: {}, {:?}. Continuing...",
                        p.display(),
                        e
                    );
                } else {
                    panic!("Failed to parse file: {}, {:?} ", p.display(), e);
                }
            }
        }
    }

    (rs_files_used, tainted_things)
}

/// Trigger a `cargo build` and listen to the cargo/rustc communication to
/// figure out which source files were used by the build.
pub fn resolve_rs_file_deps(
    copt: &CompileOptions,
    ws: &Workspace,
) -> anyhow::Result<HashMap<PathBuf, u32>> {
    let config = ws.config();
    set_var("RUSTFLAGS", crate::callgraph_gen::RUSTFLAGS);
    let inner_arc = Arc::new(Mutex::new(CustomExecutorInnerContext::default()));
    {
        let cust_exec = CustomExecutor {
            cwd: config.cwd().to_path_buf(),
            inner_ctx: inner_arc.clone(),
        };
        let exec: Arc<dyn Executor> = Arc::new(cust_exec);
        cargo::ops::compile_with_exec(ws, copt, &exec)
            .map_err(|e| RsResolveError::Cargo(e.to_string()))
            .with_context(|| "`compile_with_exec` failed")?;
    }
    let ws_root = ws.root().to_path_buf();
    let inner_mutex = Arc::try_unwrap(inner_arc).map_err(|_| RsResolveError::ArcUnwrap())?;
    let (rs_files, out_dir_args) = {
        let ctx = inner_mutex.into_inner()?;
        (ctx.rs_file_args, ctx.out_dir_args)
    };
    let mut hm = HashMap::<PathBuf, u32>::new();
    for out_dir in out_dir_args {
        // TODO: Figure out if the `.d` dep files are used by one or more rustc
        // calls. It could be useful to know which `.d` dep files belong to
        // which rustc call. That would allow associating each `.rs` file found
        // in each dep file with a PackageId.
        for ent in WalkDir::new(&out_dir) {
            let ent = ent.map_err(RsResolveError::Walkdir)?;
            if !is_file_with_ext(&ent, "d") {
                continue;
            }
            let deps = parse_rustc_dep_info(ent.path())
                .map_err(|e| RsResolveError::DepParse(e.to_string(), ent.path().to_path_buf()))?;
            let canon_paths = deps
                .into_iter()
                .flat_map(|t| t.1)
                .map(PathBuf::from)
                .map(|pb| ws_root.join(pb))
                .map(|pb| pb.canonicalize().map_err(|e| RsResolveError::Io(e, pb)));
            for p in canon_paths {
                hm.insert(p?, 0);
            }
        }
    }
    for pb in rs_files {
        // rs_files must already be canonicalized
        hm.insert(pb, 0);
    }
    Ok(hm)
}

/// Copy-pasted (almost) from the private module `cargo::core::compiler::fingerprint`.
///
/// TODO: Make a PR to the cargo project to expose this function or to expose
/// the dependency data in some other way.
fn parse_rustc_dep_info(rustc_dep_info: &Path) -> CargoResult<Vec<(String, Vec<String>)>> {
    let contents = paths::read(rustc_dep_info)?;
    contents
        .lines()
        .filter_map(|l| l.find(": ").map(|i| (l, i)))
        .map(|(line, pos)| {
            let target = &line[..pos];
            let mut deps = line[pos + 2..].split_whitespace();
            let mut ret = Vec::new();
            while let Some(s) = deps.next() {
                let mut file = s.to_string();
                while file.ends_with('\\') {
                    file.pop();
                    file.push(' ');
                    //file.push_str(deps.next().ok_or_else(|| {
                    //internal("malformed dep-info format, trailing \\".to_string())
                    //})?);
                    file.push_str(
                        deps.next()
                            .ok_or_else(|| anyhow!("malformed dep-info format, trailing \\"))?,
                    );
                }
                ret.push(file);
            }
            Ok((target.to_string(), ret))
        })
        .collect()
}

#[derive(Debug, Default)]
struct CustomExecutorInnerContext {
    /// Stores all lib.rs, main.rs etc. passed to rustc during the build.
    rs_file_args: HashSet<PathBuf>,

    /// Investigate if this needs to be intercepted like this or if it can be
    /// looked up in a nicer way.
    out_dir_args: HashSet<PathBuf>,
}

use std::sync::PoisonError;

/// A cargo Executor to intercept all build tasks and store all ".rs" file
/// paths for later scanning.
///
/// TODO: This is the place(?) to make rustc perform macro expansion to allow
/// scanning of the the expanded code. (incl. code generated by build.rs).
/// Seems to require nightly rust.
#[derive(Debug)]
struct CustomExecutor {
    /// Current work dir
    cwd: PathBuf,

    /// Needed since multiple rustc calls can be in flight at the same time.
    inner_ctx: Arc<Mutex<CustomExecutorInnerContext>>,
}

use std::error::Error;
use std::fmt;

#[derive(Debug)]
enum CustomExecutorError {
    OutDirKeyMissing(String),
    OutDirValueMissing(String),
    InnerContextMutex(String),
    Io(io::Error, PathBuf),
}

impl Error for CustomExecutorError {}

/// Forward Display to Debug. See the crate root documentation.
impl fmt::Display for CustomExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Executor for CustomExecutor {
    /// In case of an `Err`, Cargo will not continue with the build process for
    /// this package.
    /// TODO: add doing things with `on_stdout_line` and `on_stderr_line`
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn exec(
        &self,
        command: &ProcessBuilder,
        _id: PackageId,
        _target: &Target,
        _mode: CompileMode,
        _on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        _on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        let args = command.get_args();
        let out_dir_key = OsString::from("--out-dir");
        let out_dir_key_idx = args
            .iter()
            .position(|s| *s == out_dir_key)
            .ok_or_else(|| CustomExecutorError::OutDirKeyMissing(command.to_string()))?;
        let out_dir = args
            .get(out_dir_key_idx + 1)
            .ok_or_else(|| CustomExecutorError::OutDirValueMissing(command.to_string()))
            .map(PathBuf::from)?;

        // This can be different from the cwd used to launch the wrapping cargo
        // plugin. Discovered while fixing
        // https://github.com/anderejd/cargo-geiger/issues/19
        let cwd = command
            .get_cwd()
            .map_or_else(|| self.cwd.clone(), PathBuf::from);

        {
            // Scope to drop and release the mutex before calling rustc.
            let mut ctx = self
                .inner_ctx
                .lock()
                .map_err(|e| CustomExecutorError::InnerContextMutex(e.to_string()))?;
            for tuple in args
                .iter()
                .map(|s| (s, s.to_string_lossy().to_lowercase()))
                .filter(|t| t.1.ends_with(".rs"))
            {
                let raw_path = cwd.join(tuple.0);
                let p = raw_path
                    .canonicalize()
                    .map_err(|e| CustomExecutorError::Io(e, raw_path))?;
                ctx.rs_file_args.insert(p);
            }
            ctx.out_dir_args.insert(out_dir);
        }
        command.exec()?;
        Ok(())
    }

    /// Queried when queuing each unit of work. If it returns true, then the
    /// unit will always be rebuilt, independent of whether it needs to be.
    fn force_rebuild(&self, _unit: &Unit) -> bool {
        true // Overriding the default to force all units to be processed.
    }
}

pub fn get_tainted(
    config: &cargo::Config,
    workspace: &cargo::core::Workspace,
    _package: &Option<String>,
    include_tests: bool,
) -> anyhow::Result<Vec<String>> {
    let (packages, _resolve) = cargo::ops::resolve_ws(workspace)?;

    let copt = CompileOptions::new(config, CompileMode::Build)?;
    let rs_files_used_in_compilation = resolve_rs_file_deps(&copt, workspace)?;

    let allow_partial_results = true;

    let (rs_files_scanned, tainted_things) = find_unsafe_in_packages(
        &packages,
        rs_files_used_in_compilation,
        allow_partial_results,
        include_tests,
    );

    rs_files_scanned
        .iter()
        .filter(|(_k, v)| **v == 0)
        .for_each(|(k, _v)| {
            // TODO: Ivestigate if this is related to code generated by build
            // scripts and/or macros. Some of the warnings of this kind is
            // printed for files somewhere under the "target" directory.
            // TODO: Find out if we can lookup PackageId associated with each
            // `.rs` file used by the build, including the file paths extracted
            // from `.d` dep files.
            warn!("Dependency file was never scanned: {}", k.display());
        });

    Ok(tainted_things)
}
