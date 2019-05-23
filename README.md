# Siderophile

This suite of tools identifies unsafe functions, expressions, trait methods, etc. in the crate's dependencies, and traces their usage up the call graph to find a caller in the crate. Its purpose is to aid in finding potentially fruitful fuzzing targets in a given crate. We call the tool "siderophile" because it eats things that are not Rusty.

## How it works

This tool primarily extends `cargo-geiger`, whose goal is to find unsafety at the crate-level. `siderophile` will first find all the sources of the current crate, find every Rust file in the sources, and parse each file individually using the `syn` crate. Each file is recursively combed through for unsafety occurring in functions, trait declarations, trait implementations, and submodules. `siderophile` will output the path of these objects, along with an indication of what type of syntactic block they were found in.

The list received from this step contains every unsafe block in every dependency of the crate, regardless of whether it's used. To narrow this down, we need to compare `siderophile`'s list to nodes in the callgraph of the crate. The callgraph is created by having `cargo` output the crate's bitcode, and using the `llvm-opt` analysis printer to produce a graph where each node is a name-mangled block. To unmangle the graph labels, `rustfilt` is run over the graph file, which will replace every name-mangled string with its unmangled counterpart.

With the callgraph in hand, we see which elements from `siderophile` are actually executed from the crate in question. Currently, this step is done manually, and it can take several hours for a large crate, but better more accurate output from `siderophile` could make this significantly more automatable. The labels of the nodes that are found to be unsafe are copied into a separate file that will be used as input for the final step.

The final step of this process is to trace these unsafe nodes in the callgraph. The `trace_unsafety.py` script loads the callgraph, the list of tainted nodes, and the current crate name and processes the list of tainted nodes one-by-one. For each node in the list, the script will find every upstream node in the callgraph, and increment their "badness" by one, thus indicating that they use unsafety at some point in their execution. At the end of this process, all the nodes with nonzero badness are printed out, sorted in descending order by badness.

## How to use

1. Compile `siderophile` in release mode and copy `target/release/siderophile` to the crate you'd like to perform this analysis on.

2. Install `rustfilt` with `cargo install rustfilt`. Make sure cargo's `bin` folder (most likely in `~/.cargo/bin`) is in your path.

3. `cd` into the crate you would like to perform this analysis on.

4. Execute `./siderophile -o siderophile_out.txt` (for help, run `siderophile --help`)

5. To create a single bitcode file with minimal optimization, execute the below command. An LLVM bitcode file produced like this can be readily turned into a callgraph.
```
env RUSTFLAGS="-C lto=no -C opt-level=0 -C debuginfo=2 -C codegen-units=16 -C inline-threshold=9999 --emit=llvm-bc" CARGO_INCREMENTAL="0" cargo rustc --lib -- --emit=llvm-bc
```

6. To create the callgraph, execute `LLVM_BIN/opt -dot-callgraph ./target/debug/deps/CRATENAME-xxxxxxxxxxxxxxxx.bc` where `LLVM_BIN` is the path to the `bin/` directory of your local LLVM installation, `CRATENAME` is the name of the current crate (e.g., "molasses"), and the x's are some sequence of hexadecimal digits. This command should produce a `callgraph.out` file in the current directory.

7. To unmangle the callgraph, run `rustfilt -i callgraph.dot -o unmangled_callgraph.dot`

8. Create a Python 3 virtualenv called `trace_unsafety` and copy in `script/trace_unsafety_requirements.txt`, `script/trace_unsafety.py`, and `script/find_unsafe_nodes.py`. Also copy `unmangled_callgraph.dot` and `siderophile_out.txt` into the folder.

9. `cd` into `trace_unsafety/` and activate the environment (that's `source bin/activate.sh` for bash, and `. bin/activate.fish` for fish). Then run `pip install -r trace_unsafety_requirements.txt` to install the necessary dependencies.

10. The last thing you need is to create a `nodes_to_taint.txt` file. Each line in this file should contain a label from `unmangled_callgraph.dot` (without the wrapping braces) of a node that is believed to be `unsafe`. You can fill this file by running `python3 find_unsafe_nodes.py unmangled_callgraph.dot siderophile_out.txt > nodes_to_taint.txt`. You can further edit the file by hand or comment out lines by prepending `#` to the line.

11. Once `nodes_to_taint.txt` is filled to the user's contentment, run `python3 trace_unsafety.py unmangled_callgraph.dot nodes_to_taint.txt CRATENAME > badness.txt` where `CRATENAME` is the same as the above `CRATENAME`. This `badness.txt` contains all the nodes occurring in `CRATENAME` that use unsafety in their execution, along with their "badness" score.

## Debugging

To get debugging output from `siderophile`, set the `RUST_LOG` environment variable to `siderophile=XXX` where `XXX` can be `info`, `debug`, or `trace`.

To get debugging output from `trace_unsafety.py` set the `LOGLEVEL` environment variable to `INFO` or `DEBUG`.

To get debugging output from `find_unsafe_nodes.py`, add some print statements somewhere, I don't know.

## Sample Data

Samples of an unmangled callgraph, a list of nodes to taint, output from `siderophile`, and output from `trace_unsafety.py` can all be found in `samples/`.

## Thanks

To [`cargo-geiger`](https://github.com/anderejd/cargo-geiger) and [`rust-praezi`](https://github.com/praezi/rust/) for current best practices. This project is mostly due to their work.
