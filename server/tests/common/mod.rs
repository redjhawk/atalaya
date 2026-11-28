//! Shared helpers for the integration tests: the example values of the test
//! cases, a runner for the `atalaya-server` binary, readers of the database
//! file, and request builders for the routers and for real TCP listeners.
//!
//! Every value here is fake: fingerprints are one repeated hex digit and
//! locations are round numbers.

#![allow(dead_code)]

use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use atalaya_server::{Config, Server, device_router, open_database, panel_router};
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderName, Method, Request, StatusCode};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Row, SqliteConnection, SqlitePool};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tower::ServiceExt;

/// 64 × `a`.
pub const FP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// 64 × `b`.
pub const FP_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// 64 × `c`.
pub const FP_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
/// 64 × `f`, enrolled to no device.
pub const FP_F: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
/// 64 × `A`, the uppercase form of `FP_A`.
pub const FP_A_UPPER: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

pub const NAME: &str = "Móvil de ejemplo";

/// A valid fix body.
pub const VALID: &str =
    r#"{"location":{"lat":40.0,"lon":-3.0,"acc":12.5,"ts":1700000000000,"prov":"fused"}}"#;

/// The default fingerprint header.
pub const HEADER: &str = "x-client-cert-sha256";

/// How long a CLI call may take before the test kills it and fails.
pub const DEADLINE: Duration = Duration::from_secs(10);

/// A 64-character fingerprint made of one hex digit.
pub fn hex64(digit: char) -> String {
    digit.to_string().repeat(64)
}

pub fn default_header() -> HeaderName {
    HeaderName::from_static(HEADER)
}

/// Wall clock in Unix milliseconds, to bracket a request with `t0`/`t1`.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

// ---------------------------------------------------------------------------
// The CLI binary
// ---------------------------------------------------------------------------

/// What a finished CLI process left behind.
#[derive(Debug)]
pub struct Outcome {
    /// `None` when the process was killed (by a signal or on the deadline).
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// The process did not exit by itself within `DEADLINE`.
    pub timed_out: bool,
}

impl Outcome {
    pub fn succeeded(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }

    /// Exited by itself with a non-zero code.
    pub fn failed(&self) -> bool {
        !self.timed_out && self.code.is_some_and(|code| code != 0)
    }

    /// The only stdout line, when stdout is exactly one line.
    pub fn single_line(&self) -> Option<&str> {
        let mut lines = self.stdout.lines();
        match (lines.next(), lines.next()) {
            (Some(line), None) => Some(line),
            _ => None,
        }
    }

    /// Stdout is exactly one line and it contains `text`.
    pub fn confirms(&self, text: &str) -> bool {
        self.single_line().is_some_and(|line| line.contains(text))
    }
}

/// A way to run `atalaya-server` with a controlled environment: every
/// `ATALAYA_*` variable of the test process is removed, then the ones of the
/// case are set.
#[derive(Clone, Default)]
pub struct Cli {
    vars: Vec<(String, OsString)>,
    dir: Option<PathBuf>,
}

impl Cli {
    pub fn new() -> Self {
        Self::default()
    }

    /// A runner whose `ATALAYA_DATABASE_PATH` is `path`.
    pub fn database(path: &Path) -> Self {
        Self::new().env("ATALAYA_DATABASE_PATH", path)
    }

    pub fn env(mut self, name: &str, value: impl Into<OsString>) -> Self {
        self.vars.push((name.to_string(), value.into()));
        self
    }

    pub fn current_dir(mut self, dir: &Path) -> Self {
        self.dir = Some(dir.to_path_buf());
        self
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_atalaya-server"));
        command.args(args);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("ATALAYA_") {
                command.env_remove(name);
            }
        }
        for (name, value) in &self.vars {
            command.env(name, value);
        }
        if let Some(dir) = &self.dir {
            command.current_dir(dir);
        }
        command
    }

    /// Starts the binary without waiting for it.
    pub fn spawn(&self, args: &[&str]) -> Running {
        let child = self
            .command(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the atalaya-server binary starts");
        let pid = child.id();
        let (done, output) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(child.wait_with_output());
        });
        Running {
            pid,
            output,
            deadline: Instant::now() + DEADLINE,
        }
    }

    /// Runs the binary to completion, or kills it after `DEADLINE`.
    pub fn run(&self, args: &[&str]) -> Outcome {
        self.spawn(args).finish()
    }

    /// Runs a command that must succeed, as the "Given" of a case.
    pub fn expect_success(&self, args: &[&str]) {
        let outcome = self.run(args);
        assert!(outcome.succeeded(), "{args:?} must succeed: {outcome:?}");
    }

    pub fn enrol(&self, id: &str, name: &str, fingerprint: &str) {
        self.expect_success(&["device", "add", id, name, fingerprint]);
    }

    pub fn revoke(&self, id: &str) {
        self.expect_success(&["device", "revoke", id]);
    }

    /// Starts the server (no arguments) and returns once it has logged its
    /// start line, that is once both listeners are bound.
    pub fn start_server(&self) -> ServerProcess {
        let mut child = self
            .command(&[])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the atalaya-server binary starts");
        let stderr = child.stderr.take().unwrap();
        let (first, line) = mpsc::channel();
        // Keeps draining stderr so the server never writes to a closed pipe.
        std::thread::spawn(move || {
            let mut lines = BufReader::new(stderr).lines();
            if let Some(Ok(text)) = lines.next() {
                let _ = first.send(text);
            }
            for _ in lines {}
        });
        let started = line.recv_timeout(DEADLINE);
        ServerProcess {
            child: Some(child),
            start_line: started.ok(),
        }
    }
}

