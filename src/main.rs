#![forbid(unsafe_code)]

#[macro_use]
extern crate log;

mod callgraph_gen;
mod mark_source;
mod trawl_source;
mod utils;

use std::collections::HashMap;

use anyhow::{anyhow, bail};
use cargo::{
    core::{Package, Workspace},
    util::Filesystem,
};
use structopt::{clap, StructOpt};
use tempfile::tempdir_in;

#[derive(StructOpt, Debug)]
#[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
pub struct Args {
    #[structopt(long = "crate-name", value_name = "NAME")]
    /// Crate name (deprecated)
    crate_name: Option<String>,

    #[structopt(long = "package", short = "p", value_name = "SPEC")]
    /// Package to be used as the root of the tree
    package: Option<String>,

    #[structopt(long = "include-tests")]
    /// Count unsafe usage in tests.
    include_tests: bool,

    #[structopt(flatten)]
    mark_opts: mark_source::MarkOpts,
}

fn real_main(args: &Args) -> anyhow::Result<HashMap<String, (u32, utils::LabelInfo)>> {
    let config = cargo::Config::default()?;
    let workspace_root = cargo::util::important_paths::find_root_manifest_for_wd(config.cwd())?;
    let ws = cargo::core::Workspace::new(&workspace_root, &config)?;

    let mut ws = if let Some(name) = &args.package {
        let package =
            find_package(&ws, name).ok_or_else(|| anyhow!("Could not find package `{}`", name))?;
        Workspace::ephemeral(package.clone(), &config, None, false)?
    } else {
        ws
    };

    let tempdir = tempdir_in(config.cwd())?;
    ws.set_target_dir(Filesystem::new(tempdir.path().to_path_buf()));

    let crate_name = crate_name(&ws, &args.package)?;

    if let Some(deprecated_crate_name) = &args.crate_name {
        eprintln!("Warning: `--crate-name` is deprecated. Use `--package` instead.");
        if deprecated_crate_name != &crate_name {
            bail!(
                "Crate `{}` was specified, but crate `{}` was found",
                deprecated_crate_name,
                crate_name
            );
        }
    }

    // new language, same horrible horrible hack. see PR#22 and related issues, this makes me sad....
    utils::configure_rustup_toolchain();

    // smoelius: `trawl_source::get_tainted` must be called before `callgraph_gen::gen_callgraph`
    // because `get_tainted` performs the build.
    let tainted = trawl_source::get_tainted(&config, &ws, &args.package, args.include_tests)?;
    let callgraph = callgraph_gen::gen_callgraph(&ws, &crate_name)?;
    Ok(callgraph_gen::trace_unsafety(
        &callgraph,
        &crate_name,
        &tainted,
    ))
}

fn find_package<'ws>(ws: &'ws Workspace, name: &str) -> Option<&'ws Package> {
    ws.members().find(|package| package.name() == name)
}

fn crate_name(ws: &Workspace, package: &Option<String>) -> anyhow::Result<String> {
    package.as_ref().cloned().map_or_else(
        || ws.current().map(|package| package.name().to_string()),
        Ok,
    )
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::from_args();
    real_main(&args).and_then(|badness| {
        println!("Badness  Function");
        let mut badness_out_list: Vec<(&str, &u32)> =
            badness.iter().map(|(a, (b, _))| (a as &str, b)).collect();
        badness_out_list.sort_by_key(|(a, b)| (u32::MAX - *b, *a));
        for (label, badness) in badness_out_list {
            println!("    {badness:03}  {label}");
        }
        mark_source::mark_source(&args.mark_opts, &badness)?;
        Ok(())
    })
}
