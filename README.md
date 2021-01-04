# Siderophile

Siderophile finds the "most unsafe" functions in your Rust codebase, so you can fuzz them or refactor them out entirely. It checks the callgraph of each function in the codebase, estimates how many `unsafe` expressions are called in an evalutation of that function, then produces a list sorted by this value. Here's what Siderophile's output format looks like:

```
Badness  Function
    092  <myProject::myThing as my_project::myThing>::tempt_fate
    064  <myProject::myOtherThing::whatever as my_project::myThing>::defy_death
    [...]
```

"Badness" of a function is simply an approximation of how many unsafe expressions are evaluated during an evaluation of that function. For instance, marking unsafe functions with a `*`, suppose your function `f` calls functions `g*` and `h`. Furthermore, `h` calls `i*`. Then the badness of `f` is 2. Functions with high badness have a lot of opportunities to be memory unsafe.

## Installation

Make sure that you have the following requirements:

  * `rustup` and `cargo` must be installed and in your `PATH`
  * LLVM 11 is required. Older versions such as LLVM 8, 9 or 10 may work (see https://crates.io/crates/llvm-ir) but require the `llvm-ir` package's features change in `Cargo.toml` before compiling Siderophile.
  
Then, run `cargo build --release`, and you'll have a Siderophile binary :)

## How to use

Make sure that you followed the above steps, then do the following:

1. `cd` to the root directory of the crate you want to analyze

2. Run `SIDEROPHILE_LOCATION/target/release/siderophile --crate-name CRATENAME`, where `CRATENAME` is the name of the crate you want to analyze, and `SIDEROPHILE_LOCATION` is the location where you put the siderophile code (you know, normal running-rust-binary stuff).

Functions are written to stdout, ordered by their badness. 

## How it works

Siderophile extends `cargo-geiger`, whose goal is to find unsafety at the crate-level. 

First, the callgraph is created by having `cargo` output the crate's bitcode, then parsing it to produce a callgraph and demangle the names into things that we can match with the source code.

Next, `siderophile` finds all the sources of the current crate, finds every Rust file in the sources, and parses each file individually using the `syn` crate. Each file is recursively combed through for unsafety occurring in functions, trait declarations, trait implementations, and submodules. `siderophile` will output the path of these objects, along with an indication of what type of syntactic block they were found in. The list received from this step contains every unsafe block in every dependency of the crate, regardless of whether it's used. To narrow this down, we need to compare `siderophile`'s list to nodes in the callgraph of the crate.

Using the callgraph produced in the first step, we check which elements from the `siderophile` output are actually executed from the crate in question. This step (implemented in `src/callgraph_matching`) is not guaranteed to find everything, but it has shown good results against manual search. It is also not immune to false positives, although none have been found yet. The labels of the nodes that are found to be unsafe are used as input for the final step.

The final step is to trace these unsafe nodes in the callgraph. For each node in the list, `siderophile` will find every upstream node in the callgraph, and increment their badness by one, thus indicating that they use unsafety at some point in their execution. At the end of this process, all the nodes with nonzero badness are printed out, sorted in descending order by badness.

## Limitations

Siderophile is _not_ guaranteed to catch all the unsafety in a crate's deps. Since things are only tagged at a source-level, we do not have the ability to inspect macros or resolve dynamically dispatched methods. Accordingly, this tool should not be used to "prove" that a crate uses no unsafety.

## Debugging

To get debugging output from `siderophile`, set the `RUST_LOG` environment variable to `siderophile=XXX` where `XXX` can be `info`, `debug`, or `trace`.

## Thanks

To [`cargo-geiger`](https://github.com/anderejd/cargo-geiger) and [`rust-praezi`](https://github.com/praezi/rust/) for current best practices. This project is mostly due to their work.

# License

Siderophile is licensed and distributed under the AGPLv3 license. [Contact us](opensource@trailofbits.com) if you're looking for an exception to the terms.

