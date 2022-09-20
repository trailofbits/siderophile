use std::collections::HashMap;
use std::fs::{copy, File};
use std::io::{BufRead, BufReader, LineWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use regex::Regex;
use structopt::StructOpt;
use tempfile::NamedTempFile;

use crate::utils::LabelInfo;

type BadnessMap = HashMap<String, (u32, LabelInfo)>;

#[derive(StructOpt, Debug)]
pub struct MarkOpts {
    #[structopt(long = "mark", value_name = "TEXT")]
    /// Mark bad functions with TEXT
    mark: Option<String>,

    #[structopt(long = "no-mark-closures")]
    /// Do not mark closures
    no_mark_closures: bool,

    #[structopt(long = "threshold", value_name = "BADNESS", default_value)]
    /// Minimum badness required to mark a function
    threshold: u32,
}

pub fn mark_source(opts: &MarkOpts, badness: &BadnessMap) -> Result<()> {
    let text = if let Some(text) = &opts.mark {
        text
    } else {
        return Ok(());
    };

    let grouped = group_by_path(badness);

    for entry in grouped {
        mark_path(opts, text, &entry.0, &entry.1)?;
    }

    Ok(())
}

fn group_by_path(badness: &BadnessMap) -> HashMap<PathBuf, BadnessMap> {
    let mut grouped = HashMap::new();
    for entry in badness {
        if let Some(debugloc) = &entry.1 .1.debugloc {
            let path = debugloc
                .directory
                .as_ref()
                .map_or(PathBuf::from(&debugloc.filename), |directory| {
                    PathBuf::from(directory).join(&debugloc.filename)
                });
            grouped
                .entry(path)
                .or_insert_with(HashMap::new)
                .insert(entry.0.clone(), entry.1.clone());
        }
    }
    grouped
}

fn mark_path(opts: &MarkOpts, text: &str, path: &Path, badness: &BadnessMap) -> Result<()> {
    let line_numbers = extract_line_numbers(opts, badness);

    let source = File::open(path)?;
    let reader = BufReader::new(source);

    let tempfile = NamedTempFile::new()?;
    let mut writer = LineWriter::new(&tempfile);

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if line_numbers.binary_search(&(i + 1)).is_ok() {
            let spaces = Regex::new(r"^\s*")?
                .find(&line)
                .ok_or_else(|| anyhow!("Unexpected input"))?
                .as_str();
            writeln!(writer, "{}{}", spaces, text)?;
        }
        writeln!(writer, "{}", line)?;
    }

    drop(writer);

    copy(tempfile.path(), path)?;

    Ok(())
}

fn extract_line_numbers(opts: &MarkOpts, badness: &BadnessMap) -> Vec<usize> {
    let mut line_numbers = badness
        .iter()
        .filter_map(|entry| {
            if (!opts.no_mark_closures || !entry.0.ends_with("{{closure}}"))
                && entry.1 .0 >= opts.threshold
            {
                entry
                    .1
                     .1
                    .debugloc
                    .as_ref()
                    .map(|debugloc| debugloc.line as usize)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    line_numbers.sort_unstable();
    line_numbers
}
