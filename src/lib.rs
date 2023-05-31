#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
#![deny(clippy::pedantic, clippy::nursery, warnings)]

mod callgraph_gen;
mod utils;
pub use callgraph_gen::{gen_callgraph, trace_unsafety};
pub use utils::{configure_rustup_toolchain, simplify_trait_paths, CallGraph};
