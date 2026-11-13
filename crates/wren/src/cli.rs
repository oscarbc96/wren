use clap::Parser;

/// Wren browser command line interface.
#[derive(Debug, Clone, Parser)]
#[command(name = "wren")]
pub struct Args {
    /// Profile directory. Sets both config and data dir.
    #[arg(long)]
    pub profile_dir: Option<String>,

    /// URLs to open.
    #[arg(value_name = "URL")]
    pub urls: Vec<String>,
}

pub fn parse() -> Args {
    Args::parse()
}
