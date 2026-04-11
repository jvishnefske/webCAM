use std::path::PathBuf;

use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

/// Native development server for the RustCAM frontend.
#[derive(Parser, Debug)]
#[command(name = "native-server")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Path to the www directory to serve.
    #[arg(long, default_value = "www")]
    www_dir: PathBuf,

    /// Don't open browser on startup.
    #[arg(long)]
    no_open: bool,
}

#[cfg_attr(not(test), tokio::main)]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let router = native_server::app(&args.www_dir);

    let addr = format!("0.0.0.0:{}", args.port);
    let url = format!("http://localhost:{}", args.port);
    info!("Serving {} on {}", args.www_dir.display(), url);

    let listener = TcpListener::bind(&addr).await?;

    if !args.no_open {
        let url_clone = url.clone();
        tokio::spawn(async move {
            // Small delay so the server is ready before the browser hits it
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let _ = open_browser(&url_clone);
        });
    }

    axum::serve(listener, router).await?;

    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn open_browser(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", url]).spawn()?;
    }
    Ok(())
}
