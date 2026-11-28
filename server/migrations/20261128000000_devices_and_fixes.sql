-- Enrolled devices and the fixes they send (issue #12).

CREATE TABLE devices (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL,
    fingerprint  TEXT UNIQUE,          -- lowercase hex; NULL when revoked
    last_contact INTEGER               -- Unix ms; NULL until the first accepted fix
) STRICT;

CREATE TABLE fixes (
    id          INTEGER PRIMARY KEY,
    device_id   TEXT NOT NULL REFERENCES devices (id),
    lat         REAL NOT NULL,
    lon         REAL NOT NULL,
    acc         REAL NOT NULL,
    ts          INTEGER NOT NULL,      -- Unix ms, device clock
    prov        TEXT NOT NULL CHECK (prov IN ('gps', 'fused', 'network')),
    received_at INTEGER NOT NULL       -- Unix ms, server clock
) STRICT;

CREATE INDEX fixes_by_device ON fixes (device_id, received_at);
