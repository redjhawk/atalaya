//! Device enrolment: the validated values an operator gives on the command
//! line, and the writes that enrol a device, replace or revoke its
//! fingerprint. Nothing here is reachable over the network.

use std::fmt;

use sqlx::SqlitePool;

/// The `{device}` of the device API routes: 1 to 64 bytes of `a-z`, `0-9`
/// or `-`, not starting with `-`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceId(String);

/// Free text of 1 to 100 characters, not only spaces. Stored as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceName(String);

/// The SHA-256 of the client certificate: 64 hex digits, kept lowercase,
/// the form the edge forwards (ADR 0003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Id,
    Name,
    Fingerprint,
}

/// A command-line value that breaks the input rules. The rejected value is
/// never echoed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    pub field: Field,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.field {
            Field::Id => "invalid device id: 1 to 64 characters a-z, 0-9 or -, not starting with -",
            Field::Name => "invalid name: 1 to 100 characters, not only spaces",
            Field::Fingerprint => "invalid fingerprint: 64 hexadecimal characters, no separators",
        })
    }
}

impl std::error::Error for InputError {}

impl DeviceId {
    pub fn parse(value: &str) -> Result<Self, InputError> {
        let allowed = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-';
        let bytes = value.as_bytes();
        let valid =
            (1..=64).contains(&bytes.len()) && bytes[0] != b'-' && bytes.iter().all(allowed);
        if valid {
            Ok(Self(value.to_string()))
        } else {
            Err(InputError { field: Field::Id })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl DeviceName {
    pub fn parse(value: &str) -> Result<Self, InputError> {
        let valid = (1..=100).contains(&value.chars().count()) && !value.trim().is_empty();
        if valid {
            Ok(Self(value.to_string()))
        } else {
            Err(InputError { field: Field::Name })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Fingerprint {
    pub fn parse(value: &str) -> Result<Self, InputError> {
        if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(value.to_ascii_lowercase()))
        } else {
            Err(InputError {
                field: Field::Fingerprint,
            })
        }
    }

    /// The lowercase form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub enum EnrolError {
    DeviceExists(DeviceId),
    UnknownDevice(DeviceId),
    FingerprintTaken,
    Database(sqlx::Error),
}

impl fmt::Display for EnrolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceExists(id) => write!(f, "device {id} already exists"),
            Self::UnknownDevice(id) => write!(f, "no device {id}"),
            Self::FingerprintTaken => f.write_str("fingerprint already belongs to another device"),
            Self::Database(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for EnrolError {}

impl From<sqlx::Error> for EnrolError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// What `set_fingerprint` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintChange {
    Replaced,
    /// The device already had that fingerprint; nothing was written.
    Unchanged,
}

/// Enrols a new device with no fixes and no last contact.
pub async fn add(
    pool: &SqlitePool,
    id: &DeviceId,
    name: &DeviceName,
    fingerprint: &Fingerprint,
) -> Result<(), EnrolError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    if exists(&mut tx, "SELECT 1 FROM devices WHERE id = ?", id.as_str()).await? {
        return Err(EnrolError::DeviceExists(id.clone()));
    }
    if exists(
        &mut tx,
        "SELECT 1 FROM devices WHERE fingerprint = ?",
        fingerprint.as_str(),
    )
    .await?
    {
        return Err(EnrolError::FingerprintTaken);
    }
    let inserted = sqlx::query(
        "INSERT INTO devices (id, name, fingerprint, last_contact) VALUES (?, ?, ?, NULL)",
    )
    .bind(id.as_str())
    .bind(name.as_str())
    .bind(fingerprint.as_str())
    .execute(&mut *tx)
    .await;
    match inserted {
        Ok(_) => {}
        // Defence in depth: the checks above run under the write lock.
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            let id_taken =
                exists(&mut tx, "SELECT 1 FROM devices WHERE id = ?", id.as_str()).await?;
            return Err(if id_taken {
                EnrolError::DeviceExists(id.clone())
            } else {
                EnrolError::FingerprintTaken
            });
        }
        Err(e) => return Err(e.into()),
    }
    tx.commit().await?;
    Ok(())
}

/// Replaces the fingerprint of an existing device, revoked or not. Name,
/// fixes and last contact are kept.
pub async fn set_fingerprint(
    pool: &SqlitePool,
    id: &DeviceId,
    fingerprint: &Fingerprint,
) -> Result<FingerprintChange, EnrolError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let current: Option<Option<String>> =
        sqlx::query_scalar("SELECT fingerprint FROM devices WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&mut *tx)
            .await?;
    let Some(current) = current else {
        return Err(EnrolError::UnknownDevice(id.clone()));
    };
    if current.as_deref() == Some(fingerprint.as_str()) {
        tx.commit().await?;
        return Ok(FingerprintChange::Unchanged);
    }
    let taken: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM devices WHERE fingerprint = ? AND id <> ?")
            .bind(fingerprint.as_str())
            .bind(id.as_str())
            .fetch_optional(&mut *tx)
            .await?;
    if taken.is_some() {
        return Err(EnrolError::FingerprintTaken);
    }
    sqlx::query("UPDATE devices SET fingerprint = ? WHERE id = ?")
        .bind(fingerprint.as_str())
        .bind(id.as_str())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(FingerprintChange::Replaced)
}

/// Leaves the device without a fingerprint. Revoking a revoked device
/// succeeds. Name, fixes and last contact are kept.
pub async fn revoke(pool: &SqlitePool, id: &DeviceId) -> Result<(), EnrolError> {
    let result = sqlx::query("UPDATE devices SET fingerprint = NULL WHERE id = ?")
        .bind(id.as_str())
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(EnrolError::UnknownDevice(id.clone()));
    }
    Ok(())
}

async fn exists(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    query: &'static str,
    value: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(query)
        .bind(value)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.is_some())
}
