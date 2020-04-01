#!/bin/bash

TESTS=(inlining librarycrate crate-uses-rust-toolchain)

# https://stackoverflow.com/questions/5947742/how-to-change-the-output-color-of-printf-in-linux
INFO='\033[1;33m'   # Yellow
OK='\033[0;32m'     # Green
WARN='\033[0;31m'   # Red
NC='\033[0m'        # No Color
printf "$WARN[!!!] crate-uses-rust-toolchain's expected_badness.txt may be wrong. the test was broken and i just set it to use siderophile's output when i started developing!$NC\n"
printf "$INFO[!!!] Tests that will be run (space-delimited): ${TESTS[*]}$NC\n"
echo ""

for testdir in ${TESTS[@]}; do
    printf "$INFO[@@@] Going to run '$testdir' test$NC\n"
    echo ""
    pushd $testdir
    ../../analyze.sh $testdir
    if ! (diff ./expected_badness.txt ./siderophile_out/badness.txt); then
        echo ""
        printf "$WARN[!!!] Tests failed on $testdir: the expected_badness.txt does not match the siderophile_out/badness.txt file!$NC\n"
        exit 1
    fi
    popd
done

echo ""
printf "$OK[+++] Tests succeeded!$NC\n"
