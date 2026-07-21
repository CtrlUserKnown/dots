#!/usr/bin/env bash
# ── dots CI test runner ────────────────────────────────────────────────────────
# Runs the full CI pipeline inside a Podman container.
#
# Usage:
#   test-env/test.sh              # run on fedora (default)
#   test-env/test.sh ubuntu       # run on ubuntu
#   test-env/test.sh fedora       # run on fedora explicitly
#   test-env/test.sh both         # run on both sequentially

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(dirname "$HERE")"

CI_IMAGE_TAG="dots-ci"

run_ci() {
    local base="$1"
    local image="${CI_IMAGE_TAG}:${base}"

    echo "========================================"
    echo "  Building CI image: ${base}"
    echo "========================================"
    podman build \
        --build-arg "BASE=${base}" \
        -t "${image}" \
        -f "${HERE}/Containerfile.ci" \
        "${REPO}"

    echo ""
    echo "========================================"
    echo "  Running CI pipeline: ${base}"
    echo "========================================"
    podman run --rm \
        -v "${REPO}:/src:Z" \
        "${image}"
}

case "${1:-fedora}" in
    both)
        run_ci fedora
        echo ""
        run_ci ubuntu
        ;;
    fedora|ubuntu)
        run_ci "$1"
        ;;
    *)
        echo "Usage: $0 [fedora|ubuntu|both]" >&2
        exit 1
        ;;
esac
