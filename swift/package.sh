#!/bin/sh
set -eu

if [ "$#" -ne 1 ] || [ -z "$1" ]; then
    echo "usage: swift/package.sh <xcframework-zip-url>" >&2
    exit 1
fi

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
URL=$1
TARGET="$ROOT/target"
XCFRAMEWORK="$ROOT/swift/Artifacts/AvdkitFFI.xcframework"
GENERATED="$ROOT/swift/Sources/Avdkit/Generated.swift"
CONVENIENCE="$ROOT/swift/Sources/Avdkit/Operation+AsyncSequence.swift"
DIST="$TARGET/swift-package"
ZIP="$TARGET/AvdkitFFI.xcframework.zip"

# shellcheck source=xcode.sh
. "$ROOT/swift/xcode.sh"

if [ ! -d "$XCFRAMEWORK" ] || [ ! -f "$GENERATED" ] || [ ! -f "$CONVENIENCE" ]; then
    echo "missing Swift build output; run swift/build.sh first" >&2
    exit 1
fi

mkdir -p "$TARGET"
ditto -c -k --keepParent "$XCFRAMEWORK" "$ZIP"
CHECKSUM=$(cd "$ROOT/swift" && swift package compute-checksum "$ZIP")
case "$CHECKSUM" in
    *[!0-9a-fA-F]* | "")
        echo "unexpected checksum: $CHECKSUM" >&2
        exit 1
        ;;
esac
printf '%s\n' "$CHECKSUM" > "$ZIP.sha256"

ESCAPED=$(printf '%s' "$URL" | sed 's/\\/\\\\/g; s/"/\\"/g')

rm -rf "$DIST"
mkdir -p "$DIST/Sources/Avdkit"
cp "$GENERATED" "$DIST/Sources/Avdkit/Generated.swift"
cp "$CONVENIENCE" "$DIST/Sources/Avdkit/Operation+AsyncSequence.swift"

cat > "$DIST/Package.swift" <<EOF
// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Avdkit",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "Avdkit", targets: ["Avdkit"]),
    ],
    targets: [
        .binaryTarget(
            name: "AvdkitFFI",
            url: "$ESCAPED",
            checksum: "$CHECKSUM"
        ),
        .target(
            name: "Avdkit",
            dependencies: ["AvdkitFFI"],
            path: "Sources/Avdkit"
        ),
    ]
)
EOF

FILES=$(find "$DIST" -type f | sed "s|^$DIST/||" | sort)
EXPECTED=$(printf '%s\n' \
    "Package.swift" \
    "Sources/Avdkit/Generated.swift" \
    "Sources/Avdkit/Operation+AsyncSequence.swift")
if [ "$FILES" != "$EXPECTED" ]; then
    echo "swift package contains unexpected files:" >&2
    echo "$FILES" >&2
    exit 1
fi

echo "$ZIP"
echo "$CHECKSUM"
echo "$DIST"
