#!/bin/bash

set -e

TESTS=(inlining librarycrate)

echo "[!!!] Tests that will be run (space-delimited): ${TESTS[*]}"
echo ""

for testdir in $TESTS; do
    echo "[@@@] Going to run '$testdir' test"
    echo ""
    pushd $testdir
    ../../analyze.sh $testdir
    if ! (diff ./expected_badness.txt ./siderophile_out/badness.txt); then
        echo ""
        echo "[!!!] Tests failed on $testdir: the expected_badness.txt does not match the siderophile_out/badness.txt file!"
        exit 1
    fi
    popd
done

echo ""
echo "[+++] Tests succeeded!"
