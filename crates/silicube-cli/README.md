# silicube-cli

Command-line interface for sandboxed code execution, built on the `silicube` library.

## Commands

| Command                | Description                                |
|------------------------|--------------------------------------------|
| `silicube init`        | Create a default silicube.toml config file |
| `silicube compile`     | Compile source code in a sandbox           |
| `silicube run`         | Compile (if needed) and execute code       |
| `silicube languages`   | List available languages                   |
| `silicube show-config` | Display current configuration              |

## Examples

```sh
# Initialize configuration
silicube init

# Run a Python script
silicube run --language python3 solution.py

# Run C++ with custom limits and input
silicube run --language cpp17 --time-limit 2.0 --memory-limit 262144 main.cpp --input test.txt

# Compile only
silicube compile --language rust solution.rs
```

## Global Options

| Flag                  | Description                 |
|-----------------------|-----------------------------|
| `-c, --config <PATH>` | Path to configuration file  |
| `-b, --box-id <ID>`   | Isolate box ID (default: 0) |
| `-v, --verbose`       | Enable debug logging        |

## Requirements

- Linux with `isolate` installed
- Root privileges or equivalent capabilities
- Language toolchains for the languages you want to run
