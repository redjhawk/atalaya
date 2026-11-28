# Atalaya Agent Instructions

## Scope

This repository is a monorepo with three independently buildable components:

- `apps/locator/`: Kotlin Android app.
- `agent/`: Magisk module and privileged Android scripts.
- `server/`: Rust Axum server and SQLite persistence.

Use the root README and `docs/GLOSSARY.md` for project context. Read component README files before changing a component.

`SCOPE.md` is the project boundary. Read it before proposing functionality, and reject any spec, ticket or PR that crosses it.

## Agent skills

### Issue tracker

GitHub Issues tracks specs, tickets and factory tasks. See `docs/agents/issue-tracker.md`.

### Triage labels

The canonical labels are `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context `docs/GLOSSARY.md` and ADRs in `docs/adr/`. See `docs/agents/domain.md`.

### Factory tasks

Automated tasks, their triggers and where a human re-enters. See `docs/agents/factory-tasks.md`.

## Delivery rules

1. Read the originating issue/spec before implementation.
2. Keep changes inside the smallest relevant component.
3. Test the component through public behavior before implementation details.
4. Build server changes as static Linux artifacts and app changes through Gradle.
5. Never include credentials, device tokens or real location data in source control.
6. Treat SOS, privilege escalation and privileged module updates as sensitive changes requiring human review.
7. Reconcile only a PR diff against the originating spec and repository standards.

## Factory gates

Every PR must pass:

- deterministic formatting and lint checks,
- unit and integration tests,
- security/secret scanning,
- an automated standards and spec review,
- and component builds.

Human re-entry is required for production device deployment, privileged Magisk changes, authentication, secret handling and SOS behavior.

<!-- factory:begin (managed by the agent factory installer; edit outside this block) -->
## Agent factory

Feature work and behaviour changes go through the agent factory (`.factory/core/docs/WORKFLOW.md`): specification, use cases, test cases, technical specification, tests first (TDD), implementation, verification on the running system, independent two-axis review, triage and merge. Start it by labelling an issue with the factory label, or run `.factory/core/bin/factory run <issue>`. Prepare issues with the owner using the `/factory-request` skill.

When you run as one of the factory's stage agents, the stage prompt is your task: do that stage only; the pipeline commits, pushes and opens pull requests. This project's own rules above and below this block, and its skills, keep applying to the code you write.
<!-- factory:end -->
