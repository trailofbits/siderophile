#![forbid(unsafe_code)]

#[macro_use]
extern crate log;
mod utils;
mod callgraph_gen;
mod trawl_source;
pub use utils::{configure_rustup_toolchain, CallGraph};
pub use trawl_source::get_tainted;
pub use callgraph_gen::{gen_callgraph, trace_unsafety};
