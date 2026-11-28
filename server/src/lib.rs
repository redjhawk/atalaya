//! Atalaya server: the device API and the panel, on two separate listeners.

pub mod config;

use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderName, StatusCode};
use axum::routing::get;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tokio::net::TcpListener;
use tokio::sync::watch;

pub use config::{Config, ConfigError};

/// Opens the SQLite database, creating the file when missing, and applies
/// every pending migration.
pub async fn open_database(path: &Path) -> Result<SqlitePool, StartError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .map_err(StartError::Database)?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(StartError::Migration)?;
    Ok(pool)
}

/// Routes served to the device, behind mTLS at the edge. The edge forwards
/// the client certificate fingerprint in `fingerprint_header` (ADR 0003).
pub fn device_router(pool: SqlitePool, _fingerprint_header: HeaderName) -> Router {
    // Stub: `POST /telemetry/{device}` is not implemented yet.
    Router::new()
        .route("/healthz", get(health))
        .with_state(pool)
}

/// Routes served to the panel, behind an interactive login at the edge.
pub fn panel_router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .with_state(pool)
}

/// Healthy only when the database answers a query.
async fn health(State(pool): State<SqlitePool>) -> (StatusCode, &'static str) {
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => (StatusCode::OK, "ok\n"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "database unavailable\n"),
    }
}

#[derive(Debug)]
pub enum StartError {
    Database(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
    Bind { port: u16, source: std::io::Error },
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(e) => write!(f, "cannot open the database: {e}"),
            Self::Migration(e) => write!(f, "cannot migrate the database: {e}"),
            Self::Bind { port, source } => write!(f, "cannot listen on port {port}: {source}"),
        }
    }
}

impl std::error::Error for StartError {}

/// A server with its database open and both listeners bound, not yet
/// accepting requests.
pub struct Server {
    device: TcpListener,
    panel: TcpListener,
    pool: SqlitePool,
    fingerprint_header: HeaderName,
}

impl Server {
    /// Opens the database and binds both listeners on the loopback
    /// interface. The server is only ever reached through the tunnel, so
    /// it never listens on another interface.
    pub async fn bind(config: &Config) -> Result<Self, StartError> {
        let pool = open_database(&config.database_path).await?;
        let device = listen(config.device_port).await?;
        let panel = listen(config.panel_port).await?;
        Ok(Self {
            device,
            panel,
            pool,
            fingerprint_header: config.fingerprint_header.clone(),
        })
    }

    pub fn device_addr(&self) -> SocketAddr {
        self.device
            .local_addr()
            .expect("bound listener has an address")
    }

    pub fn panel_addr(&self) -> SocketAddr {
        self.panel
            .local_addr()
            .expect("bound listener has an address")
    }

    /// Serves both listeners until `shutdown` completes, then lets
    /// in-flight requests finish and closes the database.
    pub async fn run(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let (stop, stopped) = watch::channel(());
        tokio::spawn(async move {
            shutdown.await;
            drop(stop);
        });

        let device = serve(
            self.device,
            device_router(self.pool.clone(), self.fingerprint_header),
            stopped.clone(),
        );
        let panel = serve(self.panel, panel_router(self.pool.clone()), stopped);
        let result = tokio::try_join!(device, panel).map(|_| ());
        self.pool.close().await;
        result
    }
}

async fn listen(port: u16) -> Result<TcpListener, StartError> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|source| StartError::Bind { port, source })
}

async fn serve(
    listener: TcpListener,
    router: Router,
    mut stopped: watch::Receiver<()>,
) -> std::io::Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            // Resolves when the sender is dropped.
            let _ = stopped.changed().await;
        })
        .await
}