/// A CLI process that has been started.
pub struct Running {
    pid: u32,
    output: mpsc::Receiver<std::io::Result<Output>>,
    deadline: Instant,
}

impl Running {
    pub fn finish(self) -> Outcome {
        let wait = self.deadline.saturating_duration_since(Instant::now());
        let (output, timed_out) = match self.output.recv_timeout(wait) {
            Ok(output) => (output, false),
            Err(_) => {
                signal(self.pid, "KILL");
                (self.output.recv().expect("the waiter reports"), true)
            }
        };
        let output = output.expect("the atalaya-server process can be waited on");
        Outcome {
            code: if timed_out {
                None
            } else {
                output.status.code()
            },
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            timed_out,
        }
    }
}

/// The binary started with no arguments.
pub struct ServerProcess {
    child: Option<Child>,
    /// The first stderr line, `None` when it did not come within `DEADLINE`.
    pub start_line: Option<String>,
}

impl Drop for ServerProcess {
    /// A failed test never leaves a server running.
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl ServerProcess {
    /// Sends `SIGTERM` and returns the exit code, or `None` when the process
    /// did not exit within `DEADLINE` (it is then killed).
    pub fn terminate(mut self) -> Option<i32> {
        let mut child = self.child.take().expect("the server is running");
        let pid = child.id();
        let (done, status) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(child.wait());
        });
        signal(pid, "TERM");
        match status.recv_timeout(DEADLINE) {
            Ok(status) => status.expect("the server can be waited on").code(),
            Err(_) => {
                signal(pid, "KILL");
                None
            }
        }
    }
}

/// The standard library only sends `SIGKILL`, and no signal crate is a
/// dependency, so signals go through `kill`.
fn signal(pid: u32, name: &str) {
    Command::new("kill")
        .arg(format!("-{name}"))
        .arg(pid.to_string())
        .status()
        .expect("kill runs");
}

/// Two loopback ports held open by the test for as long as the value lives,
/// so that a process that tries to listen on them fails.
pub struct HeldPorts {
    device: std::net::TcpListener,
    panel: std::net::TcpListener,
}

impl HeldPorts {
    pub fn new() -> Self {
        Self {
            device: std::net::TcpListener::bind("127.0.0.1:0").unwrap(),
            panel: std::net::TcpListener::bind("127.0.0.1:0").unwrap(),
        }
    }

    /// `cli` with `ATALAYA_DEVICE_PORT` and `ATALAYA_PANEL_PORT` set to the
    /// held ports.
    pub fn apply(&self, cli: Cli) -> Cli {
        cli.env(
            "ATALAYA_DEVICE_PORT",
            self.device.local_addr().unwrap().port().to_string(),
        )
        .env(
            "ATALAYA_PANEL_PORT",
            self.panel.local_addr().unwrap().port().to_string(),
        )
    }
}

/// A loopback port that was free a moment ago.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

// ---------------------------------------------------------------------------
// The temporary database
// ---------------------------------------------------------------------------

/// A temporary database file, migrated by `open_database`, with the pool the
/// routers use.
pub struct Db {
    _dir: TempDir,
    pub path: PathBuf,
    pub pool: SqlitePool,
}

impl Db {
    pub async fn migrated() -> Self {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("atalaya.db");
        let pool = open_database(&path).await.unwrap();
        Self {
            _dir: dir,
            path,
            pool,
        }
    }

    pub fn cli(&self) -> Cli {
        Cli::database(&self.path)
    }

    pub fn device_router(&self) -> Router {
        device_router(self.pool.clone(), default_header())
    }

    pub fn panel_router(&self) -> Router {
        panel_router(self.pool.clone())
    }

    pub async fn device(&self, id: &str) -> Option<DeviceRow> {
        read_device(&self.path, id).await
    }

    pub async fn fixes(&self, id: &str) -> Vec<FixRow> {
        read_fixes(&self.path, id).await
    }

    pub async fn last_contact(&self, id: &str) -> Option<i64> {
        self.device(id)
            .await
            .expect("the device exists")
            .last_contact
    }
}

/// A row of `devices`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub fingerprint: Option<String>,
    pub last_contact: Option<i64>,
}

/// A row of `fixes`, without its own id and device.
#[derive(Debug, Clone, PartialEq)]
pub struct FixRow {
    pub lat: f64,
    pub lon: f64,
    pub acc: f64,
    pub ts: i64,
    pub prov: String,
    pub received_at: i64,
}

