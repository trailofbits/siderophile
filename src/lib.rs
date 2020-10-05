#![forbid(unsafe_code)]

mod callgraph_gen;
mod utils;
pub use callgraph_gen::{gen_callgraph, trace_unsafety};
pub use utils::{configure_rustup_toolchain, simplify_trait_paths, CallGraph};
