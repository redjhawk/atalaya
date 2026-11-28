# Agent factory workflow

The factory takes one GitHub issue to one merged pull request, with agents doing the work and deterministic gates deciding whether it advances. It runs the same way on a workstation (`.factory/core/bin/factory run <issue>`) and in GitHub Actions (`.github/workflows/factory.yml`). Paths below use the defaults of `.factory/config.sh`: documentation root `docs`, base branch `main`, label `ai-dev`.

## Stages

| # | Stage | Who | Output | Gate (deterministic) |
|---|---|---|---|---|
| 1 | `spec` | functional specifier | `docs/specifications/<subject>/functional.md` (`FR-<SUBJECT>-NNN`), the index, `feature-path.md`; the only writer of `docs/GLOSSARY.md` and `docs/adr/` | a functional spec changed and defines FR IDs; a feature path exists when verify is enabled |
| ⏸ | questions | — | `questions.md`, posted on the issue | the run stops until the owner answers in a comment |
| 2 | `usecases` (optional) | use-case writer | `docs/use-cases/<subject>/UC-*.md` | every new use case cites an FR ID |
| 3 | `testcases` | test designer | `<change>/testcases.md` (`TC-NNN`, Given/When/Then, level) | every FR ID of the change has a TC |
| 4 | `techspec` (optional) | technical specifier | `technical.md` per subject, the API contract | each subject has a technical spec |
| 5 | `tests` | test author | tests for every TC | every TC is in a test and the suite **fails** (TDD red); project checks (`project_red_checks`); tests locked in `tests.lock`. Before the agent, the script merges the base branch and resets every implementation path to it |
| 6 | `implement` | developer | code | locked tests unchanged and the whole suite (`project_tests`) **passes**; up to 3 attempts with the gate output; each round first merges the base branch |
| 7 | `verify` (when `project_start` is defined) | verifier | `<change>/verify.json` | the system starts (`project_ready`); every step of the feature path passes and no finding blocks. A failure goes back to `implement` with the findings, twice, then asks the owner |
| 8 | `pr` | script | one PR to the base branch, `Closes #<n>`, with the door (one-way/two-way) and the blast radius | — |
| 9 | `review` | two independent reviewers in parallel (read-only): spec axis and standards axis | `review-rN.json` (ids `R<round>-NN`, each with its axis), posted as one inline review | valid JSON from both |
| 10 | `triage` | independent triage agent | `triage-rN.json`. Blocking → `blocking.md` and back to `implement`. Non-blocking → an issue only when the triage names the consequence of leaving it; otherwise a note in `<change>/followups.md` | one decision per review id; after 3 rounds with blocking findings, 3 more start once without asking, then the owner decides |
| 11 | `merge` | script | squash merge, branch deleted | no sensitive file changed; no one-way door; `project_security_scan` passes |

When the gate of stages 1 to 5 fails, the agent runs once more with the gate's output (`FACTORY_STAGE_ATTEMPTS`), and only then does the run stop. A project adds checks to any gate with `local_<gate>` functions in `.factory/local/gates.sh`; they run after the core gate and can only make it stricter.

Every agent stage is a separate `claude -p` process with its own prompt and only the tools its stage needs, so roles share no context: the reviewers and the triage agent cannot see the developer's reasoning. Agents never commit, push or open PRs; the script does, after the gate passes.

## What each agent reads

- The core prompt of its stage (`.factory/core/prompts/common.md` + `<stage>.md`), then this project's additions (`.factory/local/prompts/common.md` + `<stage>.md`).
- The run context: issue, change folder, stack, test commands, documentation layout, the paths the code stages may edit.
- **Every skill already in the repository** (`FACTORY_SKILL_DIRS`), listed with its description; their rules apply to the agent's work. Where a skill describes a process (asking, committing, opening PRs), the factory runs that part. The factory never edits, renames or removes a project skill, and installs its own skills under the `factory-` prefix.
- `AGENTS.md`, whose factory block sits beside the project's own rules.

## State and resuming

- Branch `ai-dev/issue-<n>`; change folder `docs/changes/<YYYY-MM-DD-HHMM>-issue-<n>-<slug>/` with `state.json` (`stage`, `round`, `pr`, counters), `issue.md` and the stage outputs. The script commits and pushes after every stage, so any run resumes from the recorded stage; the stage is also shown as a `factory:<stage>` label.
- A developer who finds a wrong locked test writes `test-problem.md`; the run returns to `tests`, where the test author checks the report against the specifications. Once per issue without asking; the second time the owner decides.
- A developer who cannot finish for a reason outside the change writes `blocked.md`. `Category: environment` stops the run. `Category: base-branch` makes the factory run the whole suite on the base branch in a separate worktree: if it passes there, the failures come from the change; if it fails there too, the factory opens a `base-bug` issue (labelled for the factory), and waits for it.
- Stops: `factory:needs-input` (questions, failed gate, test problem again, verify again, too many rounds), `factory:needs-human` (sensitive files, one-way door, merge conflict, failed merge), `factory:blocked` (environment), `factory:waiting` (a base bug), `factory:waiting-limit` (usage limit), `factory:failed` (any other error). Every stop comment carries `<!-- factory-stop {"reason": …, "stage": …, "round": …} -->` for the supervisor.
- Only trusted authors (`FACTORY_TRUSTED_ACTORS`, by default the repository owner) give instructions. The body of an issue by anyone else is dropped from `issue.md`; their comments, the factory's own comments, review comments, logs and code are data.
- To move a change back by hand, edit `stage` in its `state.json`, commit and push, and run the factory again.

