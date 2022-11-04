use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{anyhow, Context};
use cargo::core::Workspace;
use glob::glob;
use llvm_ir::Name::Name;
use llvm_ir::Operand::ConstantOperand;
use llvm_ir::Terminator::CallBr;
use llvm_ir::Terminator::Invoke;
use llvm_ir::{instruction::Instruction, Module};
use regex::Regex;
use rustc_demangle::demangle;
use utils::LabelInfo;

use crate::utils;

// emit llvm IR. disable optimizations. just want debug info and call graph...
#[allow(dead_code)]
pub const RUSTFLAGS: &str = "-C lto=no -C opt-level=0 -C debuginfo=2 --emit=llvm-bc";

fn parse_ir_file(ir_path: &Path) -> anyhow::Result<utils::CallGraph> {
    // removes hex identifiers for short ids
    let re = Regex::new("(.*)::h[a-f0-9]{16}")?;

    let module = Module::from_bc_path(ir_path).map_err(|s| anyhow::anyhow!(s))?;
    let mut label_to_label_info: HashMap<String, LabelInfo> = HashMap::new();
    let mut short_label_to_labels: HashMap<String, HashSet<String>> = HashMap::new();

    for fun in module.functions {
        let dem_fun = demangle(&fun.name).to_string();
        let short_fun = {
            let simplified = utils::simplify_trait_paths(&dem_fun.clone());
            re.captures(&simplified).map_or(|caps| caps[1].to_string(), simplified)
        };
        short_label_to_labels
            .entry(short_fun.clone())
            .or_insert_with(HashSet::new)
            .insert(dem_fun.clone());
        let label_info = label_to_label_info
            .entry(dem_fun.clone())
            .or_insert_with(LabelInfo::new);
        label_info.short_label = Some(short_fun);
        label_info.debugloc = fun.debugloc;
        // TODO: clean this up wow what a mess...
        for bb in fun.basic_blocks {
            for instr in bb.instrs {
                if let Instruction::Call(call) = instr {
                    if let Some(ConstantOperand(op)) = call.function.right() {
                        if let llvm_ir::constant::Constant::GlobalReference {
                            name: Name(called_name),
                            ..
                        } = &*op
                        {
                            let dem_called = demangle(called_name).to_string();
                            label_to_label_info
                                .entry(dem_called)
                                .or_insert_with(LabelInfo::new)
                                .caller_labels
                                .insert(dem_fun.clone());
                        }
                    }
                }
            }
            // TODO: something about this clone. prolly a match?
            if let Invoke(inv) = bb.term.clone() {
                if let Some(ConstantOperand(op)) = inv.function.right() {
                    if let llvm_ir::constant::Constant::GlobalReference {
                        name: Name(called_name),
                        ..
                    } = &*op
                    {
                        let dem_called = demangle(called_name).to_string();
                        label_to_label_info
                            .entry(dem_called)
                            .or_insert_with(LabelInfo::new)
                            .caller_labels
                            .insert(dem_fun.clone());
                    }
                }
            }
            if let CallBr(cbr) = bb.term {
                if let Some(ConstantOperand(op)) = cbr.function.right() {
                    if let llvm_ir::constant::Constant::GlobalReference {
                        name: Name(called_name),
                        ..
                    } = &*op
                    {
                        let dem_called = demangle(called_name).to_string();
                        label_to_label_info
                            .entry(dem_called)
                            .or_insert_with(LabelInfo::new)
                            .caller_labels
                            .insert(dem_fun.clone());
                    }
                }
            }
        }
    }
    Ok(utils::CallGraph {
        label_to_label_info,
        short_label_to_labels,
    })
}

#[allow(clippy::missing_errors_doc)]
pub fn gen_callgraph(ws: &Workspace, crate_name: &str) -> anyhow::Result<utils::CallGraph> {
    // find llvm IR file
    let mut file = ws.target_dir().into_path_unlocked();
    file.push("debug");
    file.push("deps");
    file.push(format!("{}*.bc", str::replace(crate_name, "-", "_")));
    let filestr = file
        .to_str()
        .ok_or_else(|| anyhow!("Failed to make file string for finding bytecode"))?;
    // TODO: error handle, test against other OS's
    let path = glob(filestr)
        .with_context(|| "Failed to read glob pattern")?
        .next()
        .ok_or_else(|| anyhow!("could not find bytecode file"))??;
    parse_ir_file(&path)
}

#[allow(clippy::missing_panics_doc, clippy::unwrap_used)]
#[must_use]
pub fn trace_unsafety(
    callgraph: &utils::CallGraph,
    crate_name: &str,
    tainted_function_names: &[String],
) -> HashMap<String, (u32, LabelInfo)> {
    let mut tainted_function_labels = HashSet::new();
    for t in tainted_function_names {
        let short_label = utils::simplify_trait_paths(t);
        if let Some(labels) = callgraph.short_label_to_labels.get(&short_label) {
            tainted_function_labels.extend(labels);
        }
    }

    let mut label_to_badness: HashMap<String, (u32, LabelInfo)> = HashMap::new();
    for tainted_function in tainted_function_labels {
        // traversal of the call graph from tainted node
        let mut queued_to_traverse: Vec<String> = vec![tainted_function.to_string()];
        let mut tainted_by: HashSet<String> = HashSet::new();
        tainted_by.insert(tainted_function.to_string());
        while !queued_to_traverse.is_empty() {
            let current_node = queued_to_traverse.pop().unwrap();
            if let Some(label_info) = callgraph.label_to_label_info.get(&current_node) {
                for caller_node in &label_info.caller_labels {
                    if !tainted_by.contains(caller_node) {
                        queued_to_traverse.push(caller_node.clone());
                        tainted_by.insert(caller_node.clone());
                    }
                }
            }
        }

        for tainted_by_node_id in &tainted_by {
            if let Some(label_info) = callgraph.label_to_label_info.get(tainted_by_node_id) {
                if let Some(shortlabel) = &label_info.short_label {
                    label_to_badness
                        .entry(shortlabel.to_string())
                        .and_modify(|e| e.0 += 1)
                        .or_insert((1, label_info.clone()));
                }
            }
        }
    }

    let mut ret_badness: HashMap<String, (u32, LabelInfo)> = HashMap::new();
    // To print this out, we have to dedup all the node labels, since multiple nodes can have the same label
    for (label, badness) in &label_to_badness {
        ret_badness
            .entry(utils::simplify_trait_paths(label))
            .or_insert((0, badness.1.clone()))
            .0 += badness.0;
    }
    // filter out any badness results that are not in the crate
    let re = Regex::new(&format!(r"^<*{}::", str::replace(crate_name, "-", "_"))).unwrap();
    ret_badness.retain(|k, _| re.is_match(k));
    ret_badness
}
