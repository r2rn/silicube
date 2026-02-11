# silicube

Async Rust library for sandboxed code execution using [IOI Isolate](https://github.com/ioi/isolate).

## Features

- **Sandboxed execution** — Pool-based lifecycle for running untrusted code safely using Isolate
- **Multi-language** — Supports both compiled and interpreted languages
- **TOML configuration** — Flexible per-language compiler/runtime settings
- **Interactive execution** — FIFO-based sessions for interactive programs
- **Resource limits** — Enforce CPU time, memory, wall time, processes, and output constraints
- **cgroup v2 support** — Memory limiting in container environments

## Usage

```rust
use silicube::{Config, BoxPool, Runner};

let config = Config::default();
let pool = BoxPool::new(0, 1, config.isolate_binary(), config.cgroup);
let sandbox = pool.acquire().await?;

let runner = Runner::new(config.clone());
let language = config.get_language("python3").unwrap();

// Write source and run
sandbox.write_file("solution.py", b"print('hello')").await?;
let result = runner.run(&sandbox, None, language, None).await?;
```

## Requirements

- Linux (Isolate uses kernel namespaces and cgroups)
- Root privileges or equivalent capabilities
- `isolate` binary on `$PATH`
