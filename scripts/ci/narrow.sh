#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: $0 <branch> [--platforms a,b] [--stage s] [--packages a,b] [--tests '<filterset>' [--repeat N]] [--mode fast|exhaustive]"
}

if [[ ${1:-} == --help || ${1:-} == -h ]]; then
    usage
    exit 0
fi
if [[ ${1:-} == '' || $1 == -* ]]; then
    usage >&2
    exit 2
fi

branch=$1
shift
repo=ethosengine/brit
dispatch=(gh workflow run ci -R "$repo" --ref "$branch")
while (($#)); do
    case $1 in
        --platforms|--stage|--packages|--tests|--repeat|--mode)
            if (($# < 2)) || [[ $2 == --* ]]; then
                echo "Missing value for $1" >&2
                exit 2
            fi
            if [[ $1 == --mode && $2 != fast && $2 != exhaustive ]]; then
                echo 'Mode must be fast or exhaustive' >&2
                exit 2
            fi
            dispatch+=(-f "${1#--}=$2")
            shift 2
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac
done

list=(gh run list -R "$repo" --workflow ci --branch "$branch" --event workflow_dispatch --limit 1 --json url --jq '.[0].url // empty')
previous=$("${list[@]}")
"${dispatch[@]}" >/dev/null

for ((attempt = 0; attempt < 10; attempt++)); do
    url=$("${list[@]}")
    if [[ -n $url && $url != "$previous" ]]; then
        printf '%s\n' "$url"
        exit 0
    fi
    sleep 2
done

echo "Dispatch accepted, but the new run URL for $branch is not visible yet." >&2
exit 1
