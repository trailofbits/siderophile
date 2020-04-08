use cargo::CliResult;
use regex::{Captures, Regex};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

// This funciton takes a Rust module path like
// `<T as failure::as_fail::AsFail>::as_fail and strips`
// down the fully-qualified trait paths within to just the base trait name, like
// `<T as AsFail>::as_fail`
fn get_base_trait_name(after_as: &str) -> Option<String> {
    //Read until the first ">" character, which marks the end of the trait path. We do not modify *rest
    let mut parts = after_as.split(">");
    let path = parts.next()?;
    let mut rest: Vec<&str> = parts.collect();
    // This is the "AsFail" in the example
    let basename: &str = *path.split("::").collect::<Vec<&str>>().last()?;
    rest.insert(0, basename);
    Some(rest.join(">"))
}

fn simplify_trait_paths(path: String) -> String {
    let parts: Vec<&str> = path.split(" as ").collect();
    if parts.len() == 1 {
        path
    } else {
        parts.into_iter()
            .enumerate()
            .map(|(i, after_as)|
                // Every other segment here is what comes before the " as ", which we do not modify.
                // So just append it to the list and move on
                if i % 2 == 0 {after_as.to_string()} else { get_base_trait_name(after_as).unwrap() }
            )
            .collect::<Vec<String>>()
            // Surgery complete. Stitch it all back up.
            .join(" as ").to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::matching::simplify_trait_paths;

    #[test]
    fn test_1() {
        assert_eq!(simplify_trait_paths("<&mut std::collections::hash::table::RawTable<K,V> as std::collections::hash::table::Put<K,V>>::borrow_table_mut".to_string()), "<&mut std::collections::hash::table::RawTable<K,V> as Put<K,V>>::borrow_table_mut");
    }
    #[test]
    fn test_2() {
        assert_eq!(
            simplify_trait_paths(
                "<futures::lock::TryLock<T> as core::ops::deref::Deref>::deref".to_string()
            ),
            "<futures::lock::TryLock<T> as Deref>::deref"
        );
    }
    #[test]
    fn test_3() {
        assert_eq!(simplify_trait_paths("<network::proto::state_synchronizer::RequestChunk as ::protobuf::Message>::default_instance".to_string()), "<network::proto::state_synchronizer::RequestChunk as Message>::default_instance");
    }
    #[test]
    fn test_4() {
        assert_eq!(
            simplify_trait_paths("<T as failure::as_fail::AsFail>::as_fail".to_string()),
            "<T as AsFail>::as_fail"
        );
    }
}

struct CallGraph {
    node_id_to_full_label: HashMap<String, String>,
    node_id_to_caller_nodes: HashMap<String, HashSet<String>>,
    tainted_node_ids: HashSet<String>,
}

// TODO: nicer error handling than all these unwrap()s
fn parse_input_data(
    demangled_callgraph_lines: Vec<String>,
    tainted_nodes: Vec<String>,
) -> CallGraph {
    let node_re = Regex::new(r#"^\W*(.*?) \[shape=record,label="\{(.*?)\}"\];"#).unwrap();
    let edge_re = Regex::new(r#"\W*(.*) -> (.*);"#).unwrap();

    let mut node_id_to_full_label: HashMap<String, String> = HashMap::new();
    let mut node_id_to_caller_nodes: HashMap<String, HashSet<String>> = HashMap::new();
    let mut label_to_node_ids: HashMap<String, HashSet<String>> = HashMap::new();

    for contents in demangled_callgraph_lines {
        if contents.find("->").is_none() {
            // found a new node!
            for cap in node_re.captures_iter(&contents) {
                let node_id = cap[1].to_string();
                let full_label = cap[2].to_string();
                node_id_to_full_label.insert(node_id.clone(), full_label.clone());
                let short_label = simplify_trait_paths(full_label.clone());
                if let Some(node_ids) = label_to_node_ids.get_mut(&short_label) {
                    node_ids.insert(node_id.to_string());
                } else {
                    let mut hs = HashSet::new();
                    hs.insert(node_id);
                    label_to_node_ids.insert(short_label, hs);
                }
            }
        } else {
            // found a new edge!
            for cap in edge_re.captures_iter(&contents) {
                match node_id_to_caller_nodes.get_mut(&cap[2].to_string()) {
                    Some(set) => {
                        set.insert(cap[1].to_string());
                    }
                    None => {
                        let mut set: HashSet<String> = HashSet::new();
                        set.insert((&cap[1]).to_string());
                        node_id_to_caller_nodes.insert(cap[2].to_string(), set);
                    }
                }
            }
        }
    }

    let mut tainted_node_ids: HashSet<String> = HashSet::new();

    for tainted_node in tainted_nodes {
        let label = simplify_trait_paths(tainted_node);
        if let Some(node_ids) = label_to_node_ids.get(&label) {
            for nid in node_ids.iter() {
                tainted_node_ids.insert(nid.to_string());
            }
        }
    }
    CallGraph {
        node_id_to_full_label,
        node_id_to_caller_nodes,
        tainted_node_ids,
    }
}

fn trace_unsafety(callgraph: CallGraph, crate_name: &String) -> HashMap<String, u32> {
    // TODO: for each tainted node, parse through and get all things that call it. then increment each of their badnesses by 1.
    let mut node_id_to_badness: HashMap<String, u32> = HashMap::new();
    // for (node_id, _) in callgraph.node_id_to_full_label.iter() {
    //     node_id_to_badness.insert(node_id.to_string(), 0);
    // }

    for tainted_node_id in callgraph.tainted_node_ids.iter() {
        // traversal of the call graph from tainted node
        let mut queued_to_traverse: Vec<String> = vec![tainted_node_id.clone()];
        let mut tainted_by: HashSet<String> = HashSet::new();
        tainted_by.insert(tainted_node_id.clone());
        while queued_to_traverse.len() > 0 {
            let current_node_id = queued_to_traverse.pop().unwrap();
            if let Some(caller_nodes) = callgraph.node_id_to_caller_nodes.get(&current_node_id) {
                for caller_node_id in caller_nodes {
                    if !tainted_by.contains(caller_node_id) {
                        queued_to_traverse.push(caller_node_id.clone());
                        tainted_by.insert(caller_node_id.clone());
                    }
                }
            }
        }

        // TODO: iterate over all tainted_by and increment their badness
        for tainted_by_node_id in tainted_by.iter() {
            node_id_to_badness
                .entry(tainted_by_node_id.to_string())
                .and_modify(|e| *e += 1)
                .or_insert(1);
        }
    }

    let mut ret_badness: HashMap<String, u32> = HashMap::new();
    // To print this out, we have to dedup all the node labels, since multiple nodes can have the same label
    for (tainted_node_id, badness) in node_id_to_badness.iter() {
        let node = callgraph
            .node_id_to_full_label
            .get(&tainted_node_id.clone())
            .unwrap();
        ret_badness
            .entry(node.clone())
            .and_modify(|old_badness| *old_badness += *badness)
            .or_insert(*badness);
    }
    // filter out any badness results that are not in the crate
    let re = Regex::new(&format!(r"^<*{}::", *crate_name)).unwrap();
    ret_badness.retain(|k, _| re.is_match(&k));
    ret_badness
}

fn do_output(badness: HashMap<String, u32>) {
    println!("Badness  Function");
    let mut badness_out_list = badness.iter().collect::<Vec<(&String, &u32)>>();
    badness_out_list.sort_by_key(|(a, b)| (std::u32::MAX - *b, a.clone()));
    for (label, badness) in badness_out_list {
        println!("    {:03}  {}", badness, label)
    }
}

fn do_demangle(input: &PathBuf) -> io::Result<Vec<String>> {
    let mut in_reader = BufReader::new(File::open(input)?);
    let mangled_name_regex: Regex = Regex::new(r"_(ZN|R)[\$\._[:alnum:]]*").unwrap();
    // NOTE: this is actually more efficient than lines(), since it re-uses the buffer
    let mut buf = String::new();
    let mut output_vec: Vec<String> = Vec::new();
    while in_reader.read_line(&mut buf)? > 0 {
        // NOTE: This includes the line-ending, and leaves it untouched
        output_vec.push(
            mangled_name_regex
                .replace_all(&buf, |captures: &Captures| {
                    format!("{:#}", rustc_demangle::demangle(&captures[0]))
                })
                .to_owned()
                .to_string(),
        );

        buf.clear(); // Reset the buffer's position, without freeing it's underlying memory
    }
    Ok(output_vec) // Successfully hit EOF
}

pub fn callgraph_matching(
    callgraph_file: &PathBuf,
    unsafe_deps: Vec<String>,
    crate_name: String,
) -> CliResult {
    let demangled_callgraph_lines = do_demangle(callgraph_file).unwrap();
    let callgraph = parse_input_data(demangled_callgraph_lines, unsafe_deps);
    let badness = trace_unsafety(callgraph, &crate_name);
    do_output(badness);
    Ok(())
}
