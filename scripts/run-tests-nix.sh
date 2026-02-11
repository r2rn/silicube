#!/usr/bin/env bash

# Helper script for running Silicube integration tests via Nix

set -euo pipefail

nix build .#docker-test -o silicube-test
docker load < silicube-test

docker run --rm --privileged -v "$(pwd):/build" -w /build silicube-test:latest \
    cargo test -p silicube --features integration-tests -- --include-ignored
