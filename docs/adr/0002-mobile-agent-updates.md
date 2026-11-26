# ADR 0002: Mobile and Agent Update Channel

## Status

Proposed

## Decision

Use release artifacts produced by GitHub Actions:

- a signed APK for the locator,
- a signed Magisk module ZIP for the privileged agent,
- a signed manifest containing version, artifact SHA-256, and signature metadata.

The server publishes the latest stable manifest. The locator downloads a normal update or, for privileged agent scripts, reports availability and invokes the root agent. The agent verifies the manifest signature and artifact hash before replacing files atomically.

## Development deployment

Use `adb install -r` for rapid APK iteration and direct Magisk ZIP installation for agent development.

## Production deployment

Run updates through an explicit stable channel:

1. GitHub Actions builds and signs artifacts.
2. The server publishes the signed manifest.
3. The locator or agent verifies signatures and hashes.
4. The agent stages files under `/data/adb/modules/atalaya-agent/`.
5. Restart the relevant service and report success or rollback to the server.

## Consequences

A signed, auditable update path is simpler and safer than relying on interactive manual copies. Deployment remains separately gated from source merging.
