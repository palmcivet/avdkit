#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TARGET="$ROOT/target"
TRIPLE="aarch64-apple-darwin"
RELEASE="$TARGET/$TRIPLE/release"
GENERATED="$TARGET/swift-generated"
HEADERS="$TARGET/swift-headers"
ARTIFACTS="$ROOT/swift/Artifacts"
XCFRAMEWORK="$ARTIFACTS/AvdkitFFI.xcframework"
LIBRARY="$RELEASE/libavdkit_ffi.a"
DYLIB="$RELEASE/libavdkit_ffi.dylib"
GENERATOR="$TARGET/release/swift-bindgen"

if [ -z "${DEVELOPER_DIR:-}" ] && [ -d /Applications/Xcode.app/Contents/Developer ]; then
    DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
    export DEVELOPER_DIR
fi

cd "$ROOT"
export CARGO_TARGET_DIR="$TARGET"
cargo build --manifest-path "$ROOT/tools/swift-bindgen/Cargo.toml" --release --locked
cargo build -p avdkit-ffi --release --target "$TRIPLE" --lib --locked

rm -rf "$GENERATED" "$HEADERS" "$XCFRAMEWORK"
mkdir -p "$GENERATED" "$HEADERS" "$ARTIFACTS"

CARGO_NET_OFFLINE=true "$GENERATOR" generate \
    --library \
    --metadata-no-deps \
    --crate avdkit_ffi \
    --language swift \
    --config "$ROOT/crates/ffi/uniffi.toml" \
    --out-dir "$GENERATED" \
    "$DYLIB"

cp "$GENERATED/AvdkitBindings.swift" \
    "$ROOT/swift/Sources/Avdkit/Generated.swift"
cp "$GENERATED/AvdkitFFI.h" "$HEADERS/"
cp "$GENERATED/AvdkitFFI.modulemap" "$HEADERS/module.modulemap"

xcrun strip -S -x "$LIBRARY"
xcodebuild -create-xcframework \
    -library "$LIBRARY" \
    -headers "$HEADERS" \
    -output "$XCFRAMEWORK"

ARCHS=$(xcrun lipo -archs "$LIBRARY")
if [ "$ARCHS" != "arm64" ]; then
    echo "expected arm64-only library, found: $ARCHS" >&2
    exit 1
fi

swift test --package-path "$ROOT/swift"

xcrun swiftc \
    -swift-version 6 \
    -strict-concurrency=complete \
    -warnings-as-errors \
    -parse-as-library \
    "$GENERATED/AvdkitBindings.swift" \
    "$ROOT/swift/Sources/Avdkit/Operation+AsyncSequence.swift" \
    "$ROOT/swift/Validation/CancellationValidation.swift" \
    -Xcc "-fmodule-map-file=$GENERATED/AvdkitFFI.modulemap" \
    -I "$GENERATED" \
    "$LIBRARY" \
    -Xlinker -dead_strip \
    -o "$TARGET/avdkit-cancellation-validation"

"$TARGET/avdkit-cancellation-validation"
