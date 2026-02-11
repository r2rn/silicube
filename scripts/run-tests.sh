#!/usr/bin/env bash

# Helper script for running Silicube integration tests via Nix + Docker
#
# Builds a test image with `nix build .#docker-test`, loads it, and runs
# the integration test suite inside a privileged container.
#
# Usage:
#    ./scripts/run-tests.sh                          # Run all integration tests
#    ./scripts/run-tests.sh -- --test-threads=1      # Pass extra args to cargo test
#    ./scripts/run-tests.sh -- sandbox_lifecycle     # Run a specific test module
#
# Environment variables:
#    CONTAINER_ENGINE - Container runtime to use (default: docker)

set -euo pipefail

ENGINE="${CONTAINER_ENGINE:-docker}"
IMAGE="silicube-test:latest"

CARGO_EXTRA_ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --)
            shift
            CARGO_EXTRA_ARGS=("$@")
            break
            ;;
        *)
            echo "Usage: $0 [-- <cargo test args>]"
            exit 1
            ;;
    esac
done

echo "Building test image..."
nix build .#docker-test -o silicube-test
"$ENGINE" load < silicube-test

TEST_CMD=(cargo test -p silicube --features integration-tests -- --include-ignored)
if [[ ${#CARGO_EXTRA_ARGS[@]} -gt 0 ]]; then
    TEST_CMD+=("${CARGO_EXTRA_ARGS[@]}")
fi

exec "$ENGINE" run --rm --privileged \
    -v "$(pwd):/build" -w /build \
    "$IMAGE" \
    "${TEST_CMD[@]}"
