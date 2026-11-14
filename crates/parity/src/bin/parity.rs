use clap::{Parser, Subcommand};
use parity::{ram, topsites, wpt};
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
    Topsites {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        must_work: bool,
        #[arg(long)]
        profile_dir: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
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
        Commands::Topsites {
            out,
            must_work,
            profile_dir,
            limit,
        } => {
            topsites::run_topsites_command(&out, must_work, profile_dir.as_deref(), limit)?;
        }
    }
    Ok(())
}
