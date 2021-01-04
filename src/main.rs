#![forbid(unsafe_code)]

#[macro_use]
extern crate log;

mod callgraph_gen;
mod mark_source;
mod trawl_source;
mod utils;

use cargo::core::shell::Shell;
use cargo::util::errors::CliError;
use std::collections::HashMap;
use structopt::{clap, StructOpt};

#[derive(StructOpt, Debug)]
#[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
pub struct Args {
    #[structopt(long = "crate-name", value_name = "NAME")]
    /// Crate name
    crate_name: String,

    #[structopt(long = "package", short = "p", value_name = "SPEC")]
    /// Package to be used as the root of the tree
    package: Option<String>,

    #[structopt(long = "include-tests")]
    /// Count unsafe usage in tests.
    include_tests: bool,

    #[structopt(flatten)]
    mark_opts: mark_source::MarkOpts,
}

fn real_main(args: &Args) -> anyhow::Result<HashMap<String, (u32, utils::LabelInfo)>, CliError> {
    let config = cargo::Config::default()?;
    let workspace_root = cargo::util::important_paths::find_root_manifest_for_wd(config.cwd())?;
    let ws = cargo::core::Workspace::new(&workspace_root, &config)?;

    // new language, same horrible horrible hack. see PR#22 and related issues, this makes me sad....
    utils::configure_rustup_toolchain();

    let tainted = trawl_source::get_tainted(&config, &ws, &args.package, args.include_tests)?;
    let callgraph = callgraph_gen::gen_callgraph(&ws, &args.crate_name)?;
    Ok(callgraph_gen::trace_unsafety(
        callgraph,
        &args.crate_name,
        tainted,
    ))
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::from_args();
    match real_main(&args) {
        Ok(badness) => {
            println!("Badness  Function");
            let mut badness_out_list: Vec<(&str, &u32)> =
                badness.iter().map(|(a, (b, _))| (a as &str, b)).collect();
            badness_out_list.sort_by_key(|(a, b)| (std::u32::MAX - *b, *a));
            for (label, badness) in badness_out_list {
                println!("    {:03}  {}", badness, label)
            }
            mark_source::mark_source(&args.mark_opts, &badness)?;
        }
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e, &mut shell);
        }
    }
    Ok(())
}
