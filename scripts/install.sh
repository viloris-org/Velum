#!/usr/bin/env sh
# Install an explicitly selected Velum release after checksum verification.
set -eu

repository='viloris-org/Velum'
version=''
channel=''
install_dir="${HOME}/.local/bin"
add_to_path=false
latest=false

usage() {
    cat <<'EOF'
Usage: install.sh --channel <beta|stable> (--version <vX.Y.Z[-prerelease]> | --latest) [--install-dir <directory>] [--add-to-path]

Installs a checksum-verified Velum release from GitHub Releases.
Beta releases are prereleases and do not establish a stable protocol or support
commitment. --latest selects the most recently published matching release and
is not reproducible; use --version for a pinned installation.

--add-to-path appends ~/.local/bin to the login shell's startup file when the
default install directory is used. It does not modify shell configuration by
default.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            version=${2:?missing version value}
            shift 2
            ;;
        --channel)
            channel=${2:?missing channel value}
            shift 2
            ;;
        --latest)
            latest=true
            shift
            ;;
        --install-dir)
            install_dir=${2:?missing install directory value}
            shift 2
            ;;
        --add-to-path)
            add_to_path=true
            shift
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

case "$channel" in
    beta|stable) ;;
    '') echo 'error: --channel beta or --channel stable is required' >&2; exit 2 ;;
    *) echo 'error: --channel must be beta or stable' >&2; exit 2 ;;
esac

if [ "$latest" = true ] && [ -n "$version" ]; then
    echo 'error: --version and --latest cannot be used together' >&2
    exit 2
fi
if [ "$latest" = false ] && [ -z "$version" ]; then
    echo 'error: --version or --latest is required' >&2
    exit 2
fi

if [ "$add_to_path" = true ] && [ "$install_dir" != "${HOME}/.local/bin" ]; then
    echo 'error: --add-to-path is supported only with the default ~/.local/bin install directory' >&2
    exit 2
fi

case "$(uname -s)" in
    Linux)
        case "$(uname -m)" in
            x86_64) platform='linux-x86_64' ;;
            aarch64|arm64) platform='linux-aarch64' ;;
            *) echo "error: no Linux release is published for $(uname -m)" >&2; exit 1 ;;
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

resolve_latest_version() {
    releases="${temporary_dir}/releases.json"
    download "https://api.github.com/repos/${repository}/releases?per_page=100" "$releases"
    tags=$(grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' "$releases" \
        | sed 's/.*"\([^"]*\)"$/\1/' || true)

    for candidate in $tags; do
        if ! printf '%s\n' "$candidate" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$'; then
            continue
        fi
        case "$channel:$candidate" in
            beta:*-*) printf '%s\n' "$candidate"; return 0 ;;
            stable:*-*) ;;
            stable:*) printf '%s\n' "$candidate"; return 0 ;;
        esac
    done

    echo "error: no published ${channel} release was found" >&2
    exit 1
}

if [ "$latest" = true ]; then
    version=$(resolve_latest_version)
    printf 'Resolved latest %s release to %s\n' "$channel" "$version"
fi

if ! printf '%s\n' "$version" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$'; then
    echo 'error: --version must name an explicit vX.Y.Z or vX.Y.Z-prerelease tag' >&2
    exit 2
fi

case "$channel:$version" in
    beta:*-*) ;;
    stable:v[0-9]*.[0-9]*.[0-9]*)
        case "$version" in
            *-*) echo 'error: stable releases cannot use a prerelease tag' >&2; exit 2 ;;
        esac
        ;;
    beta:*) echo 'error: beta releases require a prerelease tag such as v0.0.1-beta' >&2; exit 2 ;;
    *) echo 'error: stable releases require a vX.Y.Z tag' >&2; exit 2 ;;
esac

archive="velum-${version}-${platform}.tar.gz"
base_url="https://github.com/${repository}/releases/download/${version}"

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
printf 'Installed velum %s release %s to %s/velum\n' "$channel" "$version" "$install_dir"

if [ "$add_to_path" = true ]; then
    case "${SHELL:-}" in
        */zsh) profile="${HOME}/.zshrc" ;;
        */bash) profile="${HOME}/.bashrc" ;;
        *) profile="${HOME}/.profile" ;;
    esac
    path_line='export PATH="$HOME/.local/bin:$PATH"'
    if [ ! -f "$profile" ] || ! grep -Fqx "$path_line" "$profile"; then
        printf '\n# Added by the Velum installer\n%s\n' "$path_line" >> "$profile"
        printf 'Added ~/.local/bin to PATH in %s\n' "$profile"
    fi
    printf 'Open a new shell or run: export PATH="$HOME/.local/bin:$PATH"\n'
elif ! command -v velum >/dev/null 2>&1; then
    printf 'Run ~/.local/bin/velum, add ~/.local/bin to PATH, or rerun with --add-to-path.\n'
fi

if [ -t 0 ] && [ -t 1 ]; then
    printf 'Starting Velum first-time setup...\n'
    exec "${install_dir}/velum" setup
fi

printf 'No interactive terminal detected. Run %s/velum setup to start first-time setup.\n' "$install_dir"
