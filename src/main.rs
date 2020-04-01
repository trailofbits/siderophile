#![forbid(unsafe_code)]

#[macro_use]
extern crate log;

mod callgraph_matching;
mod trawl_source;

use cargo::core::shell::Shell;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum Args {
    Trawl(trawl_source::TrawlArgs),
    Trace(callgraph_matching::TraceArgs),
}

fn main() {
    env_logger::init();
    let mut config = match cargo::Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };
    if let Err(e) = match Args::from_args() {
        Args::Trawl(args) => trawl_source::real_main(&args, &mut config),
        Args::Trace(args) => callgraph_matching::real_main(&args),
    } {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
