#!/usr/bin/env bash
set -euo pipefail

# Produces the arm64 runtime library consumed by Flutter and the Android host.
# TUN is deliberately not enabled by this script; it only packages the shared
# client ABI required by the later JNI packet-engine integration.

root=$(cd "$(dirname "$0")/.." && pwd)
ndk=${ANDROID_NDK_HOME:-}
if [[ -z "$ndk" ]]; then
  ndk=$(find "$HOME/Android/Sdk/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
fi
if [[ -z "$ndk" || ! -d "$ndk" ]]; then
  echo "Android NDK not found; set ANDROID_NDK_HOME." >&2
  exit 1
fi

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android${ANDROID_API_LEVEL:-24}-clang"
cd "$root"
cargo build --release --target aarch64-linux-android -p velum-client-ffi
install -Dm755 target/aarch64-linux-android/release/libvelum_client_ffi.so \
  apps/velum-client/flutter/android/app/src/main/jniLibs/arm64-v8a/libvelum_client_ffi.so
