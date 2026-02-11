# Silicube

Silicube is both a library and standalone tool wrapping IOI Isolate for sandboxed code execution.

Since Isolate requires Linux-specific kernel features (e.g., namespaces, cgroups), it can only be run natively on Linux. To run it on other operating systems, you must use a Docker container. Nix is optionally used for toolchain management.

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

Integration tests require Linux, root privileges, and `isolate` (which uses kernel namespaces and cgroups). The easiest way is via the Nix-built Docker test image:

```sh
# Run all integration tests
./scripts/run-tests.sh

# Pass extra args to cargo test
./scripts/run-tests.sh -- --test-threads=1

# Run a specific test module
./scripts/run-tests.sh -- sandbox_lifecycle
```

This builds a test image with `nix build .#docker-test`, loads it into Docker, and runs the tests inside a privileged container with your source mounted.

### Running integration tests natively

If you have `isolate` installed on your Linux system (e.g. via `nix build .#isolate`), you can run integration tests directly without Docker:

```sh
sudo -E cargo test -p silicube --features integration-tests -- --include-ignored
```

Note that native execution requires:

- Linux (Isolate uses kernel namespaces and cgroups)
- Root privileges (or equivalent capabilities)
- `isolate` on `$PATH`
- Language toolchains from `toolchain.nix` available on the system
