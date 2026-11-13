use clap::{Parser, Subcommand};
use parity::{ram, wpt};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "parity")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ram {
        #[arg(long)]
        browser: String,
        #[arg(long)]
        workload: String,
        #[arg(long)]
        out: PathBuf,
    },
    WptDiff {
        servo: PathBuf,
        chrome: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Ram {
            browser,
            workload,
            out,
        } => {
            ram::run_ram(&browser, &workload, &out)?;
        }
        Commands::WptDiff { servo, chrome, out } => {
            let s = wpt::read_report(&servo)?;
            let c = wpt::read_report(&chrome)?;
            let md = wpt::diff_markdown(&s, &c);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&out, &md)?;
            println!("{}", md);
        }
    }
    Ok(())
}