## Security model

- Agents run without `GH_TOKEN`/`GITHUB_TOKEN` and with `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1`; tests, builds and the system under verification run with every credential removed. Only the scripts use the token, to push, comment, open and merge.
- Tool profiles: `spec`/`docs` edit only the documentation root and the API contract (only `spec` may write the glossary and the decision records: deny rules for the others, and the script restores those files after any other stage); `code` edits only `FACTORY_CODE_PATHS`, the specifications and the change folder, and runs only `FACTORY_CODE_COMMANDS`; `review` and `triage` write only the change folder; `verify` writes only the change folder and runs only `FACTORY_VERIFY_COMMANDS`; `merge` edits only the conflicted files.
- No profile can delete a file. An agent lists the files in `<change>/delete.txt`; the script deletes those its profile may edit and that are not sensitive, and keeps the others.
- `tests.lock` is read from the test reference commit (the TDD red commit, or the factory's later merge of the base branch); any test file changed after it fails the green gate, and the implement stage restores test files before each attempt.
- **Never merged by the factory** (`is_sensitive` in `lib.sh`): CI (`.github/`), the factory (`.factory/`), agent instructions and skills (`.claude/`, `.agents/`, every `FACTORY_SKILL_DIRS` folder, `AGENTS.md`, `CLAUDE.md`), dependency manifests, `.gitignore`, plus the project's `FACTORY_SENSITIVE_PATTERN`. **One-way doors** (`FACTORY_ONE_WAY_PATHS`, `FACTORY_ONE_WAY_PATTERN`: migrations, schema statements) also wait for a human.
- The workflows copy `.factory/` from the base branch before switching to the work branch, so a change cannot alter its own pipeline, configuration or extensions.
- Every factory comment starts with `<!-- agent-factory -->`, and the factory workflow ignores comments that contain it, so the factory cannot restart itself.

## Supervisor and repair

`factory-supervisor.yml` runs `supervisor.mjs` whenever a factory or repair run ends and about every 15 minutes. It resumes runs that stopped for a reason that goes away without the owner (a usage limit after its reset; an error or a crash after 15, 30 and 45 minutes; a base bug once its issue is closed; an owner comment that started no run), and starts a **repair** for technical stops that a relaunch does not cure (a failed gate, a blocked environment, spent retries). Everything else waits for the owner. Before relaunching, a probe agent checks that the runner can run agents; when it cannot, one `factory-infra` issue says so.

A repair (`factory-repair.yml`, `repair.sh`, prompt `repair.md`) is an agent that reads the stop, the logs and the agents' refused tool calls, reproduces the cause and fixes it on the base branch. It may change this project's code, tests, documentation and `.factory/local/` (a recurring agent mistake becomes one rule in `.factory/local/prompts/<stage>.md`). It may not change CI, dependencies, `.factory/core/`, `.factory/config.sh`, `AGENTS.md`, `CLAUDE.md` or any skill: those need the owner, and a bug in the core is fixed in the factory's own repository. The fix must pass the whole suite; then it is merged and the issue relaunched. At most 2 repairs per stage and round.

The supervisor also feeds the factory: while fewer than `FACTORY_MAX_ACTIVE` issues (1 by default) are in development, it launches the next open issue of a trusted author that has never been in the factory, ordered by the owner's `priority:high`/`priority:low` labels and then by a ranking agent (blocking bugs, dependencies, size, age). Off switch: repository variable `FACTORY_SUPERVISOR=off`; `FACTORY_MAX_ACTIVE=0` stops only the launches; the label `factory-hold` excludes one issue; `factory supervise --dry-run` prints the decisions.

## Settings

Environment: `FACTORY_MODEL` (`claude-opus-5-5`), `FACTORY_EFFORT` (`high`), `FACTORY_MAX_TURNS` (150), `FACTORY_MAX_ROUNDS` (3), `FACTORY_MAX_ATTEMPTS` (3), `FACTORY_STAGE_ATTEMPTS` (2), `FACTORY_MAX_VERIFY` (2), `FACTORY_VERIFY_WAIT` (60 checks of `project_ready`, 2 s apart), `FACTORY_SECURITY` (1), `FACTORY_LOG_DIR`; for repairs `FACTORY_REPAIR_ATTEMPTS` (2) and `FACTORY_REPAIR_MAX_TURNS` (200); for the supervisor `FACTORY_WORKFLOW` (`factory.yml`, the workflow whose runs it reads) and `FACTORY_RANK_MODEL` (`claude-sonnet-5-5`, the queue's ranking agent). Project settings and commands: `.factory/config.sh` (reference: `.factory/config.example.sh`).

## Scope rules

- One issue, one PR. A request too large for one PR is split into several issues, ideally before the factory starts (`/factory-request`).
- Functional decisions belong to the owner: the spec stage asks and the run stops. Technical, API, refactoring and review decisions belong to the agents and are recorded in the specifications and the triage files.
- Refactoring, dependency updates and formatting unrelated to the issue are not part of the change; the spec reviewer flags them as `refactor-pr` and triage blocks them.
