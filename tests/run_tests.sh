#!/bin/bash

set -eo pipefail

TESTS=(inlining librarycrate crate-uses-rust-toolchain)

# https://stackoverflow.com/questions/5947742/how-to-change-the-output-color-of-printf-in-linux
INFO='\033[1;33m'   # Yellow
OK='\033[0;32m'     # Green
WARN='\033[0;31m'   # Red
NC='\033[0m'        # No Color

echo -e "${INFO}[!!!] Tests that will be run (space-delimited): ${TESTS[*]}${NC}"
echo -e ""

for testdir in "${TESTS[@]}"; do
    echo -e "${INFO}[@@@] Going to run '${testdir}' test${NC}"
    echo ""
    pushd "${testdir}"
    rm -f output_badness.txt
    ../../target/release/siderophile --crate-name "${testdir}" > output_badness.txt
    if ! (diff ./expected_badness.txt ./output_badness.txt); then
        echo ""
        echo -e "${WARN}[!!!] Tests failed on $testdir: the expected_badness.txt does not match the output_badness.txt file!${NC}"
        exit 1
    fi
    popd
done

echo ""
echo -e "${OK}[+++] Tests succeeded!${NC}"
