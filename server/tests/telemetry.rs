//! `POST /telemetry/{device}` on the device router: identity by the forwarded
//! fingerprint, fix validation and storage, against a temporary database on
//! which devices are enrolled with the CLI binary.

mod common;

use atalaya_server::{Config, device_router};
use axum::http::{Method, StatusCode};
use common::*;
use serde_json::{Value, json};

/// `VALID` with one location field replaced.
fn valid_with(field: &str, value: Value) -> String {
    let mut report: Value = serde_json::from_str(VALID).unwrap();
    report["location"][field] = value;
    report.to_string()
}

/// `VALID` without one location field.
fn valid_without(field: &str) -> String {
    let mut report: Value = serde_json::from_str(VALID).unwrap();
    report["location"].as_object_mut().unwrap().remove(field);
    report.to_string()
}

/// TC-027
#[tokio::test]
async fn tc_027_an_accepted_fix_is_stored_for_its_device_with_the_reception_time() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().enrol("phone-2", "Otro", FP_B);

    let t0 = now_ms();
    let status = post_fix(&db.device_router(), "phone-1", Some(FP_A), VALID).await;
    let t1 = now_ms();

    assert_eq!(status, StatusCode::CREATED);
    let fixes = db.fixes("phone-1").await;
    let stored: Vec<_> = fixes
        .iter()
        .map(|f| (f.lat, f.lon, f.acc, f.ts, f.prov.as_str()))
        .collect();
    assert_eq!(stored, vec![(40.0, -3.0, 12.5, 1_700_000_000_000, "fused")]);
    assert!(
        (t0..=t1).contains(&fixes[0].received_at),
        "received_at {} not in [{t0}, {t1}]",
        fixes[0].received_at
    );
    let last_contact = db.last_contact("phone-1").await.unwrap_or_default();
    assert!(
        (t0..=t1).contains(&last_contact),
        "last contact {last_contact} not in [{t0}, {t1}]"
    );
    assert!(db.fixes("phone-2").await.is_empty());
    assert_eq!(db.last_contact("phone-2").await, None);
}

/// TC-028
#[tokio::test]
async fn tc_028_fixes_at_the_bounds_are_accepted() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let router = db.device_router();
    let bodies = [
        r#"{"location":{"lat":-90.0,"lon":-180.0,"acc":0,"ts":1,"prov":"gps"}}"#,
        r#"{"location":{"lat":90.0,"lon":180.0,"acc":0.0,"ts":1700000000000,"prov":"network"}}"#,
        r#"{"location":{"lat":0.0,"lon":0.0,"acc":5.0,"ts":1700000000001,"prov":"fused"}}"#,
        r#"{"location":{"lat":40,"lon":-3,"acc":12.5,"ts":1700000000000,"prov":"fused"}}"#,
    ];

    let mut statuses = Vec::new();
    for body in bodies {
        statuses.push(post_fix(&router, "phone-1", Some(FP_A), body).await);
    }

    assert_eq!(statuses, vec![StatusCode::CREATED; 4]);
    let fixes = db.fixes("phone-1").await;
    assert_eq!(fixes.len(), 4);
    let south: Vec<_> = fixes
        .iter()
        .filter(|f| f.lat == -90.0)
        .map(|f| (f.lon, f.acc, f.ts, f.prov.as_str()))
        .collect();
    assert_eq!(south, vec![(-180.0, 0.0, 1, "gps")]);
}

/// TC-029
#[tokio::test]
async fn tc_029_other_sections_and_extra_location_fields_are_ignored() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let body = r#"{"battery":{"level":"oops"},"network":[1,2],"location":{"lat":40.0,"lon":-3.0,"acc":12.5,"ts":1700000000000,"prov":"fused","alt":650,"speed":"x"}}"#;

    let status = post_fix(&db.device_router(), "phone-1", Some(FP_A), body).await;

    assert_eq!(status, StatusCode::CREATED);
    let stored: Vec<_> = db
        .fixes("phone-1")
        .await
        .into_iter()
        .map(|f| (f.lat, f.prov))
        .collect();
    assert_eq!(stored, vec![(40.0, "fused".to_string())]);
}

