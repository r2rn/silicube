#!/usr/bin/env bash

# Helper script for running Silicube integration tests in Docker
#
# Usage:
#    ./scripts/run-tests.sh                      # Local dev: mounts source, caches deps
#    ./scripts/run-tests.sh --ci                 # CI: hermetic, uses baked-in source
#    ./scripts/run-tests.sh --build-only         # Just build the test image
#    ./scripts/run-tests.sh --no-build           # Skip image build, use existing image
#    ./scripts/run-tests.sh -- --test-threads=1  # Pass extra args to cargo test
#
# Environment variables:
#    CONTAINER_ENGINE - Container runtime to use (default: docker)
#    TEST_IMAGE       - Image name (default: silicube-test:latest)

set -euo pipefail

ENGINE="${CONTAINER_ENGINE:-docker}"
IMAGE="${TEST_IMAGE:-silicube-test:latest}"

usage() {
    echo "Usage: $0 [--ci|--build-only|--no-build] [-- <cargo test args>]"
    echo
    echo "Modes:"
    echo "  (default)     Local dev: mounts source dir, uses named volumes for cargo cache"
    echo "  --ci          CI mode: hermetic build, no mounts, uses baked-in source"
    echo "  --build-only  Just build the test image, don't run tests"
    echo "  --no-build    Skip image build, use existing image"
    echo
    echo "Environment variables:"
    echo "  CONTAINER_ENGINE  Container runtime (docker or podman, default: docker)"
    echo "  TEST_IMAGE        Image name (default: silicube-test:latest)"
    echo
    echo "Examples:"
    echo "  $0                              # Run all tests (local dev)"
    echo "  $0 --ci                         # Run all tests (CI mode)"
    echo "  $0 -- --test-threads=1          # Run tests single-threaded"
    echo "  $0 --no-build -- sandbox_lifecycle  # Run specific test module"
    exit 1
}

build_image() {
    echo "Building test image..."
    "$ENGINE" build -f Dockerfile.test -t "$IMAGE" .
}

MODE="local"
SKIP_BUILD=false
CARGO_EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --ci)
            MODE="ci"
            shift
            ;;
        --build-only)
            MODE="build-only"
            shift
            ;;
        --no-build)
            SKIP_BUILD=true
            shift
            ;;
        --help|-h)
            usage
            ;;
        --)
            shift
            CARGO_EXTRA_ARGS=("$@")
            break
            ;;
        *)
            echo "Error: Unknown option '$1'"
            echo
            usage
            ;;
    esac
done

# Build the test command
TEST_CMD=(cargo test -p silicube --features integration-tests -- --include-ignored)
if [[ ${#CARGO_EXTRA_ARGS[@]} -gt 0 ]]; then
    TEST_CMD+=("${CARGO_EXTRA_ARGS[@]}")
fi

case "$MODE" in
    build-only)
        build_image
        echo "Test image built: $IMAGE"
        ;;
    ci)
        if [[ "$SKIP_BUILD" == false ]]; then
            build_image
        fi
        exec "$ENGINE" run --rm --privileged \
            "$IMAGE" \
            "${TEST_CMD[@]}"
        ;;
    local)
        if [[ "$SKIP_BUILD" == false ]]; then
            build_image
        fi
        exec "$ENGINE" run --rm --privileged \
            -v "$(pwd):/build" \
            -v silicube-cargo-registry:/usr/local/cargo/registry \
            -v silicube-cargo-git:/usr/local/cargo/git \
            -v silicube-target:/build/target \
            "$IMAGE" \
            "${TEST_CMD[@]}"
        ;;
esac
