# ADR 0001: Software Factory Architecture

Without this ADR, a competent agent would merge and deploy on green checks alone, including privileged agent changes, SOS behavior and production device deployment, and would pick its own tracker, runners or repository layout for each component.

## Status

Accepted

## Decision

Atalaya uses one GitHub monorepo for the locator, agent and server. GitHub Issues is the ticket tracker, GitHub Actions is the orchestrator, and GitHub is the git host. Reusable agent discipline comes from the Matt Pocock skills installed under `.agents/skills/`.

## Runner topology

Start with GitHub-hosted runners for code compilation and deterministic checks. Add labels for specialized self-hosted runners when required:

- `atalaya-factory`: agent runs that only read the repository and write to the ticket tracker, on ephemeral pods in a home k3s cluster. Never used by workflows that a pull request can trigger.
- `atalaya-linux`: server builds and integration tests.
- `atalaya-android`: Gradle builds and Android emulator tests.
- `atalaya-device`: controlled XCover hardware tests.

## Autonomy boundary

Automated review and merging are allowed only after deterministic checks and an automated cold diff review pass. Production privilege changes, SOS behavior and device deployment require human approval until reliability earns an exception.

## Consequences

This keeps a simple initial setup, while retaining the option to scale individual runner pools by component.
