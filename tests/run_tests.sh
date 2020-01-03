#!/bin/bash

set -e

TESTS=(inlining)

for testdir in $TESTS; do
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
