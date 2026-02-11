# Pure Nix image builder using dockerTools
#
# This creates a minimal container image containing only the necessary
# components for running Silicube with language support.
{
  pkgs,
  toolchain,
  silicube,
  isolate,
}: let
  # Collect all packages from enabled languages
  languagePackages = builtins.concatLists (
    map (lang: toolchain.languages.${lang}.packages) toolchain.enabled
  );
in
  pkgs.dockerTools.buildLayeredImage {
    name = "silicube";
    tag = "latest";

    contents =
      [
        pkgs.coreutils
        pkgs.bash
        pkgs.cacert
        isolate
        silicube
      ]
      ++ languagePackages;

    config = {
      Entrypoint = ["${silicube}/bin/silicube"];
      Env = [
        "PATH=/bin:${silicube}/bin:${isolate}/bin"
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
      WorkingDir = "/work";
      Volumes = {
        "/work" = {};
      };
    };

    # Create required directories:
    # - var/local/lib/isolate: box root for isolate sandbox data
    # - usr, lib: isolate's built-in defaults bind-mount these from the host,
    #   but Nix packages don't create FHS directories, so we need empty ones
    #   to prevent isolate from failing
    extraCommands = ''
      mkdir -p var/local/lib/isolate usr lib
    '';

    # Use many layers for better caching
    maxLayers = 100;
  }
