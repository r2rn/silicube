# Silicube

Silicube is a tool wrapping IOI Isolate for sandboxed code execution. It can be run natively on Linux, or can be used inside a Docker container. Nix is used for toolchain management.

> [!NOTE]
> This tool only works on Linux, given IOI Isolate requires Linux-specific kernel features such as namespaces and control groups.

Interactive execution is supported through the core `silicube` library. We may add additional "adapters" provided in `silicube-server` that provide HTTP and WebSocket interfaces later.

## Setup

### Prerequisites

- Nix (with flakes enabled), **or** Docker/Podman

### Option 1: Nix (recommended)

Build the Docker image directly with Nix (produces the smallest image):

```sh
nix build .#docker -o silicube-image
docker load < silicube-image
```

### Option 2: Dockerfile

Build without Nix on the host (uses Nix internally via multi-stage build):

```sh
docker build -t silicube:latest .
```

### Running the container

Isolate requires elevated privileges. Use the helper script or run directly:

```sh
# Helper script (privileged mode)
./scripts/run-silicube.sh --privileged run --language cpp17 main.cpp

# Helper script (hardened mode - minimal capabilities)
./scripts/run-silicube.sh --hardened run --language python3 solution.py

# Or run directly
docker run --rm --privileged -v "$(pwd):/work" silicube:latest run --language cpp17 main.cpp
```

Use `CONTAINER_ENGINE=podman` for Podman support. Edit `toolchain.nix` to configure which languages are included in the image.

## Testing

Unit tests (no special requirements):

```sh
cargo test -p silicube
```

Integration tests require root and the `isolate` binary. The easiest way is via Docker:

```sh
# Local dev (mounts source, caches dependencies between runs)
./scripts/run-tests.sh

# CI mode (hermetic, no mounts)
./scripts/run-tests.sh --ci

# Pass extra args to cargo test
./scripts/run-tests.sh -- --test-threads=1
```

Or with Nix:

```sh
./scripts/run-tests-nix.sh
```
