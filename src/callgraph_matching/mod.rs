use cargo::CliResult;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct MatchArgs {
    #[structopt(long = "callgraph-file", value_name = "FILE", parse(from_os_str))]
    /// input callgraph file
    input_callgraph: PathBuf,
    #[structopt(long = "unsafe-deps-file", value_name = "FILE", parse(from_os_str))]
    /// input unsafe deps file
    input_unsafe_deps_file: PathBuf,
    #[structopt(long = "crate-name", value_name = "NAME")]
    /// crate name
    crate_name: String
}

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
                //Every other segment here is what comes before the " as ", which we do not modify.
                //So just append it to the list and move on
                if i % 2 == 0 {after_as.to_string()} else { get_base_trait_name(after_as).unwrap() }
            )
            .collect::<Vec<String>>()
            // Surgery complete. Stitch it all back up.
            .join(" as ").to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::callgraph_matching::simplify_trait_paths;

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

struct Node {
    node_id: String,
    label: String,
    full_label: String,
    // TODO: ideally would use node pointers here... but like idk lifetimes make no sense haha
    caller_node_ids: HashSet<String>,
    badness: u32,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}
impl Eq for Node {}

impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
    }
}

struct CallGraph {
    node_id_to_node: HashMap<String, Node>,
    label_to_node_id: HashMap<String, String>,
    tainted_node_ids: Vec<String>,
}

// TODO: nicer error handling than all these unwrap()s
fn parse_input_data(
    callgraph_filename: &PathBuf,
    tainted_nodes_filename: &PathBuf,
) -> CallGraph {
    let node_re = Regex::new(r#"^\W*(.*?) \[shape=record,label="{(.*?)}"\];"#).unwrap();
    let edge_re = Regex::new(r#"\W*(.*) -> (.*);"#).unwrap();

    let mut node_id_to_node: HashMap<String, Node> = HashMap::new();
    let mut label_to_node_id: HashMap<String, String> = HashMap::new();

    let cg_file = File::open(callgraph_filename).unwrap();
    for line in io::BufReader::new(cg_file).lines() {
        if let Ok(contents) = line {
            if contents.find("->").is_none() {
                // found a new node!
                for cap in node_re.captures_iter(&contents) {
                    let node_id = cap[1].to_string();
                    let full_label = cap[2].to_string();
                    let label = simplify_trait_paths(full_label.clone());
                    let node = Node {
                        node_id: node_id.clone(),
                        label: label.clone(),
                        full_label,
                        caller_node_ids: HashSet::new(),
                        badness: 0,
                    };
                    node_id_to_node.insert(node_id.clone(), node);
                    label_to_node_id.insert(label, node_id);
                }
            } else {
                // found a new edge!
                for cap in edge_re.captures_iter(&contents) {
                    let from_node_id = node_id_to_node.get(&cap[1]).unwrap().node_id.clone();
                    let to_node = node_id_to_node.get_mut(&cap[2]).unwrap();
                    to_node.caller_node_ids.insert(from_node_id);
                }
            }
        }
    }

    let tn_file = File::open(tainted_nodes_filename).unwrap();
    let mut tainted_node_ids : Vec<String> = Vec::new();

    for line in io::BufReader::new(tn_file).lines() {
        if let Ok(contents) = line {
            tainted_node_ids.push(label_to_node_id.get(&simplify_trait_paths(contents)).unwrap().to_string());
        }
    }

    CallGraph {
        node_id_to_node,
        label_to_node_id,
        tainted_node_ids,
    }
}

fn trace_unsafety(_callgraph: CallGraph, _crate_name: &String) -> HashMap<String, u32> {
    // TODO: for each tainted node, parse through and get all things that call it. then increment each of their badnesses by 1.
    unimplemented!();
    // HashMap::new();
}

fn do_output(_badness: HashMap<String, u32>) {
    // TODO: output badness, sorted by... badness... lol
    unimplemented!();
}

/*
echo "matching unsafe deps with callgraph nodes"
python3 "$SIDEROPHILE_PATH/script/find_unsafe_nodes.py" \
    "$SIDEROPHILE_OUT/unmangled_callgraph.dot" \
    "$SIDEROPHILE_OUT/unsafe_deps.txt" \
    > "$SIDEROPHILE_OUT/nodes_to_taint.txt"

echo "tracing the unsafety up the tree"
python3 "$SIDEROPHILE_PATH/script/trace_unsafety.py" \
    "$SIDEROPHILE_OUT/unmangled_callgraph.dot" \
    "$SIDEROPHILE_OUT/nodes_to_taint.txt" \
    "$CRATENAME" \
    > "$SIDEROPHILE_OUT/badness.txt"
*/
pub fn real_main(args: &MatchArgs) -> CliResult {
    let callgraph = parse_input_data(&args.input_callgraph, &args.input_unsafe_deps_file);
    let badness = trace_unsafety(callgraph, &args.crate_name);
    do_output(badness);
    Ok(())
}