/// TC-030
#[tokio::test]
async fn tc_030_a_repeated_fix_is_stored_again_and_moves_the_last_contact() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let router = db.device_router();
    assert_eq!(
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED,
        "the first fix is accepted"
    );
    let l1 = db.last_contact("phone-1").await.unwrap();
    // The server reads its own clock and offers no seam to set it; the case
    // only needs the two receptions to fall in different milliseconds.
    while now_ms() < l1 + 5 {
        tokio::task::yield_now().await;
    }

    let t0 = now_ms();
    let status = post_fix(&router, "phone-1", Some(FP_A), VALID).await;
    let t1 = now_ms();

    assert_eq!(status, StatusCode::CREATED);
    let ts: Vec<i64> = db.fixes("phone-1").await.iter().map(|f| f.ts).collect();
    assert_eq!(ts, vec![1_700_000_000_000, 1_700_000_000_000]);
    let last_contact = db.last_contact("phone-1").await.unwrap();
    assert!(
        (t0..=t1).contains(&last_contact) && last_contact > l1,
        "last contact {last_contact} not in [{t0}, {t1}] or not after {l1}"
    );
}

/// TC-031
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tc_031_concurrent_fixes_are_all_stored() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let router = db.device_router();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let router = router.clone();
            tokio::spawn(async move { post_fix(&router, "phone-1", Some(FP_A), VALID).await })
        })
        .collect();
    let mut statuses = Vec::new();
    for handle in handles {
        statuses.push(handle.await.unwrap());
    }

    assert_eq!(statuses, vec![StatusCode::CREATED; 10]);
    assert_eq!(db.fixes("phone-1").await.len(), 10);
}

/// TC-032
#[tokio::test]
async fn tc_032_only_the_configured_fingerprint_header_is_read() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let config = Config::from_lookup(|name| {
        (name == "ATALAYA_FINGERPRINT_HEADER").then(|| "x-test-fingerprint".to_string())
    })
    .unwrap();
    let router = device_router(db.pool.clone(), config.fingerprint_header);
    let with_header =
        |name: &str| request(Method::POST, "/telemetry/phone-1", &[(name, FP_A)], VALID);

    let statuses = [
        send(&router, with_header("x-test-fingerprint")).await.0,
        send(&router, with_header("x-client-cert-sha256")).await.0,
    ];

    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::UNAUTHORIZED]);
}

/// TC-033
#[tokio::test]
async fn tc_033_every_identity_failure_gets_the_same_401_and_stores_nothing() {
    let db = Db::migrated().await;
    let router = db.device_router();
    db.cli().enrol("phone-1", NAME, FP_A);
    db.cli().enrol("phone-2", "Otro", FP_B);
    db.cli().enrol("phone-3", "Tres", FP_C);
    db.cli().revoke("phone-3");
    for (device, fingerprint) in [("phone-1", FP_A), ("phone-2", FP_B)] {
        assert_eq!(
            post_fix(&router, device, Some(fingerprint), VALID).await,
            StatusCode::CREATED,
            "the first fix of {device} is accepted"
        );
    }
    let l1 = db.last_contact("phone-1").await;
    let l2 = db.last_contact("phone-2").await;

    let mut responses = Vec::new();
    for (device, fingerprint) in [
        ("phone-1", None),
        ("phone-1", Some(FP_F)),
        ("phone-1", Some(FP_B)),
        ("phone-9", Some(FP_A)),
        ("phone-3", Some(FP_C)),
    ] {
        responses.push(send(&router, fix_request(device, fingerprint, VALID)).await);
    }

    assert!(
        responses
            .iter()
            .all(|(status, _)| *status == StatusCode::UNAUTHORIZED),
        "{responses:?}"
    );
    assert!(
        responses.iter().all(|(_, body)| *body == responses[0].1),
        "{responses:?}"
    );
    let counts = [
        db.fixes("phone-1").await.len(),
        db.fixes("phone-2").await.len(),
        db.fixes("phone-3").await.len(),
    ];
    assert_eq!(counts, [1, 1, 0]);
    let last_contacts = [
        db.last_contact("phone-1").await,
        db.last_contact("phone-2").await,
        db.last_contact("phone-3").await,
    ];
    assert_eq!(last_contacts, [l1, l2, None]);
}

