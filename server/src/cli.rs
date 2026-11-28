//! The command line of the `atalaya-server` binary: no arguments starts the
//! server, the `device` subcommands enrol a device and replace or revoke its
//! fingerprint. A subcommand opens the database only, never a listener.

use std::ffi::OsString;
use std::fmt;

use crate::devices::{
    self, DeviceId, DeviceName, EnrolError, Fingerprint, FingerprintChange, InputError,
};
use crate::{Config, StartError, open_database};

pub const USAGE: &str = "\
usage:
  atalaya-server                                         start the server
  atalaya-server device add <id> <name> <fingerprint>
  atalaya-server device set-fingerprint <id> <fingerprint>
  atalaya-server device revoke <id>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    Device(DeviceCommand),
}

/// A `device` subcommand, with its validated values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceCommand {
    Add {
        id: DeviceId,
        name: DeviceName,
        fingerprint: Fingerprint,
    },
    SetFingerprint {
        id: DeviceId,
        fingerprint: Fingerprint,
    },
    Revoke {
        id: DeviceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// Unknown subcommand, wrong number of arguments or a non-UTF-8 one.
    Usage,
    Input(InputError),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(f, "invalid arguments\n{USAGE}"),
            Self::Input(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CliError {}

impl From<InputError> for CliError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

/// Parses the arguments after the program name. Every value is validated
/// here, before anything is opened.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, CliError> {
    let args = args
        .into_iter()
        .map(OsString::into_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CliError::Usage)?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let command = match args.as_slice() {
        [] => return Ok(Command::Serve),
        ["device", "add", id, name, fingerprint] => DeviceCommand::Add {
            id: DeviceId::parse(id)?,
            name: DeviceName::parse(name)?,
            fingerprint: Fingerprint::parse(fingerprint)?,
        },
        ["device", "set-fingerprint", id, fingerprint] => DeviceCommand::SetFingerprint {
            id: DeviceId::parse(id)?,
            fingerprint: Fingerprint::parse(fingerprint)?,
        },
        ["device", "revoke", id] => DeviceCommand::Revoke {
            id: DeviceId::parse(id)?,
        },
        _ => return Err(CliError::Usage),
    };
    Ok(Command::Device(command))
}

/// Why a subcommand failed after its arguments were accepted.
#[derive(Debug)]
pub enum RunError {
    Start(StartError),
    Enrol(EnrolError),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(e) => e.fmt(f),
            Self::Enrol(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for RunError {}

/// Runs one device subcommand on the configured database, creating and
/// migrating it when missing, and returns the confirmation line.
pub async fn run(command: DeviceCommand, config: &Config) -> Result<String, RunError> {
    let pool = open_database(&config.database_path)
        .await
        .map_err(RunError::Start)?;
    let result = execute(&pool, command).await;
    pool.close().await;
    result.map_err(RunError::Enrol)
}

async fn execute(pool: &sqlx::SqlitePool, command: DeviceCommand) -> Result<String, EnrolError> {
    match command {
        DeviceCommand::Add {
            id,
            name,
            fingerprint,
        } => {
            devices::add(pool, &id, &name, &fingerprint).await?;
            Ok(format!("device {id} enrolled"))
        }
        DeviceCommand::SetFingerprint { id, fingerprint } => {
            let line = match devices::set_fingerprint(pool, &id, &fingerprint).await? {
                FingerprintChange::Replaced => format!("device {id} fingerprint replaced"),
                FingerprintChange::Unchanged => format!("device {id} fingerprint unchanged"),
            };
            Ok(line)
        }
        DeviceCommand::Revoke { id } => {
            devices::revoke(pool, &id).await?;
            Ok(format!("device {id} revoked"))
        }
    }
}
