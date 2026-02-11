# Build Silicube image without requiring Nix on host
#
# This Dockerfile uses a multi-stage build to create a Silicube container
# image for users who don't have Nix installed on their system.
#
# Usage:
#   docker build -t silicube:latest .
#   docker load < result  # if using nix build .#docker instead
#
# Note: For smallest images, use `nix build .#docker` instead.

FROM nixos/nix:2.33.2 AS builder

RUN mkdir -p /etc/nix && \
    echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

WORKDIR /build
COPY . .

RUN nix build .#default --out-link /build/silicube
RUN nix build .#isolate --out-link /build/isolate
RUN nix build .#runtimeEnv --out-link /build/runtime

# Copy the closure of all required store paths
RUN mkdir -p /output/nix/store /output/bin /output/etc && \
    nix copy --to /output --no-check-sigs \
        $(nix path-info .#default) \
        $(nix path-info .#isolate) \
        $(nix path-info .#runtimeEnv)

# Create convenience symlinks
RUN ln -s $(nix path-info .#default)/bin/* /output/bin/ && \
    ln -s $(nix path-info .#isolate)/bin/* /output/bin/ && \
    for pkg in $(nix path-info .#runtimeEnv)/bin/*; do \
        ln -sf "$pkg" /output/bin/ 2>/dev/null || true; \
    done

FROM nixos/nix:2.33.2

# Copy built artifacts including Nix store
COPY --from=builder /output/nix/store /nix/store
COPY --from=builder /output/bin /app/bin

# Set up environment
ENV PATH="/app/bin:$PATH"

# Create required directories
RUN mkdir -p /var/local/lib/isolate /work

WORKDIR /work
ENTRYPOINT ["/app/bin/silicube"]
