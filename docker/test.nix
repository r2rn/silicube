# Pure Nix test image builder using dockerTools
#
# This creates a container image containing isolate, the runtime environment,
# and a Rust stable toolchain for compiling and running integration tests.
#
# Usage:
#    nix build .#docker-test
#    docker load < result
#    docker run --rm --privileged -v $(pwd):/build -w /build silicube-test:latest \
#      cargo test -p silicube --features integration-tests -- --include-ignored
{
  pkgs,
  toolchain,
  isolate,
  rustToolchain,
}: let
  # Collect all packages from enabled languages
  languagePackages = builtins.concatLists (
    map (lang: toolchain.languages.${lang}.packages) toolchain.enabled
  );
in
  pkgs.dockerTools.buildLayeredImage {
    name = "silicube-test";
    tag = "latest";

    contents =
      [
        pkgs.coreutils
        pkgs.bash
        pkgs.gcc
        pkgs.cacert
        isolate
        rustToolchain
      ]
      ++ languagePackages;

    config = {
      Env = [
        "PATH=/bin:${rustToolchain}/bin:${isolate}/bin"
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
      WorkingDir = "/build";
      Volumes = {
        "/build" = {};
      };
    };

    # Create required directories:
    # - var/local/lib/isolate: box root for isolate sandbox data
    # - usr, lib: isolate's built-in defaults bind-mount these from the host,
    #   but Nix packages don't create FHS directories, so we need empty ones
    extraCommands = ''
      mkdir -p var/local/lib/isolate usr lib tmp
    '';

    maxLayers = 100;
  }
