use std::error::Error;

use clap::{Args, Parser, Subcommand, ValueEnum};
use easyreg_core::{AnalyzeRequest, MatchMode};
use easyreg_engine::analyze;

#[derive(Debug, Parser)]
#[command(name = "easyreg", version, about = "Example-driven regular expression inference")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Infer strict, balanced, and flexible expressions from examples.
    Infer(InferArgs),
}

#[derive(Debug, Args)]
struct InferArgs {
    /// An example that must match. Repeat this option for multiple examples.
    #[arg(short = 'p', long = "positive", required = true)]
    positive_examples: Vec<String>,

    /// An example that must not match. Repeat this option as needed.
    #[arg(short = 'n', long = "negative")]
    negative_examples: Vec<String>,

    /// Match the complete input or search within it.
    #[arg(long, value_enum, default_value_t = CliMatchMode::Full)]
    mode: CliMatchMode,

    /// Emit compact JSON rather than pretty-printed JSON.
    #[arg(long)]
    compact: bool,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum CliMatchMode {
    #[default]
    Full,
    Search,
}

impl From<CliMatchMode> for MatchMode {
    fn from(value: CliMatchMode) -> Self {
        match value {
            CliMatchMode::Full => Self::Full,
            CliMatchMode::Search => Self::Search,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Infer(args) => {
            let result = analyze(&AnalyzeRequest {
                positive_examples: args.positive_examples,
                negative_examples: args.negative_examples,
                match_mode: args.mode.into(),
            })?;
            let output = if args.compact {
                serde_json::to_string(&result)?
            } else {
                serde_json::to_string_pretty(&result)?
            };
            println!("{output}");
        }
    }

    Ok(())
}
