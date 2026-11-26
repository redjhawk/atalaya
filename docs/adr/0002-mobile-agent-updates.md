# ADR 0002: Mobile and Agent Update Channel

Without this ADR, a competent agent would let the locator or the root agent download and install the latest build straight from the server, without verifying a signature or hash, and would treat a merge to `main` as permission to update the production device.

## Status

Accepted

## Pending implementation

The automatic update pipeline is **not configured yet**. This remains a factory backlog item until all of the following exist:

- GitHub Actions jobs that build and sign the locator APK and Magisk module ZIP.
- A signing-key and release-secret model, documented without storing private material in Git.
- A signed manifest published by the server with version, SHA-256 hashes and signature metadata.
- Verification logic in the locator and agent before downloading or replacing an artifact.
- Atomic installation and rollback for both the APK and module.
- A stable/canary update channel and explicit human promotion controls.
- End-to-end tests against a disposable Android device before production deployment.

Source merging must not imply production deployment. Privileged module updates and SOS behavior require human re-entry.

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
