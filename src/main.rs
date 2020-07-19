#![forbid(unsafe_code)]

#[macro_use]
extern crate log;

mod callgraph_gen;
mod trawl_source;
mod utils;

use std::collections::HashMap;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Args {
    #[structopt(long = "crate-name", value_name = "NAME")]
    /// crate name
    crate_name: String,

    #[structopt(long = "package", short = "p", value_name = "SPEC")]
    /// Package to be used as the root of the tree
    package: Option<String>,

    #[structopt(long = "include-tests")]
    /// Count unsafe usage in tests.
    include_tests: bool,
}

fn real_main() -> anyhow::Result<HashMap<String, u32>> {
    let args = Args::from_args();

    let config = cargo::Config::default()?;
    let workspace_root = cargo::util::important_paths::find_root_manifest_for_wd(config.cwd())?;
    let ws = cargo::core::Workspace::new(&workspace_root, &config)?;

    let tainted = trawl_source::get_tainted(&config, &ws, args.package, args.include_tests)?;
    let callgraph = callgraph_gen::gen_callgraph(&ws, &args.crate_name)?;
    Ok(callgraph_gen::trace_unsafety(
        callgraph,
        &args.crate_name,
        tainted,
    ))
}

fn main() {
    env_logger::init();
    match real_main() {
        Ok(badness) => {
            println!("Badness  Function");
            let mut badness_out_list: Vec<(&str, &u32)> =
                badness.iter().map(|(a, b)| (a as &str, b)).collect();
            badness_out_list.sort_by_key(|(a, b)| (std::u32::MAX - *b, *a));
            for (label, badness) in badness_out_list {
                println!("    {:03}  {}", badness, label)
            }
        }
        // TODO: add proper tracebacks or something
        Err(e) => println!("error: {}", e),
    }
}
