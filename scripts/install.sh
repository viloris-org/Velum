#!/usr/bin/env sh
# Install an explicitly selected Velum research snapshot after checksum verification.
set -eu

repository='viloris-org/Velum'
version=''
install_dir="${HOME}/.local/bin"

usage() {
    cat <<'EOF'
Usage: install.sh --version <snapshot-tag> [--install-dir <directory>]

Installs a checksum-verified Velum research snapshot from GitHub Releases.
Snapshots are experimental and are not supported releases.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            version=${2:?missing version value}
            shift 2
            ;;
        --install-dir)
            install_dir=${2:?missing install directory value}
            shift 2
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac
done

if [ -z "$version" ]; then
    echo 'error: --version is required' >&2
    exit 2
fi

case "$(uname -s)" in
    Linux)
        case "$(uname -m)" in
            x86_64) platform='linux-x86_64' ;;
            *) echo "error: no Linux snapshot is published for $(uname -m)" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$(uname -m)" in
            arm64) platform='darwin-aarch64' ;;
            x86_64) platform='darwin-x86_64' ;;
            *) echo "error: unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
        esac
        ;;
    *) echo "error: unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac

archive="velum-${version}-${platform}.tar.gz"
base_url="https://github.com/${repository}/releases/download/${version}"
temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

download() {
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error "$1" --output "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget --quiet --output-document="$2" "$1"
    else
        echo 'error: curl or wget is required' >&2
        exit 1
    fi
}

download "${base_url}/SHA256SUMS" "${temporary_dir}/SHA256SUMS"
download "${base_url}/${archive}" "${temporary_dir}/${archive}"

expected=$(awk -v artifact="$archive" '$2 == artifact { print $1; exit }' "${temporary_dir}/SHA256SUMS")
if [ -z "$expected" ]; then
    echo "error: checksum for ${archive} is absent from SHA256SUMS" >&2
    exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "${temporary_dir}/${archive}" | awk '{print $1}')
else
    actual=$(shasum -a 256 "${temporary_dir}/${archive}" | awk '{print $1}')
fi
if [ "$actual" != "$expected" ]; then
    echo "error: checksum verification failed for ${archive}" >&2
    exit 1
fi

tar -xzf "${temporary_dir}/${archive}" -C "$temporary_dir"
if [ ! -f "${temporary_dir}/velum" ]; then
    echo 'error: archive has no velum binary' >&2
    exit 1
fi
mkdir -p "$install_dir"
if [ ! -w "$install_dir" ]; then
    echo "error: ${install_dir} is not writable; pass a writable --install-dir" >&2
    exit 1
fi
install -m 0755 "${temporary_dir}/velum" "${install_dir}/velum"
printf 'Installed velum research snapshot %s to %s/velum\n' "$version" "$install_dir"
