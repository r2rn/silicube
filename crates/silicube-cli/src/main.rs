//! Silicube Runner CLI
//!
//! A command-line tool for running code in IOI isolate sandboxes.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use silicube::{BoxPool, Config, EXAMPLE_CONFIG, ResourceLimits, Runner, prepare_cgroup};
use tracing::{Level, debug, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "silicube")]
#[command(about = "A tool for orchestrating sandboxed code execution")]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Box ID to use (default: 0)
    #[arg(short = 'b', long, global = true, default_value = "0")]
    box_id: u32,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new configuration file
    Init {
        /// Output path (default: silicube.toml)
        #[arg(short, long, default_value = "silicube.toml")]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Compile source code
    Compile {
        /// Source file to compile
        #[arg(value_name = "FILE")]
        source: PathBuf,

        /// Language ID (e.g., cpp17, rust, java)
        #[arg(short, long)]
        language: String,

        /// Time limit in seconds
        #[arg(short, long)]
        time_limit: Option<f64>,

        /// Memory limit in KB
        #[arg(short, long)]
        memory_limit: Option<u64>,
    },

    /// Run a program (compile if needed, then execute)
    Run {
        /// Source file to run
        #[arg(value_name = "FILE")]
        source: PathBuf,

        /// Language ID (e.g., cpp17, python3)
        #[arg(short, long)]
        language: String,

        /// Input file (default: stdin)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Time limit in seconds
        #[arg(short, long)]
        time_limit: Option<f64>,

        /// Memory limit in KB
        #[arg(short, long)]
        memory_limit: Option<u64>,
    },

    /// List available languages
    Languages,

    /// Show default configuration
    ShowConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::from_default_env().add_directive(Level::DEBUG.into())
    } else {
        EnvFilter::from_default_env().add_directive(Level::INFO.into())
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Load configuration
    let mut config = if let Some(ref path) = cli.config {
        info!(?path, "loading configuration");
        Config::from_file(path).context("failed to load configuration")?
    } else {
        debug!("using default configuration");
        Config::default()
    };

    // Set up cgroup hierarchy if cgroup mode is enabled
    if config.cgroup {
        match prepare_cgroup(&config.cg_root) {
            Ok(true) => debug!("cgroup hierarchy ready"),
            Ok(false) => {
                warn!(
                    "cgroup support unavailable (memory controller not found), falling back to RLIMIT_AS"
                );
                config.cgroup = false;
            }
            Err(e) => {
                warn!("cgroup setup failed: {e}, falling back to RLIMIT_AS memory limiting");
                config.cgroup = false;
            }
        }
    }

    match cli.command {
        Commands::Init { output, force } => {
            return init_config(&output, force).await;
        }
        Commands::Compile {
            source,
            language,
            time_limit,
            memory_limit,
        } => {
            run_compile(
                &config,
                cli.box_id,
                &source,
                &language,
                time_limit,
                memory_limit,
            )
            .await
        }
        Commands::Run {
            source,
            language,
            input,
            time_limit,
            memory_limit,
        } => {
            run_execute(
                &config,
                cli.box_id,
                &source,
                &language,
                input.as_deref(),
                time_limit,
                memory_limit,
            )
            .await
        }
        Commands::Languages => {
            list_languages(&config);
            Ok(())
        }
        Commands::ShowConfig => {
            show_config(&config);
            Ok(())
        }
    }
}

async fn run_compile(
    config: &Config,
    box_id: u32,
    source: &PathBuf,
    language_id: &str,
    time_limit: Option<f64>,
    memory_limit: Option<u64>,
) -> Result<()> {
    let language = config
        .get_language(language_id)
        .context("unknown language")?;

    if !language.is_compiled() {
        println!("Language '{}' does not require compilation", language.name);
        return Ok(());
    }

    let source_content = tokio::fs::read(source)
        .await
        .context("failed to read source file")?;

    info!(language = %language.name, "compiling source");

    // Create sandbox
    let pool = BoxPool::new(box_id, 1, config.isolate_binary(), config.cgroup);
    let mut sandbox = pool.acquire().await.context("failed to acquire sandbox")?;

    // Create limits (only include explicitly-specified values so they don't
    // override per-language defaults)
    let user_limits = ResourceLimits {
        time_limit,
        memory_limit,
        wall_time_limit: None,
        stack_limit: None,
        max_processes: None,
        max_output: None,
        max_open_files: None,
        extra_time: None,
    };
    let has_user_limits = time_limit.is_some() || memory_limit.is_some();

    // Compile
    let runner = Runner::new(config.clone());
    let result = runner
        .compile(
            &sandbox,
            &source_content,
            language,
            if has_user_limits {
                Some(&user_limits)
            } else {
                None
            },
        )
        .await
        .context("compilation failed")?;

    sandbox
        .cleanup()
        .await
        .context("failed to cleanup sandbox")?;

    if result.success {
        println!("Compilation successful");
        println!("Time: {:.3}s", result.execution.time);
        println!("Memory: {} KB", result.execution.memory);
    } else {
        println!("Compilation failed");
        println!("Exit code: {:?}", result.execution.exit_code);
        if !result.output.is_empty() {
            println!("\nCompiler output:\n{}", result.output);
        }
        std::process::exit(1);
    }

    Ok(())
}

