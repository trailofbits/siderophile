#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::panic, clippy::expect_used, warnings)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

mod callgraph_gen;
mod utils;
pub use callgraph_gen::{gen_callgraph, trace_unsafety};
pub use utils::{configure_rustup_toolchain, simplify_trait_paths, CallGraph};
