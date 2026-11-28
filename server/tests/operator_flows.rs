//! The operator's interfaces as deployed: the CLI binary and a running server
//! on the same database file, with requests over real TCP.

mod common;

use common::*;
use tempfile::TempDir;

/// TC-024
#[tokio::test]
async fn tc_024_no_arguments_starts_the_server_on_a_missing_database() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");
    let (device_port, panel_port) = (free_port(), free_port());
    let server = Cli::database(&path)
        .env("ATALAYA_DEVICE_PORT", device_port.to_string())
        .env("ATALAYA_PANEL_PORT", panel_port.to_string())
        .start_server();
    assert!(server.start_line.is_some(), "the server logs its start");

    let health = [
        status_line_over_tcp(
            ([127, 0, 0, 1], device_port).into(),
            "GET",
            "/healthz",
            &[],
            "",
        )
        .await,
        status_line_over_tcp(
            ([127, 0, 0, 1], panel_port).into(),
            "GET",
            "/healthz",
            &[],
            "",
        )
        .await,
    ];

    assert_eq!(health, ["HTTP/1.1 200 OK", "HTTP/1.1 200 OK"]);
    assert!(path.exists());
    assert_eq!(server.terminate(), Some(0));
}

/// TC-038
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tc_038_a_device_enrolled_while_the_server_runs_is_accepted_at_once() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");
    let server = RunningServer::start(&path).await;
    assert_eq!(
        post_fix_over_tcp(server.device, "phone-1", FP_A).await,
        "HTTP/1.1 401 Unauthorized",
        "an unknown device is rejected"
    );

    let outcome = Cli::database(&path).run(&["device", "add", "phone-1", NAME, FP_A]);
    let t0 = now_ms();
    let status = post_fix_over_tcp(server.device, "phone-1", FP_A).await;
    let t1 = now_ms();

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(outcome.confirms("phone-1"), "{outcome:?}");
    assert_eq!(status, "HTTP/1.1 201 Created");
    let stored: Vec<_> = read_fixes(&path, "phone-1")
        .await
        .into_iter()
        .map(|f| (f.lat, f.lon, f.acc, f.ts, f.prov))
        .collect();
    assert_eq!(
        stored,
        vec![(40.0, -3.0, 12.5, 1_700_000_000_000, "fused".to_string())]
    );
    let last_contact = read_device(&path, "phone-1")
        .await
        .and_then(|d| d.last_contact)
        .unwrap_or_default();
    assert!(
        (t0..=t1).contains(&last_contact),
        "last contact {last_contact} not in [{t0}, {t1}]"
    );
    server.stop().await;
}

/// A running server with `phone-1` enrolled with `FP_A` and one fix accepted
/// over TCP.
async fn running_with_one_fix(path: &std::path::Path) -> RunningServer {
    let server = RunningServer::start(path).await;
    Cli::database(path).enrol("phone-1", NAME, FP_A);
    assert_eq!(
        post_fix_over_tcp(server.device, "phone-1", FP_A).await,
        "HTTP/1.1 201 Created",
        "the first fix is accepted"
    );
    server
}

/// TC-039
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tc_039_a_replaced_fingerprint_applies_without_restarting() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");
    let server = running_with_one_fix(&path).await;

    let outcome = Cli::database(&path).run(&["device", "set-fingerprint", "phone-1", FP_B]);
    let statuses = [
        post_fix_over_tcp(server.device, "phone-1", FP_B).await,
        post_fix_over_tcp(server.device, "phone-1", FP_A).await,
    ];

    assert!(outcome.succeeded(), "{outcome:?}");
    assert!(outcome.confirms("phone-1"), "{outcome:?}");
    assert_eq!(
        statuses,
        ["HTTP/1.1 201 Created", "HTTP/1.1 401 Unauthorized"]
    );
    assert_eq!(read_fixes(&path, "phone-1").await.len(), 2);
    server.stop().await;
}

/// TC-040
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tc_040_a_revocation_applies_without_restarting_and_keeps_the_fixes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");
    let server = running_with_one_fix(&path).await;
    let l1 = read_device(&path, "phone-1").await.unwrap().last_contact;
    let cli = Cli::database(&path);

    let revoke = cli.run(&["device", "revoke", "phone-1"]);
    let revoked = post_fix_over_tcp(server.device, "phone-1", FP_A).await;

    assert!(revoke.succeeded(), "{revoke:?}");
    assert!(revoke.confirms("phone-1"), "{revoke:?}");
    assert_eq!(revoked, "HTTP/1.1 401 Unauthorized");
    assert_eq!(read_fixes(&path, "phone-1").await.len(), 1);
    assert_eq!(
        read_device(&path, "phone-1").await.unwrap().last_contact,
        l1
    );

    let reassign = cli.run(&["device", "set-fingerprint", "phone-1", FP_B]);
    let reassigned = post_fix_over_tcp(server.device, "phone-1", FP_B).await;

    assert!(reassign.succeeded(), "{reassign:?}");
    assert_eq!(reassigned, "HTTP/1.1 201 Created");
    assert_eq!(read_fixes(&path, "phone-1").await.len(), 2);
    server.stop().await;
}

/// TC-041
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tc_041_the_panel_listener_has_no_telemetry_route() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atalaya.db");
    let server = RunningServer::start(&path).await;
    Cli::database(&path).enrol("phone-1", NAME, FP_A);

    let status = post_fix_over_tcp(server.panel, "phone-1", FP_A).await;

    assert_eq!(status, "HTTP/1.1 404 Not Found");
    assert!(read_fixes(&path, "phone-1").await.is_empty());
    assert_eq!(
        read_device(&path, "phone-1").await.unwrap().last_contact,
        None
    );
    server.stop().await;
}
