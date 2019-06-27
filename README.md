# Siderophile

Siderophile finds the "most unsafe" functions in your Rust codebase, so you can fuzz them or refactor them out entirely. It checks the callgraph of each function in the codebase, estimates how many `unsafe` expressions are called in an evalutation of that function, then produces a list sorted by this value. Here is sample output from Siderophile:

```
Badness  Function
    092  <myProject::myThing as my_project::myThing>::tempt_fate
    064  <myProject::myOtherThing::whatever as my_project::myThing>::defy_death
    [...]
```

"Badness" of a function is simply an approximation of how many unsafe expressions are evaluated during an evaluation of that function. For instance, marking unsafe functions with a `*`, suppose your function `f` calls functions `g*` and `h`. Furthermore, `h` calls `i*`. Then the badness of `f` is 2. Functions with high badness have a lot of opportunities to be memory unsafe.

## Installation

Make sure that you have the following requirements:

  * LLVM must be installed and its `bin` directory must be in your `PATH` (this is because we use the `opt` utility)
  * Python 3 must be installed and in your `PATH`
  * `cargo` must be installed and in your `PATH`

Then, simply run `./setup.sh` in this root directory. That's it! This will `cargo install rustfilt` if `rustfilt` isn't already in your `PATH` and compile Siderophile.

## How to use

Make sure that you followed the above steps, then do the following:

1. `cd` to the root directory of the crate you want to analyze

2. Run `PATH_TO_SIDEROPHILE_ROOT/analyze.sh CRATENAME`, where `CRATENAME` is the name of the crate you want to analyze

Functions are written to `./siderophile_out/badness.txt`, ordered by their badness. Auxiliary files are also put in `siderophile_out`, namely:

* `unmangled_callgraph.dot` - The crate's callgraph, complete with all the Rusty symbols
* `unsafe_deps.txt` - A list of all the unsafe expressions, methods, functions, and closures found in the dependencies of the create. The items are written in (an attempted) fully-qualified form.
* `nodes_to_taint.txt` - A list of nodes in the callgraph that we want to mark as unsafe

Examples of `unmangled_callgraph.dot`, `unsafe_deps.txt`, `nodes_to_taint.txt`, and `badness.txt` can all be found in the [`samples/`](samples/) directory of this repo. These sample files are all from the same analysis pass on actix-web.

### With Tweaks

If you want to rerun the analysis with a different set of tainted nodes, then:

1. `cd` into `siderophile_out`
2. Modify `nodes_to_taint.txt` to your heart's content
3. Run `python3 PATH_TO_SIDEROPHILE_ROOT/script/trace_unsafety.py unmangled_callgraph.dot nodes_to_taint.txt > badness.txt`.


## How it works

Siderophile extends `cargo-geiger`, whose goal is to find unsafety at the crate-level. `siderophile` finds all the sources of the current crate, finds every Rust file in the sources, and parses each file individually using the `syn` crate. Each file is recursively combed through for unsafety occurring in functions, trait declarations, trait implementations, and submodules. `siderophile` will output the path of these objects, along with an indication of what type of syntactic block they were found in.

The list received from this step contains every unsafe block in every dependency of the crate, regardless of whether it's used. To narrow this down, we need to compare `siderophile`'s list to nodes in the callgraph of the crate. The callgraph is created by having `cargo` output the crate's bitcode, and using the `llvm-opt` analysis printer to produce a graph where each node is a name-mangled block. To unmangle the graph labels, `rustfilt` is run over the graph file, which will replace every name-mangled string with its unmangled counterpart.

With the callgraph in hand, we see which elements from the `siderophile` output are actually executed from the crate in question. This is done with the `find_unsafe_nodes.py` script. The script is not guaranteed to find everything, but it has shown good results against manual search. It is also not immune to false positives, although none have been found yet. The labels of the nodes that are found to be unsafe are copied into a separate file that will be used as input for the final step.

The final step is to trace these unsafe nodes in the callgraph. The `trace_unsafety.py` script loads the callgraph, the list of tainted nodes, and the current crate name and processes the list of tainted nodes one-by-one. For each node in the list, the script will find every upstream node in the callgraph, and increment their badness by one, thus indicating that they use unsafety at some point in their execution. At the end of this process, all the nodes with nonzero badness are printed out, sorted in descending order by badness.

## Limitations

Siderophile is _not_ guaranteed to catch all the unsafety in a crate's deps. Since things are only tagged at a source-level, we do not have the ability to inspect macros or resolve dynamically dispatched methods. Accordingly, this tool should not be used to "prove" that a crate uses no unsafety.

## Debugging

To get debugging output from `siderophile`, set the `RUST_LOG` environment variable to `siderophile=XXX` where `XXX` can be `info`, `debug`, or `trace`.

To get debugging output from `trace_unsafety.py` set the `LOGLEVEL` environment variable to `INFO` or `DEBUG`.

To get debugging output from `find_unsafe_nodes.py`, add some print statements somewhere, I don't know man.

## Thanks

To [`cargo-geiger`](https://github.com/anderejd/cargo-geiger) and [`rust-praezi`](https://github.com/praezi/rust/) for current best practices. This project is mostly due to their work.
