#!/usr/bin/env bash
set -euo pipefail

# Produces the runtime and TUN engine for every Flutter Android ABI.

root=$(cd "$(dirname "$0")/.." && pwd)
ndk=${ANDROID_NDK_HOME:-}
if [[ -z "$ndk" ]]; then
  ndk=$(find "$HOME/Android/Sdk/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
fi
if [[ -z "$ndk" || ! -d "$ndk" ]]; then
  echo "Android NDK not found; set ANDROID_NDK_HOME." >&2
  exit 1
fi

toolchain="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
api=${ANDROID_API_LEVEL:-24}
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$toolchain/aarch64-linux-android${api}-clang"
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$toolchain/armv7a-linux-androideabi${api}-clang"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$toolchain/x86_64-linux-android${api}-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CC_armv7_linux_androideabi="$CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER"
export CC_x86_64_linux_android="$CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER"
export AR_aarch64_linux_android="$toolchain/llvm-ar"
export AR_armv7_linux_androideabi="$toolchain/llvm-ar"
export AR_x86_64_linux_android="$toolchain/llvm-ar"
cd "$root"
for target_and_abi in \
  "aarch64-linux-android arm64-v8a" \
  "armv7-linux-androideabi armeabi-v7a" \
  "x86_64-linux-android x86_64"
do
  read -r target abi <<<"$target_and_abi"
  cargo build --release --target "$target" -p velum-client-ffi
  install -Dm755 "target/$target/release/libvelum_client_ffi.so" \
    "apps/velum-client/flutter/android/app/src/main/jniLibs/$abi/libvelum_client_ffi.so"
done
