# Atalaya Server

Rust/Axum API and web interface backed by SQLite.

Component responsibilities:

- telemetry and location persistence,
- command queue and acknowledgements,
- WebSocket delivery,
- map and device status UI,
- signed package release metadata.

The release target is `x86_64-unknown-linux-musl` for the T310.
