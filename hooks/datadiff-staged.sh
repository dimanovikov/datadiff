#!/bin/sh
# Diff every staged file against its committed version.
#
# pre-commit hands us the hook's `args` first and the file list after, and the
# two are not otherwise marked, so options must be written in `--opt=value`
# form: anything starting with a dash is an option, everything else is a path.
#
# Without a --fail-on policy the hook only reports. Failing on *any* change
# would block every commit, since a commit is a change by definition.

set -u

opts=""
policy=0
while [ $# -gt 0 ]; do
    case "$1" in
        --fail-on=*) policy=1; opts="$opts $1"; shift ;;
        -*)          opts="$opts $1"; shift ;;
        *)           break ;;
    esac
done

status=0
for file in "$@"; do
    before=$(mktemp)
    if git cat-file -e "HEAD:$file" 2>/dev/null; then
        git show "HEAD:$file" >"$before"
    else
        : >"$before" # new file: report it as all-additions
    fi

    rc=0
    # shellcheck disable=SC2086 # opts is a deliberately word-split option list
    datadiff $opts "$before" "$file" || rc=$?
    rm -f "$before"

    case "$rc" in
        0) ;;
        1) [ "$policy" -eq 1 ] && status=1 ;;
        *) status=1 ;; # unreadable file or unknown format
    esac
done

exit "$status"
