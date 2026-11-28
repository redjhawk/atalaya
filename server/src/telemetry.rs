//! `POST /telemetry/{device}` on the device API: a fix is stored only when
//! the fingerprint the edge forwards is the one enrolled for `{device}`
//! (ADR 0003). Served on the device listener only.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::rejection::PathRejection;
use axum::extract::{FromRef, Path, State};
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use sqlx::SqlitePool;

/// Largest request body read, in bytes.
const BODY_LIMIT: usize = 64 * 1024;

/// State of the device router.
#[derive(Clone)]
pub struct DeviceState {
    pub pool: SqlitePool,
    /// Header in which the edge forwards the client certificate fingerprint.
    pub fingerprint_header: HeaderName,
}

impl FromRef<DeviceState> for SqlitePool {
    fn from_ref(state: &DeviceState) -> Self {
        state.pool.clone()
    }
}

/// The telemetry report. Only `location` is read; every other section is
/// ignored, not validated.
#[derive(Debug, Deserialize)]
pub struct Report {
    pub location: Fix,
}

/// One location reading. Extra fields are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Fix {
    pub lat: f64,
    pub lon: f64,
    /// Accuracy in metres.
    pub acc: f64,
    /// Unix milliseconds, device clock.
    pub ts: i64,
    pub prov: Provider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Gps,
    Fused,
    Network,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gps => "gps",
            Self::Fused => "fused",
            Self::Network => "network",
        }
    }
}

impl Fix {
    /// The bounds of a valid fix; the types are checked by deserialization.
    pub fn validate(&self) -> bool {
        (-90.0..=90.0).contains(&self.lat)
            && (-180.0..=180.0).contains(&self.lon)
            && self.acc >= 0.0
            && self.ts > 0
    }
}

/// Receives one fix. Identity is checked before the body is read.
///
/// A `{device}` segment that does not decode (not valid UTF-8) names a
/// device that cannot exist, so it gets the identity-failure response, not
/// the extractor's own rejection.
pub async fn receive(
    State(state): State<DeviceState>,
    device: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let received_at = now_ms();

    let Ok(Path(device)) = device else {
        return unauthorized();
    };
    let Some(fingerprint) = single_header(&headers, &state.fingerprint_header) else {
        return unauthorized();
    };
    match is_enrolled(&state.pool, &device, fingerprint).await {
        Ok(true) => {}
        Ok(false) => return unauthorized(),
        Err(_) => return database_unavailable(),
    }

    let Some(fix) = read_fix(body).await else {
        return invalid_fix();
    };

    match store(&state.pool, &device, fingerprint, &fix, received_at).await {
        Ok(true) => StatusCode::CREATED.into_response(),
        Ok(false) => unauthorized(),
        Err(_) => database_unavailable(),
    }
}

/// The value of `name` when the request carries it exactly once and it is
/// visible ASCII. It is used as received, never normalised.
fn single_header<'a>(headers: &'a HeaderMap, name: &HeaderName) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => value.to_str().ok(),
        _ => None,
    }
}

async fn is_enrolled(
    pool: &SqlitePool,
    device: &str,
    fingerprint: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM devices WHERE id = ? AND fingerprint = ?")
            .bind(device)
            .bind(fingerprint)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

async fn read_fix(body: Body) -> Option<Fix> {
    let bytes = axum::body::to_bytes(body, BODY_LIMIT).await.ok()?;
    let report: Report = serde_json::from_slice(&bytes).ok()?;
    report.location.validate().then_some(report.location)
}

/// Stores the fix and sets the last contact in one transaction, re-checking
/// the fingerprint so that a replacement or revocation made meanwhile
/// applies. Returns `false`, storing nothing, when it no longer matches.
async fn store(
    pool: &SqlitePool,
    device: &str,
    fingerprint: &str,
    fix: &Fix,
    received_at: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let inserted = sqlx::query(
        "INSERT INTO fixes (device_id, lat, lon, acc, ts, prov, received_at) \
         SELECT id, ?, ?, ?, ?, ?, ? FROM devices WHERE id = ? AND fingerprint = ?",
    )
    .bind(fix.lat)
    .bind(fix.lon)
    .bind(fix.acc)
    .bind(fix.ts)
    .bind(fix.prov.as_str())
    .bind(received_at)
    .bind(device)
    .bind(fingerprint)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(false);
    }
    // Requests can commit out of order (a slow body, a lock wait), so the last
    // contact only moves forward: it always shows the latest reception.
    // SQLite's multi-argument MAX returns NULL when any argument is NULL.
    sqlx::query("UPDATE devices SET last_contact = MAX(COALESCE(last_contact, 0), ?) WHERE id = ?")
        .bind(received_at)
        .bind(device)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// The one response of every identity failure, so that none reveals which
/// devices or fingerprints exist.
fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized\n").into_response()
}

fn invalid_fix() -> Response {
    (StatusCode::BAD_REQUEST, "invalid fix\n").into_response()
}

fn database_unavailable() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "database unavailable\n").into_response()
}

fn now_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{HeaderName, Method, Request, StatusCode};
    use tower::ServiceExt;

    const FP: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VALID: &str =
        r#"{"location":{"lat":40.0,"lon":-3.0,"acc":12.5,"ts":1700000000000,"prov":"fused"}}"#;

    /// FR-TELEMETRY-003: a `{device}` that is not valid UTF-8 cannot exist,
    /// so it gets the same 401 as every other identity failure.
    #[tokio::test]
    async fn a_device_segment_that_is_not_utf8_gets_the_identity_failure() {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::open_database(&dir.path().join("atalaya.db"))
            .await
            .unwrap();
        let router = crate::device_router(
            pool.clone(),
            HeaderName::from_static("x-client-cert-sha256"),
        );

        for fingerprint in [None, Some(FP)] {
            let mut request = Request::builder()
                .method(Method::POST)
                .uri("/telemetry/%FF")
                .header("content-type", "application/json");
            if let Some(fingerprint) = fingerprint {
                request = request.header("x-client-cert-sha256", fingerprint);
            }
            let response = router
                .clone()
                .oneshot(request.body(Body::from(VALID)).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(&body[..], b"unauthorized\n");
        }
        let fixes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fixes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fixes, 0);
    }
}
