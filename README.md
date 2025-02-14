# Siderophile

[![CI](https://github.com/trailofbits/siderophile/actions/workflows/ci.yml/badge.svg)](https://github.com/trailofbits/siderophile/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/siderophile)](https://crates.io/crates/siderophile)

Siderophile finds the "most unsafe" functions in your Rust codebase, so you can
fuzz them or refactor them out entirely.

It checks the callgraph of each function in the codebase, estimates how many
`unsafe` expressions are called in an evalutation of that function, then
produces a list sorted by this value. Here's what Siderophile's output format
looks like:

```
Badness  Function
    092  <myProject::myThing as my_project::myThing>::tempt_fate
    064  <myProject::myOtherThing::whatever as my_project::myThing>::defy_death
    [...]
```

"Badness" of a function is simply an approximation of how many unsafe
expressions are evaluated during an evaluation of that function. For instance,
marking unsafe functions with a `*`, suppose your function `f` calls functions
`g*` and `h`. Furthermore, `h` calls `i*`. Then the badness of `f` is 2.
Functions with high badness have a lot of opportunities to be memory unsafe.

## Installation

Siderophile is [available via crates.io](https://crates.io/crates/siderophile),
and can be installed with `cargo`:

```console
cargo install siderophile
```

When you run that step, you *may* see an error from the `llvm-sys` crate:

```console
error: No suitable version of LLVM was found system-wide or pointed
              to by LLVM_SYS_190_PREFIX.

              Consider using `llvmenv` to compile an appropriate copy of LLVM, and
              refer to the llvm-sys documentation for more information.

              llvm-sys: https://crates.io/crates/llvm-sys
              llvmenv: https://crates.io/crates/llvmenv

error: could not compile `llvm-sys` due to previous error
```

This indicates that the build was unable to automatically find a copy of LLVM to link against.

You can fix it by setting the `LLVM_SYS_190_PREFIX`. For example, for macOS with LLVM via
Homebrew, you might do:

```console
LLVM_SYS_190_PREFIX=$(brew --prefix)/opt/llvm@19/ cargo install siderophile
```

You _may_ run into other linker errors as well, e.g.:

```
  = note: ld: library not found for -lzstd
          clang: error: linker command failed with exit code 1 (use -v to see invocation)
```

You can fix this by setting the `LIBRARY_PATH`. For example, for macOS with
Homebrew:

```console
LIBRARY_PATH=$(brew --prefix)/lib cargo install siderophile
```

To tie it all together:

```console
LIBRARY_PATH=$(brew --prefix)/lib \
    LLVM_SYS_190_PREFIX=$(brew --prefix)/opt/llvm@19/
    cargo install siderophile
```

### Building and installing from source

Alternatively, if you'd like to build from source:

```console
git clone https://github.com/trailofbits/siderophile && cd siderophile

# TIP: include --release for a release build
cargo build

# optionally: install the built binary to cargo's default bin path
cargo install --path .
```

You may need the same `LLVM_SYS_190_PATH` and `LIBRARY_PATH` overrides
mentioned above.

## How to use

Make sure that you followed the above steps, then do the following:

1. `cd` to the root directory of the crate you want to analyze

2. Run `siderophile --crate-name CRATENAME`,
   where `CRATENAME` is the name of the crate you want to analyze

Functions are written to `stdout`, ordered by their badness.

## How it works

Siderophile extends `cargo-geiger`, whose goal is to find unsafety at the crate-level.

First, the callgraph is created by having `cargo` output the crate's bitcode,
then parsing it to produce a callgraph and demangle the names into things that
we can match with the source code.

Next, Siderophile finds all the sources of the current crate, finds every
Rust file in the sources, and parses each file individually using the `syn`
crate. Each file is recursively combed through for unsafety occurring in
functions, trait declarations, trait implementations, and submodules.
Siderophile will output the path of these objects, along with an indication
of what type of syntactic block they were found in. The list received from this
step contains every unsafe block in every dependency of the crate, regardless
of whether it's used. To narrow this down, Siderophile needs to compare its
list to nodes in the callgraph of the crate.

Using the callgraph produced in the first step, Siderophile checks which elements from the
output are actually executed from the crate in question. This
step (implemented in `src/callgraph_matching`) is not guaranteed to find
everything, but it has shown good results against manual search. It is also not
immune to false positives, although none have been found yet. The labels of the
nodes that are found to be unsafe are used as input for the final step.

The final step is to trace these unsafe nodes in the callgraph. For each node
in the list, Siderophile will find every upstream node in the callgraph, and
increment their badness by one, thus indicating that they use unsafety at some
point in their execution. At the end of this process, all the nodes with nonzero
badness are printed out, sorted in descending order by badness.

## Limitations

Siderophile is _not_ guaranteed to catch all the unsafety in a crate's deps.

Since things are only tagged at a source-level, Siderophile does not have the ability to
inspect macros or resolve dynamically dispatched methods. Accordingly, this tool
should not be used to "prove" that a crate contains no unsafety.

## Debugging

To get debugging output from `siderophile`, set the `RUST_LOG` environment
variable to `siderophile=XXX` where `XXX` can be `info`, `debug`, or `trace`.

## Thanks

To [`cargo-geiger`](https://github.com/anderejd/cargo-geiger) and
[`rust-praezi`](https://github.com/praezi/rust/) for current best practices.
This project is mostly due to their work.

## License

Siderophile is licensed and distributed under the AGPLv3 license.

[Contact us](opensource@trailofbits.com) if you're looking for an exception to
the terms.
