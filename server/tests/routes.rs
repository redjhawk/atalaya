//! The route set of each listener: no route on either one enrols a device or
//! changes its fingerprint.

mod common;

use axum::http::{Method, StatusCode};
use common::*;

/// TC-025
#[tokio::test]
async fn tc_025_no_route_on_either_listener_enrols_or_changes_a_device() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let body = format!(
        r#"{{"id":"phone-9","name":"Nueve","fingerprint":"{}"}}"#,
        "b".repeat(64)
    );
    let headers = [(HEADER, FP_B), ("content-type", "application/json")];
    let paths = [
        "/devices",
        "/devices/phone-9",
        "/devices/phone-1",
        "/devices/phone-1/fingerprint",
        "/enrol",
        "/admin/devices",
    ];
    let mut attempts = Vec::new();
    for path in paths {
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            attempts.push((method, path));
        }
    }
    attempts.push((Method::PUT, "/telemetry/phone-1"));
    attempts.push((Method::DELETE, "/telemetry/phone-1"));

    let mut answered = Vec::new();
    for (listener, router) in [("device", db.device_router()), ("panel", db.panel_router())] {
        for (method, path) in &attempts {
            let (status, _) = send(&router, request(method.clone(), path, &headers, &body)).await;
            if status != StatusCode::NOT_FOUND && status != StatusCode::METHOD_NOT_ALLOWED {
                answered.push((listener, method.clone(), *path, status));
            }
        }
    }

    assert!(answered.is_empty(), "{answered:#?}");
}

/// TC-025
#[tokio::test]
async fn tc_025_guessed_enrolment_requests_leave_the_fingerprints_unchanged() {
    let db = Db::migrated().await;
    db.cli().enrol("phone-1", NAME, FP_A);
    let body = format!(
        r#"{{"id":"phone-9","name":"Nueve","fingerprint":"{}"}}"#,
        "b".repeat(64)
    );
    let headers = [(HEADER, FP_B), ("content-type", "application/json")];
    for router in [db.device_router(), db.panel_router()] {
        for path in [
            "/devices",
            "/devices/phone-9",
            "/devices/phone-1",
            "/devices/phone-1/fingerprint",
            "/enrol",
            "/admin/devices",
        ] {
            for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
                send(&router, request(method, path, &headers, &body)).await;
            }
        }
    }

    let router = db.device_router();
    let statuses = [
        post_fix(&router, "phone-1", Some(FP_A), VALID).await,
        post_fix(&router, "phone-1", Some(FP_B), VALID).await,
        post_fix(&router, "phone-9", Some(FP_B), VALID).await,
    ];
    assert_eq!(
        statuses,
        [
            StatusCode::CREATED,
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED
        ]
    );
}

/// TC-026
#[tokio::test]
async fn tc_026_each_listener_serves_only_its_own_routes() {
    let db = Db::migrated().await;
    let device = db.device_router();
    let panel = db.panel_router();
    let health = || request(Method::GET, "/healthz", &[], "");

    let statuses = [
        send(&device, health()).await.0,
        post_fix(&device, "phone-1", None, VALID).await,
        send(&panel, health()).await.0,
        post_fix(&panel, "phone-1", None, VALID).await,
    ];

    assert_eq!(
        statuses,
        [
            StatusCode::OK,
            StatusCode::UNAUTHORIZED,
            StatusCode::OK,
            StatusCode::NOT_FOUND
        ]
    );
}
