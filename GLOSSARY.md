# Atalaya Glossary

- **device**: Android device managed by Atalaya, initially a rooteado Samsung Galaxy XCover 5.
- **locator**: Kotlin app that reports telemetry, location and commands on device.
- **agent**: privileged Magisk module and scripts that execute root actions, maintain persistence and process the SOS button.
- **server**: Rust/Axum service that persists data and exposes the API and web UI.
- **fix**: one location reading with `lat`, `lon`, `acc`, `ts` and `prov`.
- **acc**: estimated horizontal location precision in metres at a 68 percent confidence radius.
- **telemetry**: battery, network, app inventory, usage, integrity and screen state.
- **command**: an operation requested by the server for the device.
- **ack**: confirmation that a command was received or completed.
- **SOS**: event flow activated by the physical button that reports location and alerts the configured group.
- **task**: a reusable kind of factory work, such as review or implementation.
- **trigger**: an event that starts a factory run.
- **run**: one execution of a task on a disposable runner.
- **runner**: a disposable machine that executes a factory run.
- **ticket tracker**: GitHub Issues, the factory's human interface.
- **git host**: GitHub, where branches, pull requests and reviews live.
- **orchestrator**: GitHub Actions, which receives triggers and starts runs.
