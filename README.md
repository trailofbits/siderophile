# Siderophile

This suite of tools identifies unsafe functions, expressions, trait methods, etc. in the crate's dependencies, and traces their usage up the call graph to find a caller in the crate. Its purpose is to aid in finding potentially fruitful fuzzing targets in a given crate. We call the tool "siderophile" because it eats things that are not Rusty.

## How it works

This tool primarily extends `cargo-geiger`, whose goal is to find unsafety at the crate-level. `siderophile` will first find all the sources of the current crate, find every Rust file in the sources, and parse each file individually using the `syn` crate. Each file is recursively combed through for unsafety occurring in functions, trait declarations, trait implementations, and submodules. `siderophile` will output the path of these objects, along with an indication of what type of syntactic block they were found in.

The list received from this step contains every unsafe block in every dependency of the crate, regardless of whether it's used. To narrow this down, we need to compare `siderophile`'s list to nodes in the callgraph of the crate. The callgraph is created by having `cargo` output the crate's bitcode, and using the `llvm-opt` analysis printer to produce a graph where each node is a name-mangled block. To unmangle the graph labels, `rustfilt` is run over the graph file, which will replace every name-mangled string with its unmangled counterpart.

With the callgraph in hand, we see which elements from the `siderophile` output are actually executed from the crate in question. This is done with the `find_unsafe_nodes.py` script. The script is not guaranteed to find everything, but it has shown good results against manual search. It is also not immune to false positives, although none have been found yet. The labels of the nodes that are found to be unsafe are copied into a separate file that will be used as input for the final step.

The final step of this process is to trace these unsafe nodes in the callgraph. The `trace_unsafety.py` script loads the callgraph, the list of tainted nodes, and the current crate name and processes the list of tainted nodes one-by-one. For each node in the list, the script will find every upstream node in the callgraph, and increment their "badness" by one, thus indicating that they use unsafety at some point in their execution. At the end of this process, all the nodes with nonzero badness are printed out, sorted in descending order by badness.

## How to use

1. Run `./setup.sh` in this directory

2. Run `SIDEROPHILE_PATH=$PATH_TO_THIS_DIRECTORY ./target.sh $CRATENAME` in the root directory of the crate you wish to target

3. Rankings are in a file called `badness.txt`.

If you want to rerun the analysis with a different set of tainted nodes, modify `nodes_to_taint.txt` in the same directory, then run `python3 $THIS_DIRECTORY/script/trace_unsafety.py unmangled_callgraph.dot nodes_to_taint.txt`.

## Debugging

To get debugging output from `siderophile`, set the `RUST_LOG` environment variable to `siderophile=XXX` where `XXX` can be `info`, `debug`, or `trace`.

To get debugging output from `trace_unsafety.py` set the `LOGLEVEL` environment variable to `INFO` or `DEBUG`.

To get debugging output from `find_unsafe_nodes.py`, add some print statements somewhere, I don't know.

## Sample Data

Samples of an unmangled callgraph, a list of nodes to taint, output from `siderophile`, and output from `trace_unsafety.py` can all be found in `samples/`.

## Thanks

To [`cargo-geiger`](https://github.com/anderejd/cargo-geiger) and [`rust-praezi`](https://github.com/praezi/rust/) for current best practices. This project is mostly due to their work.
