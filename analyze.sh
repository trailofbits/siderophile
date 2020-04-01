#!/bin/bash

# -u ensures that referencing unset variables is an error
# -e ensures that the script dies if a command fails with a nonzero error code
set -ue

function usage() {
    echo "USAGE: $(basename "$0") CRATENAME"
}

function reqs() {
    echo "See siderophile's README for detailed requirements"
}

if !(hash "cargo") 2>/dev/null; then
    echo "siderophile requires cargo, which doesn't seem to be installed"
    reqs
    exit 1
fi

if !(hash "opt") 2>/dev/null; then
    echo "siderophile requires LLVM utilities, which don't seem to be installed"
    reqs
    exit 1
fi

# This script takes precisely 1 argument
if [ "$#" -ne 1 ]; then
    usage
    exit 1
fi

# The name of the crate we're analyzing
CRATENAME=$1
CRATENAME=${CRATENAME//-/_}

# The folder that this bash file is in
SIDEROPHILE_PATH=$(dirname "$0")

# The folder we output all the files into
SIDEROPHILE_OUT="siderophile_out"

# We do it to handle external crates that use `rust-toolchain` file
# see https://github.com/trailofbits/siderophile/issues/14 for more information
RUSTUP_DEFAULT_VERSION="$(rustup default | sed -e 's/ (default)//')"

# Where to look for `rustfilt`. If CARGO_HOME is set, use $CARGO_HOME/bin.
# Otherwise, use ~/.cargo/bin
CARGO_BIN=${CARGO_HOME:-~/.cargo}/bin

if !(test -x "$SIDEROPHILE_PATH/target/release/siderophile") 2>/dev/null; then
    echo "couldn't find siderophile binary, did you run setup.sh?"
    exit 1
fi

# All auxiliary files go here
mkdir -p $SIDEROPHILE_OUT

echo "trawling source code of dependencies for unsafety"
RUSTUP_TOOLCHAIN=$RUSTUP_DEFAULT_VERSION "$SIDEROPHILE_PATH/target/release/siderophile" trawl -o "$SIDEROPHILE_OUT/unsafe_deps.txt"

echo "generating LLVM bitcode for the callgraph"
cargo clean
RUSTFLAGS="-C lto=no -C opt-level=0 -C debuginfo=2 --emit=llvm-bc" \
CARGO_INCREMENTAL="0" \
cargo rustc -- --emit=llvm-bc

echo "generating callgraph"

# If we're in a crate in a workspace, check the directory above for the compiler output
if (test -e ./target/debug/deps/$CRATENAME-*.bc) 2>/dev/null; then
    opt -dot-callgraph ./target/debug/deps/$CRATENAME-*.bc
elif (test -e ../target/debug/deps/$CRATENAME-*.bc) 2>/dev/null; then
    opt -dot-callgraph ../target/debug/deps/$CRATENAME-*.bc
else
    echo "Cannot find LLVM bitcode for \"$CRATENAME\" in ./target/debug/deps or ../target/debug/deps"
    exit 1
fi

# This outputs to ./callgraph.dot no matter what. Move it
mv ./callgraph.dot "$SIDEROPHILE_OUT/mangled_callgraph.dot"

echo "unmangling callgraph symbols"
rm -f "$SIDEROPHILE_OUT/unmangled_callgraph.dot"
$CARGO_BIN/rustfilt \
    -i "$SIDEROPHILE_OUT/mangled_callgraph.dot" \
    -o "$SIDEROPHILE_OUT/unmangled_callgraph.dot"

# This file is truly useless
rm "$SIDEROPHILE_OUT/mangled_callgraph.dot"

echo "matching unsafe deps with callgraph nodes and tracing the unsafety up the tree"
RUSTUP_TOOLCHAIN=$RUSTUP_DEFAULT_VERSION\
  "$SIDEROPHILE_PATH/target/release/siderophile" trace\
  --callgraph-file "$SIDEROPHILE_OUT/unmangled_callgraph.dot"\
  --unsafe-deps-file "$SIDEROPHILE_OUT/unsafe_deps.txt"\
  --crate-name "$CRATENAME"\
  > "$SIDEROPHILE_OUT/badness.txt"

echo "done. see $SIDEROPHILE_OUT/badness.txt"