/// TC-034
#[tokio::test]
async fn tc_034_only_the_exact_lowercase_fingerprint_matches() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let router = db.device_router();
    let headers = [
        String::new(),
        FP_A_UPPER.to_string(),
        format!("{FP_A} "),
        format!("sha256:{FP_A}"),
        ["aa"; 32].join(":"),
    ];

    let mut statuses = Vec::new();
    for header in &headers {
        statuses.push(post_fix(&router, "phone-1", Some(header), VALID).await);
    }

    assert_eq!(statuses, vec![StatusCode::UNAUTHORIZED; 5]);
    assert!(db.fixes("phone-1").await.is_empty());
}

/// TC-035
#[tokio::test]
async fn tc_035_identity_is_checked_before_the_body() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let router = db.device_router();

    let statuses = [
        post_fix(&router, "phone-1", None, "not json").await,
        post_fix(&router, "phone-1", Some(FP_F), "not json").await,
        post_fix(&router, "phone-9", Some(FP_A), "{}").await,
    ];

    assert_eq!(statuses, [StatusCode::UNAUTHORIZED; 3]);
    assert_eq!(count_all_fixes(&db.path).await, 0);
}

/// TC-036
#[tokio::test]
async fn tc_036_an_invalid_fix_is_rejected_and_changes_nothing() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let router = db.device_router();
    assert_eq!(
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        StatusCode::CREATED,
        "the first fix is accepted"
    );
    let l1 = db.last_contact("phone-1").await;
    let mut bodies = vec![
        "not json".to_string(),
        String::new(),
        "{}".to_string(),
        r#"{"location":null}"#.to_string(),
        r#"{"location":[]}"#.to_string(),
        valid_with("lat", json!(91)),
        valid_with("lat", json!(-90.5)),
        valid_with("lon", json!(181)),
        valid_with("lon", json!(-181)),
        valid_with("acc", json!(-1)),
        valid_with("ts", json!(0)),
        valid_with("ts", json!(-5)),
        valid_with("ts", json!(1.5)),
        valid_with("ts", json!("1700000000000")),
        valid_with("lat", json!("40.0")),
        valid_with("prov", json!("wifi")),
        valid_with("prov", json!("GPS")),
        valid_with("prov", json!("")),
    ];
    for field in ["lat", "lon", "acc", "ts", "prov"] {
        bodies.push(valid_without(field));
    }

    let mut accepted = Vec::new();
    for body in &bodies {
        let status = post_fix(&router, "phone-1", Some(FP_A), body).await;
        if status != StatusCode::BAD_REQUEST {
            accepted.push((body.as_str(), status));
        }
    }

    assert!(accepted.is_empty(), "{accepted:#?}");
    assert_eq!(db.fixes("phone-1").await.len(), 1);
    assert_eq!(db.last_contact("phone-1").await, l1);
}

/// TC-037
#[tokio::test]
async fn tc_037_the_panel_has_no_telemetry_route_even_with_the_fingerprint() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let panel = db.panel_router();

    let statuses = [
        post_fix(&panel, "phone-1", Some(FP_A), VALID).await,
        post_fix(&panel, "phone-1", None, VALID).await,
        post_fix(&panel, "phone-1", Some(FP_A), "not json").await,
    ];

    assert_eq!(statuses, [StatusCode::NOT_FOUND; 3]);
    assert!(db.fixes("phone-1").await.is_empty());
    assert_eq!(db.last_contact("phone-1").await, None);
}
