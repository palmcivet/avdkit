#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
TARGET="$ROOT/target"
GENERATED="$TARGET/generated"
HEADERS="$TARGET/validation-headers"
XCFRAMEWORK="$TARGET/AvdkitValidation.xcframework"
LIBRARY="$TARGET/release/libavdkit_uniffi_validation.a"
STRIPPED_LIBRARY="$TARGET/libavdkit_uniffi_validation.stripped.a"

if [ -z "${DEVELOPER_DIR:-}" ] && [ -d /Applications/Xcode.app/Contents/Developer ]; then
    DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
    export DEVELOPER_DIR
fi

cd "$ROOT"
export CARGO_TARGET_DIR="$TARGET"

# The binding generator is a feature-gated tool. Rebuilding the library
# without that feature keeps generator-only dependencies out of the artifact.
cargo build --release --features bindgen --bin uniffi-bindgen
cargo build --release --lib

rm -rf "$GENERATED" "$HEADERS" "$XCFRAMEWORK" "$XCFRAMEWORK.zip"
mkdir -p "$GENERATED" "$HEADERS"

CARGO_NET_OFFLINE=true "$TARGET/release/uniffi-bindgen" generate \
    --library \
    --metadata-no-deps \
    --crate avdkit_uniffi_validation \
    --language swift \
    --out-dir "$GENERATED" \
    "$TARGET/release/libavdkit_uniffi_validation.dylib"

cp "$LIBRARY" "$STRIPPED_LIBRARY"
xcrun strip -S -x "$STRIPPED_LIBRARY"

xcrun swiftc \
    -swift-version 6 \
    -strict-concurrency=complete \
    -warnings-as-errors \
    -parse-as-library \
    "$GENERATED/avdkit_uniffi_validation.swift" \
    "$ROOT/swift/CancellationValidation.swift" \
    -Xcc "-fmodule-map-file=$GENERATED/avdkit_uniffi_validationFFI.modulemap" \
    -I "$GENERATED" \
    "$STRIPPED_LIBRARY" \
    -Xlinker -dead_strip \
    -o "$TARGET/cancellation-validation"

"$TARGET/cancellation-validation"

cp "$GENERATED/avdkit_uniffi_validationFFI.h" "$HEADERS/"
cp "$GENERATED/avdkit_uniffi_validationFFI.modulemap" "$HEADERS/module.modulemap"
xcodebuild -create-xcframework \
    -library "$STRIPPED_LIBRARY" \
    -headers "$HEADERS" \
    -output "$XCFRAMEWORK"
ditto -c -k --sequesterRsrc --keepParent "$XCFRAMEWORK" "$XCFRAMEWORK.zip"

printf 'rust_static_archive_bytes=%s\n' "$(stat -f %z "$LIBRARY")"
printf 'rust_static_stripped_bytes=%s\n' "$(stat -f %z "$STRIPPED_LIBRARY")"
printf 'xcframework_disk_kib=%s\n' "$(du -sk "$XCFRAMEWORK" | awk '{print $1}')"
printf 'xcframework_zip_bytes=%s\n' "$(stat -f %z "$XCFRAMEWORK.zip")"
