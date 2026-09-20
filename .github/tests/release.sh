#!/usr/bin/env bash
# Execute the workflow's release lookup and publishing branches with an inert gh.
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -f "$test_dir/workflow.sh" "$test_dir/commands" "$test_dir/summary" "$test_dir/output"; rmdir "$test_dir"' EXIT
sed -n '/^          existing=/,$p' "$repo/.github/workflows/release.yml" > "$test_dir/workflow.sh"
test -s "$test_dir/workflow.sh"

export GITHUB_REPOSITORY=RoGrid-HQ/rogrid
export GITHUB_STEP_SUMMARY="$test_dir/summary"
export ROGRID_TEST_COMMANDS="$test_dir/commands"
export tag=v0.0.1

gh() {
    case "$1 $2" in
        'api graphql')
            [[ "$*" == *" -f owner=RoGrid-HQ -f name=rogrid -f tag=$tag "* ]] || return 2
            [[ "$*" == *'release(tagName: $tag)'* ]] || return 2
            [[ "$*" == *'--jq .data.repository.release.isDraft' ]] || return 2
            printf 'lookup\n' >> "$ROGRID_TEST_COMMANDS"
            case "$ROGRID_TEST_RELEASE" in
                published) printf 'false\n' ;;
                draft) printf 'true\n' ;;
                missing) printf '\n' ;; # gh api --jq prints an empty result for null.
                error) printf 'GitHub lookup failed\n' >&2; return 1 ;;
                *) return 2 ;;
            esac
            ;;
        'release create'|'release upload'|'release edit')
            [[ "$3" == "$tag" ]] || return 2
            printf '%s\n' "$2" >> "$ROGRID_TEST_COMMANDS"
            ;;
        *) printf 'Unexpected gh command: %s\n' "$*" >&2; return 2 ;;
    esac
}
export -f gh

for scenario in published draft missing error; do
    export ROGRID_TEST_RELEASE="$scenario"
    : > "$ROGRID_TEST_COMMANDS"
    status=0
    bash --noprofile --norc -euo pipefail "$test_dir/workflow.sh" > "$test_dir/output" 2>&1 || status=$?
    case "$scenario" in
        published) expected='lookup' ;;
        draft) expected=$'lookup\nupload\nedit' ;;
        missing) expected=$'lookup\ncreate\nupload\nedit' ;;
        error) expected='lookup' ;;
    esac
    if [[ "$scenario" == error ]]; then
        test "$status" -ne 0
    elif [[ "$status" -ne 0 ]]; then
        cat "$test_dir/output"
        exit "$status"
    fi
    if [[ "$(cat "$ROGRID_TEST_COMMANDS")" != "$expected" ]]; then
        printf 'Unexpected commands for %s:\n' "$scenario"
        cat "$ROGRID_TEST_COMMANDS"
        exit 1
    fi
    printf 'PASS: %s release\n' "$scenario"
done
