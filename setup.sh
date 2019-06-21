#!/bin/bash

if !(hash "cargo") 2>/dev/null; then
  echo "siderophile requires $1, which doesn't seem to be installed"
  exit 1
fi

## Cargo stuff
echo "building siderophile"
cargo build --release

if !(hash rustfilt) 2>/dev/null; then
  echo "didn't find rustfilt, installing it now"
  cargo install rustfilt
fi
