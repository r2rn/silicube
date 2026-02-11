#!/usr/bin/env bash

# Helper script for running the Silicube container
#
# Usage:
#    ./scripts/run-silicube.sh --privileged <silicube args>
#    ./scripts/run-silicube.sh --hardened <silicube args>
#
# Environment variables:
#    CONTAINER_ENGINE - Container runtime to use (default: docker)
#                       Set to "podman" for Podman support

set -euo pipefail

ENGINE="${CONTAINER_ENGINE:-docker}"
IMAGE="${SILICUBE_IMAGE:-silicube:latest}"

usage() {
    echo "Usage: $0 [--privileged|--hardened] <silicube args>"
    echo
    echo "Options:"
    echo "  --privileged  Run with full privileges (simpler, less secure)"
    echo "  --hardened    Run with minimal required capabilities"
    echo
    echo "Environment variables:"
    echo "  CONTAINER_ENGINE  Container runtime (docker or podman, default: docker)"
    echo "  SILICUBE_IMAGE    Image name (default: silicube:latest)"
    echo
    echo "Examples:"
    echo "  $0 --privileged run --language cpp17 -f main.cpp"
    echo "  CONTAINER_ENGINE=podman $0 --hardened run --language python3 -f solution.py"
    exit 1
}

if [[ $# -lt 1 ]]; then
    usage
fi

MODE="$1"
shift

case "$MODE" in
    --privileged)
        exec "$ENGINE" run --rm --privileged \
            -v "$(pwd):/work" \
            "$IMAGE" "$@"
        ;;
    --hardened)
        exec "$ENGINE" run --rm \
            --cap-add=SYS_ADMIN \
            --cap-add=SYS_PTRACE \
            --security-opt apparmor=unconfined \
            --security-opt seccomp=unconfined \
            -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
            -v "$(pwd):/work" \
            "$IMAGE" "$@"
        ;;
    --help|-h)
        usage
        ;;
    *)
        echo "Error: Unknown option '$MODE'"
        echo
        usage
        ;;
esac
