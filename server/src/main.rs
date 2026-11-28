use std::process::ExitCode;

use atalaya_server::cli::{self, Command, DeviceCommand};
use atalaya_server::{Config, Server};

/// Exit code of a call whose arguments are wrong: usage or input rules.
const USAGE_ERROR: u8 = 2;

#[tokio::main]
async fn main() -> ExitCode {
    let command = match cli::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            report(&error);
            return ExitCode::from(USAGE_ERROR);
        }
    };
    let result = match command {
        Command::Serve => serve().await,
        Command::Device(command) => device(command).await,
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&*error);
            ExitCode::FAILURE
        }
    }
}

/// Writes every line of `error` to stderr with the program-name prefix.
fn report(error: &dyn std::error::Error) {
    for line in error.to_string().lines() {
        eprintln!("atalaya-server: {line}");
    }
}

async fn device(command: DeviceCommand) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let line = cli::run(command, &config).await?;
    println!("{line}");
    Ok(())
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
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