async fn run_execute(
    config: &Config,
    box_id: u32,
    source: &PathBuf,
    language_id: &str,
    input: Option<&std::path::Path>,
    time_limit: Option<f64>,
    memory_limit: Option<u64>,
) -> Result<()> {
    let language = config
        .get_language(language_id)
        .context("unknown language")?;

    let source_content = tokio::fs::read(source)
        .await
        .context("failed to read source file")?;

    let input_data = if let Some(input_path) = input {
        Some(
            tokio::fs::read(input_path)
                .await
                .context("failed to read input file")?,
        )
    } else {
        None
    };

    info!(language = %language.name, "running program");

    // Create sandbox
    let pool = BoxPool::new(box_id, 1, config.isolate_binary(), config.cgroup);
    let mut sandbox = pool.acquire().await.context("failed to acquire sandbox")?;

    // Create limits (only include explicitly-specified values so they don't
    // override per-language defaults)
    let user_limits = ResourceLimits {
        time_limit,
        memory_limit,
        wall_time_limit: None,
        stack_limit: None,
        max_processes: None,
        max_output: None,
        max_open_files: None,
        extra_time: None,
    };
    let has_user_limits = time_limit.is_some() || memory_limit.is_some();
    let limits_ref = if has_user_limits {
        Some(&user_limits)
    } else {
        None
    };

    let runner = Runner::new(config.clone());

    // Compile if needed
    if language.is_compiled() {
        info!("compiling source");
        let compile_result = runner
            .compile(&sandbox, &source_content, language, None)
            .await
            .context("compilation failed")?;

        if !compile_result.success {
            sandbox
                .cleanup()
                .await
                .context("failed to cleanup sandbox")?;
            eprintln!("Compilation failed:");
            eprintln!("{}", compile_result.output);
            std::process::exit(1);
        }

        debug!(time = compile_result.execution.time, "compilation complete");
    } else {
        // Write source for interpreted language
        sandbox
            .write_file(&language.source_name(), &source_content)
            .await
            .context("failed to write source to sandbox")?;
    }

    // Run
    info!("executing program");
    let result = runner
        .run(&sandbox, input_data.as_deref(), language, limits_ref)
        .await
        .context("execution failed")?;

    sandbox
        .cleanup()
        .await
        .context("failed to cleanup sandbox")?;

    // Output results
    if let Some(stdout) = &result.stdout {
        let output = String::from_utf8_lossy(stdout);
        println!("{output}");
    }

    if let Some(stderr) = &result.stderr {
        let err = String::from_utf8_lossy(stderr);
        if !err.is_empty() {
            eprintln!("{err}");
        }
    }

    // Log execution info via tracing (stderr), keeping stdout clean for piping
    info!(
        status = ?result.status,
        time = format_args!("{:.3}s", result.time),
        wall_time = format_args!("{:.3}s", result.wall_time),
        memory = format_args!("{} KB", result.memory),
        exit_code = result.exit_code,
        signal = result.signal,
        "execution result"
    );

    // Exit with appropriate code
    if result.is_success() {
        Ok(())
    } else {
        std::process::exit(result.exit_code.unwrap_or(1));
    }
}

fn list_languages(config: &Config) {
    println!("Available languages:\n");

    let mut languages: Vec<_> = config.languages.iter().collect();
    languages.sort_by_key(|(id, _)| *id);

    for (id, lang) in languages {
        let lang_type = if lang.is_compiled() {
            "compiled"
        } else {
            "interpreted"
        };
        println!("  {:<15} {} ({})", id, lang.name, lang_type);
    }
}

fn show_config(config: &Config) {
    println!("Default resource limits:");
    println!("  Time limit: {:?}", config.default_limits.time_limit);
    println!(
        "  Wall time limit: {:?}",
        config.default_limits.wall_time_limit
    );
    println!(
        "  Memory limit: {:?} KB",
        config.default_limits.memory_limit
    );
    println!("  Stack limit: {:?} KB", config.default_limits.stack_limit);
    println!("  Max processes: {:?}", config.default_limits.max_processes);
    println!();
    println!("Isolate binary: {}", config.isolate_binary().display());
    println!();
    println!("Languages configured: {}", config.languages.len());
}

async fn init_config(output: &PathBuf, force: bool) -> Result<()> {
    if output.exists() && !force {
        anyhow::bail!(
            "Configuration file already exists at '{}'. Use --force to overwrite.",
            output.display()
        );
    }

    tokio::fs::write(output, EXAMPLE_CONFIG)
        .await
        .context("failed to write configuration file")?;

    println!("Created configuration file at '{}'", output.display());
    Ok(())
}
