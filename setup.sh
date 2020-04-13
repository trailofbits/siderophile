#!/bin/bash

# -u ensures that referencing unset variables is an error
# -e ensures that the script dies if a command fails with a nonzero error code
set -ue

if !(hash "cargo") 2>/dev/null; then
  echo "siderophile requires cargo, which doesn't seem to be installed"
  exit 1
fi

## Cargo stuff
echo "building siderophile"
cargo build --release

echo "Done. Read README.md for further instructions"
