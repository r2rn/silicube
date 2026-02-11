# User-editable language and toolchain definitions for Silicube
#
# This file configures which programming languages are available in the
# Silicube container. Edit the `enabled` list to add or remove languages.
#
# To add a new language:
# 1. Add an entry to `languages` with the required packages
# 2. Add the language name to `enabled`
# 3. Rebuild the container
{
  pkgs,
  jdkMinimal,
}: {
  languages = {
    c = {
      packages = with pkgs; [
        gcc
      ];
    };

    cpp17 = {
      packages = with pkgs; [
        gcc
      ];
    };

    cpp20 = {
      packages = with pkgs; [
        gcc
      ];
    };

    python3 = {
      packages = with pkgs; [
        python312
      ];
    };

    java = {
      packages = [
        jdkMinimal
      ];
    };

    rust = {
      packages = with pkgs; [
        rustc
      ];
    };

    go = {
      packages = with pkgs; [
        go
      ];
    };

    javascript = {
      packages = with pkgs; [
        nodejs-slim_22
      ];
    };
  };

  # Enable/disable languages by editing this list
  # Only enabled languages will be included in the container image
  enabled = [
    "c"
    "cpp17"
    "cpp20"
    "python3"
    "java"
    "rust"
    "go"
    "javascript"
  ];
}
