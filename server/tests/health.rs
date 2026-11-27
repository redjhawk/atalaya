//! The server's public behavior at its HTTP boundary, against a temporary
//! SQLite database. Later tickets follow this pattern.

use std::net::SocketAddr;

use atalaya_server::{Config, Server, device_router, open_database, panel_router};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tower::ServiceExt;

fn config(dir: &TempDir) -> Config {
    Config::from_lookup(|name| match name {
        "ATALAYA_DEVICE_PORT" | "ATALAYA_PANEL_PORT" => Some("0".into()),
        "ATALAYA_DATABASE_PATH" => Some(dir.path().join("atalaya.db").display().to_string()),
        _ => None,
    })
    .unwrap()
}

async fn health_status(router: Router) -> StatusCode {
    let request = Request::get("/healthz").body(Body::empty()).unwrap();
    router.oneshot(request).await.unwrap().status()
}

/// Sends a plain HTTP/1.1 request over TCP and returns the status line.
async fn status_line_over_tcp(addr: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.lines().next().unwrap_or_default().to_string()
}

#[tokio::test]
async fn both_routers_report_healthy_when_the_database_answers() {
    let dir = TempDir::new().unwrap();
    let pool = open_database(&dir.path().join("atalaya.db")).await.unwrap();

    assert_eq!(
        health_status(device_router(pool.clone())).await,
        StatusCode::OK
    );
    assert_eq!(health_status(panel_router(pool)).await, StatusCode::OK);
}

#[tokio::test]
async fn health_fails_when_the_database_is_unavailable() {
    let dir = TempDir::new().unwrap();
    let pool = open_database(&dir.path().join("atalaya.db")).await.unwrap();
    pool.close().await;

    assert_eq!(
        health_status(device_router(pool.clone())).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        health_status(panel_router(pool)).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn startup_creates_the_database_and_can_reopen_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");
    assert!(!path.exists());

    open_database(&path).await.unwrap().close().await;
    assert!(path.exists());

    // A second start finds the migrations already applied.
    let pool = open_database(&path).await.unwrap();
    assert_eq!(health_status(device_router(pool)).await, StatusCode::OK);
}

#[tokio::test]
async fn server_answers_on_two_separate_loopback_listeners() {
    let dir = TempDir::new().unwrap();
    let server = Server::bind(&config(&dir)).await.unwrap();
    let (device, panel) = (server.device_addr(), server.panel_addr());

    assert!(device.ip().is_loopback());
    assert!(panel.ip().is_loopback());
    assert_ne!(device.port(), panel.port());

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(server.run(async {
        let _ = stopped.await;
    }));

    assert_eq!(
        status_line_over_tcp(device, "/healthz").await,
        "HTTP/1.1 200 OK"
    );
    assert_eq!(
        status_line_over_tcp(panel, "/healthz").await,
        "HTTP/1.1 200 OK"
    );

    stop.send(()).unwrap();
    running.await.unwrap().unwrap();
}

#[tokio::test]
async fn unknown_paths_are_not_found() {
    let dir = TempDir::new().unwrap();
    let pool = open_database(&dir.path().join("atalaya.db")).await.unwrap();
    let request = Request::get("/").body(Body::empty()).unwrap();

    let response = device_router(pool).oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
