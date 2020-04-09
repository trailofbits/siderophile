#!/bin/bash

# TODO: handle path tomfoolery so that you can get rid of this file

# -u ensures that referencing unset variables is an error
# -e ensures that the script dies if a command fails with a nonzero error code
set -ue

function usage() {
    echo "USAGE: $(basename "$0") CRATENAME"
}

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

# We do it to handle external crates that use `rust-toolchain` file
# see https://github.com/trailofbits/siderophile/issues/14 for more information
RUSTUP_DEFAULT_VERSION="$(rustup default | sed -e 's/ (default)//')"

# echo "trawling source code of dependencies for unsafety, unmangling callgraph, matching unsafe deps with callgraph nodes, and tracing the unsafety up the tree"
RUSTUP_TOOLCHAIN=$RUSTUP_DEFAULT_VERSION\
  "$SIDEROPHILE_PATH/target/release/siderophile"\
  --crate-name "$CRATENAME"\
  > "badness.txt"
