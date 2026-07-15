#!/usr/bin/env sh
# Regression coverage for release selection without contacting GitHub.
set -eu

root=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM
mkdir -p "$temporary_dir/bin"

cat > "$temporary_dir/bin/curl" <<'EOF'
#!/usr/bin/env sh
set -eu
output=''
url=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) output=$2; shift 2 ;;
        http*) url=$1; shift ;;
        *) shift ;;
    esac
done
case "$url" in
    *'/releases?per_page=100')
        printf '%s\n' \
            '[{"tag_name":"v0.0.1-beta"},{"tag_name":"v0.0.1-beta-2"}]' > "$output"
        ;;
    *) exit 1 ;;
esac
EOF
chmod +x "$temporary_dir/bin/curl"

if ! output=$(PATH="$temporary_dir/bin:$PATH" HOME="$temporary_dir/home" \
    sh "$root/scripts/install.sh" --channel beta --latest --install-dir "$temporary_dir/install" 2>&1); then
    case "$output" in
        *'Resolved latest beta release to v0.0.1-beta-2'*) exit 0 ;;
        *) printf '%s\n' "$output" >&2; exit 1 ;;
    esac
fi

printf '%s\n' 'expected the mocked artifact download to fail' >&2
exit 1
