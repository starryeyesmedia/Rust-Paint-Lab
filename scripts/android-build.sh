#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/apps/canvas-demo/Cargo.toml"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required (the repository pins Rust 1.90.0)." >&2
    exit 1
fi
if ! cargo apk version >/dev/null 2>&1; then
    echo "Install cargo-apk 0.10.0: cargo install cargo-apk --version 0.10.0 --locked" >&2
    exit 1
fi
if [[ -z "${ANDROID_NDK_HOME:-}${ANDROID_NDK_ROOT:-}" ]]; then
    echo "Set ANDROID_NDK_HOME or ANDROID_NDK_ROOT to an installed Android NDK." >&2
    exit 1
fi
if [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
    echo "Commit all source changes before building so APK provenance is exact." >&2
    exit 1
fi

cd "$repo_root"
cargo apk build \
    --manifest-path "$manifest" \
    --lib \
    --target aarch64-linux-android \
    --locked

apk_path="$(find "$repo_root/target" -type f -path '*/apk/bitchpixel.apk' -print -quit)"
if [[ -z "$apk_path" ]]; then
    echo "cargo-apk completed without producing target/**/apk/bitchpixel.apk." >&2
    exit 1
fi

printf 'Source commit: %s\n' "$(git -C "$repo_root" rev-parse HEAD)"
printf 'Debug APK: %s\n' "$apk_path"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$apk_path"
else
    shasum -a 256 "$apk_path"
fi
