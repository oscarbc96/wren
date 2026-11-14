use clap::Parser;

/// Wren browser command line interface.
#[derive(Debug, Clone, Parser)]
#[command(name = "wren", version)]
pub struct Args {
    /// Profile directory. Sets both config and data dir.
    #[arg(long)]
    pub profile_dir: Option<String>,

    /// URLs to open.
    #[arg(value_name = "URL")]
    pub urls: Vec<String>,

    /// Capture a screenshot of the window to this PNG path and quit.
    #[arg(long)]
    pub screenshot: Option<String>,

    /// Delay in milliseconds before capturing the screenshot.
    #[arg(long, default_value_t = 3000)]
    pub screenshot_delay_ms: u64,
}

pub fn parse() -> Args {
    Args::parse()
}
