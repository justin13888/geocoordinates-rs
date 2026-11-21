#!/usr/bin/env bash
# The `file-length-check` gate, run through `mise run file-length-check [base]`.
#
# Usage: file_length_check.sh MAX_LINES BASE
#
# Run from the root of the repository it checks. Fails any tracked file over
# MAX_LINES (the task passes `max_file_lines` from .mise.toml) unless
# `.file-length-baseline` lists it, and holds every listed file to its recorded
# length. Against BASE, which must resolve under `CI=true`, the baseline may
# only shrink: no new entry, raised cap, or raised `max_file_lines`. No
# associative arrays: macOS's bash 3.2.
set -euo pipefail
if [ "$#" -ne 2 ]; then
    echo "usage: file_length_check.sh MAX_LINES BASE" >&2
    exit 2
fi
max="$1"
base="$2"
case "$max" in
    '' | *[!0-9]*)
        echo "file-length-check: MAX_LINES '$max' is not a whole number" >&2
        exit 2
        ;;
esac
baseline=".file-length-baseline"
if [ ! -f "$baseline" ]; then
    echo "file-length-check: $baseline is missing" >&2
    exit 1
fi
lengths="$(mktemp)"
base_baseline="$(mktemp)"
trap 'rm -f "$lengths" "$base_baseline"' EXIT

# "<lines> <path>" for every tracked regular text file present in the worktree.
# A binary file (one `grep -I` reads as binary, e.g. the bundled gazetteer data)
# has no lines to count, so it is skipped. awk counts an unterminated last line.
git ls-files -z | while IFS= read -r -d '' path; do
    if [ -f "$path" ] && [ ! -L "$path" ] && { [ ! -s "$path" ] || grep -Iq '' "$path"; }; then
        printf '%s %s\n' "$(awk 'END { print NR }' "$path")" "$path"
    fi
done > "$lengths"

# Parses a baseline file into cap[] and gen[]; shared by both passes below.
parse='
function parse_line(line, fields, n) {
    if (line ~ /^[[:space:]]*(#|$)/) return 0
    n = split(line, fields, /[[:space:]]+/)
    if (n != 2 || (fields[1] != "generated" && fields[1] !~ /^[0-9]+$/)) {
        printf "file-length-check: malformed baseline line: %s\n", line
        bad = 1
        return 0
    }
    kind = fields[1]; entry = fields[2]
    return 1
}'

status=0
awk -v max="$max" -v file="$baseline" "$parse"'
FNR == NR {
    if (!parse_line($0)) next
    if (kind == "generated") gen[entry] = 1; else cap[entry] = kind + 0
    next
}
{
    n = $1 + 0
    path = substr($0, index($0, " ") + 1)
    seen[path] = 1
    if (path in gen) next
    if (path in cap) {
        if (n <= max) {
            printf "%s: %d lines, within the %d-line limit; remove it from %s\n", path, n, max, file
            bad = 1
        } else if (n > cap[path]) {
            printf "%s: %d lines, grew past its baselined %d; split it instead\n", path, n, cap[path]
            bad = 1
        } else if (n < cap[path]) {
            printf "%s: %d lines, shrank from %d; lower its entry in %s to %d\n", path, n, cap[path], file, n
            bad = 1
        }
    } else if (n > max) {
        printf "%s: %d lines, over the %d-line limit\n", path, n, max
        bad = 1
    }
}
END {
    for (p in cap) if (!(p in seen)) {
        printf "%s: listed in %s but not a tracked file; remove the entry\n", p, file
        bad = 1
    }
    for (p in gen) if (!(p in seen)) {
        printf "%s: listed in %s but not a tracked file; remove the entry\n", p, file
        bad = 1
    }
    exit bad
}' "$baseline" "$lengths" || status=1

base_ok=1
if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
    [ "${CI:-}" = true ] && { echo "file-length-check: no '$base' ref under CI" >&2; exit 1; }
    echo "file-length-check: warning: no '$base' ref; skipping the shrink-only comparison." >&2
    base_ok=0
fi

# The limit itself may only fall too: a raised `max_file_lines` would loosen the
# gate for every file that the baseline comparison below never sees.
if [ "$base_ok" -eq 1 ]; then
    base_max="$(git show "$base:.mise.toml" 2>/dev/null \
        | sed -n 's/^max_file_lines = "\([0-9][0-9]*\)"$/\1/p' || true)"
    if [ -z "$base_max" ]; then
        echo "file-length-check: $base sets no max_file_lines; skipping the limit comparison."
    elif [ "$max" -gt "$base_max" ]; then
        echo "max_file_lines: raised from $base_max to $max vs $base; the limit may only shrink"
        status=1
    fi
fi

if [ "$base_ok" -eq 0 ]; then
    :
elif ! git show "$base:$baseline" > "$base_baseline" 2>/dev/null; then
    echo "file-length-check: $base has no $baseline; skipping the shrink-only comparison."
else
    awk -v base="$base" -v file="$baseline" "$parse"'
    FNR == NR {
        if (!parse_line($0)) next
        if (kind == "generated") bgen[entry] = 1; else bcap[entry] = kind + 0
        next
    }
    {
        if (!parse_line($0)) next
        if (kind == "generated") {
            if (!(entry in bgen)) {
                printf "%s: new generated exclusion in %s; the baseline may only shrink\n", entry, file
                bad = 1
            }
        } else if (!(entry in bcap)) {
            printf "%s: new entry in %s; the baseline may only shrink\n", entry, file
            bad = 1
        } else if (kind + 0 > bcap[entry]) {
            printf "%s: cap raised from %d to %d vs %s; the baseline may only shrink\n", entry, bcap[entry], kind, base
            bad = 1
        }
    }
    END { exit bad }' "$base_baseline" "$baseline" || status=1
fi

if [ "$status" -eq 0 ]; then
    echo "file-length-check: every tracked file is within $max lines or its baselined length."
fi
exit "$status"
