#!/usr/bin/env sh
# Install a pinned Lego ACME client into the current user's Velum data directory.
set -eu

version='5.2.2'
install_dir="${XDG_DATA_HOME:-$HOME/.local/share}/velum/tools/lego/v${version}"

case "$(uname -s):$(uname -m)" in
    Linux:x86_64) platform='linux_amd64' ;;
    Linux:aarch64|Linux:arm64) platform='linux_arm64' ;;
    Darwin:x86_64) platform='darwin_amd64' ;;
    Darwin:arm64) platform='darwin_arm64' ;;
    *) echo "error: unsupported platform $(uname -s):$(uname -m)" >&2; exit 1 ;;
esac

archive="lego_v${version}_${platform}.tar.gz"
base="https://github.com/go-acme/lego/releases/download/v${version}"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT HUP INT TERM

download() {
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error "$1" --output "$2"
    else
        wget --quiet --output-document="$2" "$1"
    fi
}

download "$base/lego_${version}_checksums.txt" "$temporary/checksums"
download "$base/$archive" "$temporary/$archive"
expected=$(awk -v name="$archive" '$2 == name { print $1; exit }' "$temporary/checksums")
[ -n "$expected" ] || { echo "error: official checksum missing for $archive" >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum "$temporary/$archive" | awk '{print $1}'); else actual=$(shasum -a 256 "$temporary/$archive" | awk '{print $1}'); fi
[ "$actual" = "$expected" ] || { echo 'error: Lego checksum verification failed' >&2; exit 1; }
mkdir -p "$install_dir"
tar -xzf "$temporary/$archive" -C "$install_dir" lego
chmod 0755 "$install_dir/lego"
printf '%s\n' "$install_dir/lego"
