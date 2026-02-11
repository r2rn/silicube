{
  description = "Silicube - Secure code execution sandbox";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Parse Cargo.toml for metadata (single source of truth)
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        pname = cargoToml.workspace.package.name;
        version = cargoToml.workspace.package.version;

        # Stable toolchain for building (keeps downstream on stable)
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };

        # Nightly rustfmt for unstable formatting options (group_imports, etc.)
        rustfmtNightly = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rustfmt"];
        };

        # Use mold on Linux, otherwise use default linker
        rustFlags =
          if pkgs.stdenv.isLinux
          then "-C link-arg=-fuse-ld=mold"
          else "";

        devBuildInputs = with pkgs;
          [
            rustToolchain
            cargo-watch
            cargo-edit
          ]
          ++ lib.optionals stdenv.isLinux [
            mold
          ];

        # Minimal JDK (javac + java.base only)
        jdkMinimal = pkgs.callPackage ./docker/jdk-minimal.nix {};

        # Import toolchain configuration
        toolchain = import ./toolchain.nix {inherit pkgs jdkMinimal;};

        # Build silicube binary
        mkSilicube = {buildType ? "release"}:
          pkgs.rustPlatform.buildRustPackage {
            inherit pname version buildType;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [pkgs.mold];

            RUSTFLAGS = rustFlags;
          };

        silicube = mkSilicube {};
        silicube-debug = mkSilicube {buildType = "debug";};

        # Build isolate (Linux only)
        isolate = pkgs.callPackage ./docker/isolate.nix {};

        # Runtime environment for Dockerfile builds
        # Collects all packages from enabled languages
        runtimeEnv = pkgs.buildEnv {
          name = "silicube-runtime";
          paths =
            builtins.concatLists (map (lang: toolchain.languages.${lang}.packages) toolchain.enabled)
            ++ [
              pkgs.coreutils
              pkgs.bash
            ];
        };
      in {
        packages = {
          default = silicube;
          inherit silicube silicube-debug isolate runtimeEnv;

          # Docker image (release)
          docker = import ./docker {
            inherit
              pkgs
              toolchain
              silicube
              isolate
              ;
          };

          # Docker image (debug)
          docker-debug = import ./docker {
            inherit pkgs toolchain isolate;
            silicube = silicube-debug;
          };

          # Docker image for running integration tests
          docker-test = import ./docker/test.nix {
            inherit pkgs toolchain isolate rustToolchain;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = devBuildInputs ++ pkgs.lib.optionals pkgs.stdenv.isLinux [isolate];
          RUSTFLAGS = rustFlags;
          RUSTFMT = "${rustfmtNightly}/bin/rustfmt";
        };
      }
    );
}
