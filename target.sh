#!/bin/bash

if [ -z "$1" ]; then
  echo "USAGE: ./target.sh CRATENAME"
  exit 1
fi

if [ -z "$SIDEROPHILE_PATH" ]; then
  echo "\$SIDEROPHILE_PATH is not defined, please set it to the location of the siderophile directory"
  exit 1
fi

if !(test -x $SIDEROPHILE_PATH/target/release/siderophile) 2>/dev/null; then
  echo "couldn't find siderophile binary, did you run setup.sh?"
  exit 1
fi

function require() {
  if !(hash $1) 2>/dev/null; then
    echo "siderophile requires $1, which doesn't seem to be installed"
    exit 1
  fi
}

require "cargo"
require "opt"
require "python3"

echo "running siderophile"
$SIDEROPHILE_PATH/target/release/siderophile -o siderophile_out.txt

echo "generating llvm bc"
env RUSTFLAGS="-C lto=no -C opt-level=0 -C debuginfo=2 -C inline-threshold=9999 --emit=llvm-bc" CARGO_INCREMENTAL="0" cargo rustc --lib -- --emit=llvm-bc

echo "generating callgraph"
opt -dot-callgraph ./target/debug/deps/$1-*.bc

echo "unmangling callgraph symbols"
rm unmangled_callgraph.dot
~/.cargo/bin/rustfilt -i callgraph.dot -o unmangled_callgraph.dot

echo "creating nodes_to_taint.txt"
python3 $SIDEROPHILE_PATH/script/find_unsafe_nodes.py unmangled_callgraph.dot siderophile_out.txt > nodes_to_taint.txt

echo "creating badness.txt"
python3 $SIDEROPHILE_PATH/script/trace_unsafety.py unmangled_callgraph.dot nodes_to_taint.txt $1 > badness.txt
