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
Also installs the pinned, checksum-verified Lego companion used by guided ACME
certificate provisioning.
Beta releases are prereleases and do not establish a stable protocol or support
commitment. --latest selects the highest matching semantic version and is not
reproducible; use --version for a pinned installation.

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
            x86_64) platform='linux-x86_64'; lego_platform='linux_amd64' ;;
            aarch64|arm64) platform='linux-aarch64'; lego_platform='linux_arm64' ;;
            *) echo "error: no Linux release is published for $(uname -m)" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$(uname -m)" in
            arm64) platform='darwin-aarch64'; lego_platform='darwin_arm64' ;;
            x86_64) platform='darwin-x86_64'; lego_platform='darwin_amd64' ;;
            *) echo "error: unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
        esac
        ;;
    *) echo "error: unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac

temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

download() {
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error --retry 3 --retry-delay 1 \
            --retry-all-errors "$1" --output "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget --quiet --tries=4 --output-document="$2" "$1"
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

    latest_version=''
    for candidate in $tags; do
        if ! printf '%s\n' "$candidate" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$'; then
            continue
        fi
        case "$channel:$candidate" in
            beta:*-*) ;;
            stable:*-*) ;;
            stable:*) ;;
            *) continue ;;
        esac

        if [ -z "$latest_version" ] || semver_is_newer "$candidate" "$latest_version"; then
            latest_version=$candidate
        fi
    done

    if [ -n "$latest_version" ]; then
        printf '%s\n' "$latest_version"
        return 0
    fi

    echo "error: no published ${channel} release was found" >&2
    exit 1
}

semver_is_newer() {
    LC_ALL=C awk -v candidate="${1#v}" -v current="${2#v}" '
        function compare_identifiers(left, right,    left_parts, right_parts, left_count, right_count, position, left_item, right_item) {
            left_count = split(left, left_parts, ".")
            right_count = split(right, right_parts, ".")
            for (position = 1; position <= left_count && position <= right_count; position++) {
                left_item = left_parts[position]
                right_item = right_parts[position]
                if (left_item ~ /^[0-9]+$/ && right_item ~ /^[0-9]+$/) {
                    if ((left_item + 0) != (right_item + 0)) return (left_item + 0 > right_item + 0) ? 1 : -1
                } else if (left_item ~ /^[0-9]+$/) {
                    return -1
                } else if (right_item ~ /^[0-9]+$/) {
                    return 1
                } else if (left_item != right_item) {
                    return (left_item > right_item) ? 1 : -1
                }
            }
            return (left_count == right_count) ? 0 : ((left_count > right_count) ? 1 : -1)
        }

        function compare_versions(left, right,    left_base, right_base, left_pre, right_pre, left_numbers, right_numbers, position, result) {
            left_base = left
            right_base = right
            left_pre = left
            right_pre = right
            sub(/-.*/, "", left_base)
            sub(/-.*/, "", right_base)
            sub(/^[^-]*-/, "", left_pre)
            sub(/^[^-]*-/, "", right_pre)
            if (left_base == left) left_pre = ""
            if (right_base == right) right_pre = ""
            split(left_base, left_numbers, ".")
            split(right_base, right_numbers, ".")
            for (position = 1; position <= 3; position++) {
                if ((left_numbers[position] + 0) != (right_numbers[position] + 0)) {
                    return (left_numbers[position] + 0 > right_numbers[position] + 0) ? 1 : -1
                }
            }
            if (left_pre == "" || right_pre == "") {
                return (left_pre == right_pre) ? 0 : ((left_pre == "") ? 1 : -1)
            }
            return compare_identifiers(left_pre, right_pre)
        }

        BEGIN { exit compare_versions(candidate, current) > 0 ? 0 : 1 }
    '
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
lego_version='5.2.2'
lego_archive="lego_v${lego_version}_${lego_platform}.tar.gz"
lego_base_url="https://github.com/go-acme/lego/releases/download/v${lego_version}"
lego_install_dir="${XDG_DATA_HOME:-$HOME/.local/share}/velum/tools/lego/v${lego_version}"
download "${lego_base_url}/lego_${lego_version}_checksums.txt" "${temporary_dir}/lego-checksums"
download "${lego_base_url}/${lego_archive}" "${temporary_dir}/${lego_archive}"
lego_expected=$(awk -v artifact="$lego_archive" '$2 == artifact { print $1; exit }' "${temporary_dir}/lego-checksums")
if [ -z "$lego_expected" ]; then
    echo "error: checksum for ${lego_archive} is absent from Lego's checksum manifest" >&2
    exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
    lego_actual=$(sha256sum "${temporary_dir}/${lego_archive}" | awk '{print $1}')
else
    lego_actual=$(shasum -a 256 "${temporary_dir}/${lego_archive}" | awk '{print $1}')
fi
if [ "$lego_actual" != "$lego_expected" ]; then
    echo 'error: Lego checksum verification failed' >&2
    exit 1
fi
mkdir -p "${temporary_dir}/lego"
tar -xzf "${temporary_dir}/${lego_archive}" -C "${temporary_dir}/lego" lego

mkdir -p "$install_dir" "$lego_install_dir"
if [ ! -w "$install_dir" ]; then
    echo "error: ${install_dir} is not writable; pass a writable --install-dir" >&2
    exit 1
fi
if [ ! -w "$lego_install_dir" ]; then
    echo "error: ${lego_install_dir} is not writable; set XDG_DATA_HOME to a writable directory" >&2
    exit 1
fi
install -m 0755 "${temporary_dir}/velum" "${install_dir}/velum"
printf 'Installed velum %s release %s to %s/velum\n' "$channel" "$version" "$install_dir"

install -m 0755 "${temporary_dir}/lego/lego" "${lego_install_dir}/lego"
printf 'Installed Lego ACME companion %s to %s/lego\n' "$lego_version" "$lego_install_dir"

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
    printf 'Run %s/velum directly or add %s to PATH.\n' "$install_dir" "$install_dir"
fi

if [ -t 0 ] && [ -t 1 ]; then
    printf '\nStarting guided Velum setup.\n'
    printf 'Step 1: confirm the randomly selected UDP listener port and allowed target.\n'
    printf 'Step 2: choose a certificate source:\n'
    printf '  1) request a CA certificate with ACME DNS-01,\n'
    printf '  2) select an existing PEM certificate and private key, or\n'
    printf '  3) generate a self-signed certificate for explicit client trust.\n'
    printf 'For ACME, have the relay DNS name, account email, Lego DNS provider name,\n'
    printf 'and that provider\047s credential environment variables ready.\n'
    printf 'The setup is resumable if certificate issuance or another step fails.\n\n'
    exec "${install_dir}/velum" setup
fi

printf 'No interactive terminal detected. Run %s/velum setup to start the resumable guided setup.\n' "$install_dir"
