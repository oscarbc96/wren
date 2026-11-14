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

    /// Start Firefox DevTools remote debugging on 127.0.0.1:<port>
    /// (default port from settings, 6000).
    #[arg(long, value_name = "PORT", num_args = 0..=1, default_missing_value = "0")]
    pub devtools: Option<u16>,

    /// Delay in milliseconds before capturing the screenshot.
    #[arg(long, default_value_t = 3000)]
    pub screenshot_delay_ms: u64,

    /// Write a JSON memory report to this path after loading and exit.
    #[arg(long, value_name = "PATH")]
    pub memory_report: Option<String>,
}

pub fn parse() -> Args {
    Args::parse()
}