async fn connect(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new().filename(path);
    SqliteConnection::connect_with(&options)
        .await
        .expect("the database file opens")
}

fn device_row(row: &sqlx::sqlite::SqliteRow) -> DeviceRow {
    DeviceRow {
        id: row.get("id"),
        name: row.get("name"),
        fingerprint: row.get("fingerprint"),
        last_contact: row.get("last_contact"),
    }
}

/// Every device, ordered by id.
pub async fn read_devices(path: &Path) -> Vec<DeviceRow> {
    let mut connection = connect(path).await;
    let rows = sqlx::query("SELECT id, name, fingerprint, last_contact FROM devices ORDER BY id")
        .fetch_all(&mut connection)
        .await
        .expect("the devices table can be read");
    connection.close().await.unwrap();
    rows.iter().map(device_row).collect()
}

pub async fn read_device(path: &Path, id: &str) -> Option<DeviceRow> {
    read_devices(path)
        .await
        .into_iter()
        .find(|device| device.id == id)
}

/// The fixes of one device, in insertion order.
pub async fn read_fixes(path: &Path, device: &str) -> Vec<FixRow> {
    let mut connection = connect(path).await;
    let rows = sqlx::query(
        "SELECT lat, lon, acc, ts, prov, received_at FROM fixes WHERE device_id = ? ORDER BY id",
    )
    .bind(device)
    .fetch_all(&mut connection)
    .await
    .expect("the fixes table can be read");
    connection.close().await.unwrap();
    rows.iter()
        .map(|row| FixRow {
            lat: row.get("lat"),
            lon: row.get("lon"),
            acc: row.get("acc"),
            ts: row.get("ts"),
            prov: row.get("prov"),
            received_at: row.get("received_at"),
        })
        .collect()
}

/// Number of fixes of every device.
pub async fn count_all_fixes(path: &Path) -> i64 {
    let mut connection = connect(path).await;
    let count = sqlx::query_scalar("SELECT COUNT(*) FROM fixes")
        .fetch_one(&mut connection)
        .await
        .expect("the fixes table can be read");
    connection.close().await.unwrap();
    count
}

// ---------------------------------------------------------------------------
// Requests to the routers
// ---------------------------------------------------------------------------

/// A request with the given headers and body.
pub fn request(method: Method, path: &str, headers: &[(&str, &str)], body: &str) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

/// `POST /telemetry/{device}` with the fingerprint, when given, in the
/// default header.
pub fn fix_request(device: &str, fingerprint: Option<&str>, body: &str) -> Request<Body> {
    let headers: Vec<(&str, &str)> = fingerprint.map(|fp| (HEADER, fp)).into_iter().collect();
    request(
        Method::POST,
        &format!("/telemetry/{device}"),
        &headers,
        body,
    )
}

/// Sends one request to a copy of `router`; returns the status and the body.
pub async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, body.to_vec())
}

/// Sends a fix and returns the status.
pub async fn post_fix(
    router: &Router,
    device: &str,
    fingerprint: Option<&str>,
    body: &str,
) -> StatusCode {
    send(router, fix_request(device, fingerprint, body)).await.0
}

// ---------------------------------------------------------------------------
// A running server and real TCP
// ---------------------------------------------------------------------------

/// A `Server` bound on two free loopback ports and running in this process.
pub struct RunningServer {
    pub device: SocketAddr,
    pub panel: SocketAddr,
    stop: tokio::sync::oneshot::Sender<()>,
    running: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl RunningServer {
    pub async fn start(database: &Path) -> Self {
        let database = database.display().to_string();
        let config = Config::from_lookup(|name| match name {
            "ATALAYA_DEVICE_PORT" | "ATALAYA_PANEL_PORT" => Some("0".into()),
            "ATALAYA_DATABASE_PATH" => Some(database.clone()),
            _ => None,
        })
        .unwrap();
        let server = Server::bind(&config).await.unwrap();
        let (device, panel) = (server.device_addr(), server.panel_addr());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let running = tokio::spawn(server.run(async {
            let _ = stopped.await;
        }));
        Self {
            device,
            panel,
            stop,
            running,
        }
    }

    pub async fn stop(self) {
        self.stop.send(()).unwrap();
        self.running.await.unwrap().unwrap();
    }
}

/// Sends a plain HTTP/1.1 request over TCP and returns the status line.
pub async fn status_line_over_tcp(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.lines().next().unwrap_or_default().to_string()
}

/// `POST /telemetry/{device}` over TCP with the fingerprint in the default
/// header; returns the status line.
pub async fn post_fix_over_tcp(addr: SocketAddr, device: &str, fingerprint: &str) -> String {
    status_line_over_tcp(
        addr,
        "POST",
        &format!("/telemetry/{device}"),
        &[(HEADER, fingerprint), ("content-type", "application/json")],
        VALID,
    )
    .await
}
