use llvm_ir::{Module, instruction::Instruction};
use llvm_ir::Operand::ConstantOperand;
use llvm_ir::Terminator::Invoke;
use llvm_ir::Terminator::CallBr;
use llvm_ir::Name::Name;
use rustc_demangle::demangle;
use std::path::Path;
use glob::glob;

use std::process::Command;
use std::collections::HashMap;
use std::collections::HashSet;
use cargo::core::Workspace;
use regex::Regex;
use anyhow;
use crate::utils;

fn parse_ir_file(ir_path: &Path) -> anyhow::Result<utils::CallGraph> {
    // removes hex identifiers for short ids
    let re = Regex::new("(.*)::h[a-f0-9]{16}").unwrap();

    let module = Module::from_bc_path(&ir_path).map_err(|s| anyhow::anyhow!(s))?;
    let mut label_to_caller_labels: HashMap<String, HashSet<String>> = HashMap::new();
    let mut short_label_to_labels: HashMap<String, HashSet<String>> = HashMap::new();

    // for fast lookup of pretty labels for output
    let mut label_to_short_label: HashMap<String, String> = HashMap::new();
    for fun in module.functions {
        let dem_fun = demangle(&fun.name).to_string();
        let short_fun = {
            let simplified = utils::simplify_trait_paths(&dem_fun.clone());
            match re.captures(&simplified) {
                Some(caps) => caps[1].to_string(),
                None => simplified
            }
        };
        short_label_to_labels.entry(short_fun.clone()).or_insert(HashSet::new()).insert(dem_fun.clone());
        label_to_short_label.insert(dem_fun.clone(), short_fun);
        // TODO: clean this up wow what a mess...
        for bb in fun.basic_blocks {
            for instr in bb.instrs {
                if let Instruction::Call(call) = instr {
                    if let Some(ConstantOperand(op)) = call.function.right() {
                        if let llvm_ir::constant::Constant::GlobalReference { name: Name(called_name), .. } = op {
                            let dem_called = demangle(&called_name).to_string();
                            label_to_caller_labels.entry(dem_called).or_insert(HashSet::new()).insert(dem_fun.clone());
                        }
                    }
                }
            }
            // TODO: something about this clone. prolly a match?
            if let Invoke(inv) = bb.term.clone() {
                if let Some(ConstantOperand(op)) = inv.function.right() {
                    if let llvm_ir::constant::Constant::GlobalReference { name: Name(called_name), .. } = op {
                        let dem_called = demangle(&called_name).to_string();
                        label_to_caller_labels.entry(dem_called).or_insert(HashSet::new()).insert(dem_fun.clone());
                    }
                }
            }
            if let CallBr(cbr) = bb.term {
                if let Some(ConstantOperand(op)) = cbr.function.right() {
                    if let llvm_ir::constant::Constant::GlobalReference { name: Name(called_name), .. } = op {
                        let dem_called = demangle(&called_name).to_string();
                        label_to_caller_labels.entry(dem_called).or_insert(HashSet::new()).insert(dem_fun.clone());
                    }
                }
            }
        }
    }
    Ok(utils::CallGraph{label_to_caller_labels, short_label_to_labels, label_to_short_label})
}

pub fn gen_callgraph(ws: &Workspace, crate_name: &String) -> anyhow::Result<utils::CallGraph> {
    // run cargo clean
    Command::new("cargo").arg("clean").status().expect("failed to clean workspace before generating bytecode");

    // emit llvm IR. disable debug symbols and optimizations. just want call graph...
    Command::new("cargo").arg("rustc").env("RUSTFLAGS", "-C lto=no -C opt-level=0 -C debuginfo=0 --emit=llvm-bc").status().expect("failed to emit llvm IR");

    // find llvm IR file
    let mut file = ws.target_dir().into_path_unlocked();
    file.push("debug");
    file.push("deps");
    file.push(format!("{}*.bc", crate_name));
    let filestr = file.to_str().expect("Failed to make file string for finding bytecode");
    // TODO: error handle, test against other OS's
    let path = glob(filestr).expect("Failed to read glob pattern").next().expect("could not find bytecode file")?;
    parse_ir_file(&path)
}

pub fn trace_unsafety(callgraph: utils::CallGraph, crate_name: &str, tainted_function_names: Vec<String>) -> HashMap<String, u32> {
    let mut tainted_function_labels = HashSet::new();
    for t in tainted_function_names.iter() {
        let short_label = utils::simplify_trait_paths(t);
        if let Some(labels) = callgraph.short_label_to_labels.get(&short_label) {
            tainted_function_labels.extend(labels);
        }
    }

    let mut label_to_badness: HashMap<String, u32> = HashMap::new();
    for tainted_function in tainted_function_labels.iter() {
        // traversal of the call graph from tainted node
        let mut queued_to_traverse: Vec<String> = vec![tainted_function.to_string()];
        let mut tainted_by: HashSet<String> = HashSet::new();
        tainted_by.insert(tainted_function.to_string());
        while !queued_to_traverse.is_empty() {
            let current_node = queued_to_traverse.pop().unwrap();
            if let Some(caller_nodes) = callgraph.label_to_caller_labels.get(&current_node) {
                for caller_node in caller_nodes {
                    if !tainted_by.contains(caller_node) {
                        queued_to_traverse.push(caller_node.clone());
                        tainted_by.insert(caller_node.clone());
                    }
                }
            }
        }

        for tainted_by_node_id in tainted_by.iter() {
            if let Some(shortlabel) = callgraph.label_to_short_label.get(tainted_by_node_id) {
                label_to_badness
                    .entry(shortlabel.to_string())
                    .and_modify(|e| *e += 1)
                    .or_insert(1);
            }
        }
    }

    let mut ret_badness: HashMap<String, u32> = HashMap::new();
    // To print this out, we have to dedup all the node labels, since multiple nodes can have the same label
    for (label, badness) in label_to_badness.iter() {
        *ret_badness
            .entry(utils::simplify_trait_paths(label))
            .or_insert(0) += *badness;
    }
    // filter out any badness results that are not in the crate
    let re = Regex::new(&format!(r"^<*{}::", crate_name)).unwrap();
    ret_badness.retain(|k, _| re.is_match(&k));
    ret_badness
}
