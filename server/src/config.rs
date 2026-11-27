//! Runtime configuration, read from environment variables.

use std::fmt;
use std::path::PathBuf;

use axum::http::HeaderName;

pub const DEVICE_PORT: &str = "ATALAYA_DEVICE_PORT";
pub const PANEL_PORT: &str = "ATALAYA_PANEL_PORT";
pub const DATABASE_PATH: &str = "ATALAYA_DATABASE_PATH";
pub const FINGERPRINT_HEADER: &str = "ATALAYA_FINGERPRINT_HEADER";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Port of the device API listener. `0` asks the OS for a free port.
    pub device_port: u16,
    /// Port of the panel listener. `0` asks the OS for a free port.
    pub panel_port: u16,
    /// SQLite database file. Created on startup when missing.
    pub database_path: PathBuf,
    /// Request header in which the edge forwards the client certificate
    /// fingerprint (ADR 0003).
    pub fingerprint_header: HeaderName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidPort {
        variable: &'static str,
        value: String,
    },
    InvalidHeaderName {
        value: String,
    },
    /// The device API and the panel must never share a listener: the
    /// fingerprint header is only trusted on the device one.
    SharedPort {
        port: u16,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPort { variable, value } => {
                write!(f, "{variable} must be a port number, got {value:?}")
            }
            Self::InvalidHeaderName { value } => {
                write!(
                    f,
                    "{FINGERPRINT_HEADER} must be a valid header name, got {value:?}"
                )
            }
            Self::SharedPort { port } => {
                write!(
                    f,
                    "{DEVICE_PORT} and {PANEL_PORT} must differ, both are {port}"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the configuration from any variable source. Unset variables
    /// take their default.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let device_port = port(&lookup, DEVICE_PORT, 8080)?;
        let panel_port = port(&lookup, PANEL_PORT, 8081)?;
        if device_port == panel_port && device_port != 0 {
            return Err(ConfigError::SharedPort { port: device_port });
        }

        let database_path = lookup(DATABASE_PATH)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("atalaya.db"));

        let header = lookup(FINGERPRINT_HEADER).unwrap_or_else(|| "x-client-cert-sha256".into());
        let fingerprint_header = HeaderName::try_from(header.as_str())
            .map_err(|_| ConfigError::InvalidHeaderName { value: header })?;

        Ok(Self {
            device_port,
            panel_port,
            database_path,
            fingerprint_header,
        })
    }
}

fn port(
    lookup: &impl Fn(&str) -> Option<String>,
    variable: &'static str,
    default: u16,
) -> Result<u16, ConfigError> {
    match lookup(variable) {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| ConfigError::InvalidPort { variable, value }),
    }
}
