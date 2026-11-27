use std::process::ExitCode;

use atalaya_server::{Config, Server};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("atalaya-server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let server = Server::bind(&config).await?;
    eprintln!(
        "atalaya-server: device API on {}, panel on {}, database {}",
        server.device_addr(),
        server.panel_addr(),
        config.database_path.display()
    );
    server.run(shutdown_signal()).await?;
    Ok(())
}

/// Completes on Ctrl-C or, under systemd, on SIGTERM.
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).expect("SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}
