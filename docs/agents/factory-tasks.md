# Factory tasks

Tasks the Atalaya factory runs. Each task answers the five questions of the `software-factory` skill. Add one automated trigger at a time, with human review on its output, and widen it only once it has earned trust.

## Ticketing

Turns a spec into tracer-bullet tickets. Wired up in `.github/workflows/factory-tickets.yml`.

1. **What triggers it?** A human adds the `factory:tickets` label to a spec issue written by the repository owner. No clock and no other run triggers it.
2. **What does it do?** It reads the spec issue body and only the comments written by the repository owner (the repository is public, so anyone else's comment could carry instructions), `SCOPE.md`, `docs/GLOSSARY.md` and the ADRs, then follows the `to-tickets` skill without the interview. It writes only to the ticket tracker: up to 12 tickets as sub-issues of the spec, with blocking links and a triage label each. It cannot push code: its token has read access to contents and write access to issues only.
3. **What does it trigger next?** Nothing. Tickets are created with the default Actions token, which starts no further runs. Implementation is not automated yet.
4. **Where does a human re-enter?** After the run: the human reads the tickets, edits or closes them, and decides which to hand to an agent. A spec that crosses `SCOPE.md` or contradicts an ADR gets `needs-info` and no tickets.
5. **What stops two runs doing the same work?** One run per spec issue at a time (workflow concurrency). A run that finds existing sub-issues creates nothing. The workflow removes the trigger label when the run ends, even when it fails, so labelling again retries. A failed run comments on the spec with a link to the run; if it left some tickets behind, the retry creates nothing, and a human finishes or deletes them first.

The task needs one repository secret set by a human: `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY`. Without it every run fails and nothing is created.

## Agent factory: issue to merged pull request

Installed from `redjhawk/customfactory` under `.factory/`. Its full reference is `.factory/core/docs/WORKFLOW.md`; the project's settings are `.factory/config.sh` and `.factory/local/`. It replaces the implement and review tasks this document used to list as missing.

1. **What triggers it?** The `ai-dev` label on an issue written by the repository owner, added by the owner or by the supervisor. An owner comment on that issue resumes a stopped run. The supervisor (`factory-supervisor.yml`) runs after each factory run and about every 15 minutes, and launches the next open owner issue while fewer than `FACTORY_MAX_ACTIVE` are in development.
2. **What does it do?** One issue becomes one pull request: specification, use cases, test cases, technical specification, tests first, implementation, verification on the running server, two independent reviews, triage. Agents run without credentials; only the scripts push, comment and merge. For now its agents may edit only `server/` code, tests and migrations, and the documentation under `docs/`.
3. **What does it trigger next?** The supervisor resumes interrupted runs, starts a repair for technical stops, and launches the next issue.
4. **Where does a human re-enter?** The run stops and asks on the issue when a functional decision is missing. The factory never merges on its own a change that touches CI, the factory itself, agent instructions and skills, dependency manifests, `agent/`, `apps/`, `SCOPE.md`, a schema migration, or code that mentions device identity, secrets or SOS: those pull requests wait for the owner, as `AGENTS.md` requires.
5. **What stops two runs doing the same work?** One run per issue (workflow concurrency), state on the branch `ai-dev/issue-<n>`, and a `factory:<stage>` label on the issue. An issue that has been in the factory is never launched again by the supervisor.

Switches: the repository variable `FACTORY_SUPERVISOR=off` stops the supervisor; `FACTORY_MAX_ACTIVE=0` stops only new launches; the `factory-hold` label keeps one issue out of the queue. Put `factory-hold` on every issue that is a spec with sub-issues, that needs a person, or that is not ready.

It needs two repository secrets set by a human: `FACTORY_TOKEN` (the owner's token with contents, pull requests and issues access) and `CLAUDE_CODE_OAUTH_TOKEN`.

Tickets created by the ticketing task above are written by the Actions bot, not by the owner. The factory treats their body as untrusted and the supervisor does not launch them.

## Not automated yet

- **Locator and agent components**: the factory is configured for `server/` only.
