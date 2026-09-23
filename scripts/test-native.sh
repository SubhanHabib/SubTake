#!/bin/sh
# Builds native/*.swift with the tests in native/tests and runs them.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
xcrun swiftc -parse-as-library -swift-version 5 -module-name SubTakeMacOS \
    -target "$(uname -m)-apple-macos14.0" \
    "$root"/native/*.swift "$root"/native/tests/*.swift -o "$out/native-tests"
"$out/native-tests"
