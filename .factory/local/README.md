# Local extensions of the agent factory

This folder is yours: the installer creates it once and updates never touch it. Everything here adds to the
factory core (`.factory/core/`); nothing replaces it, so core updates keep reaching this project.

| File | Effect |
|---|---|
| `prompts/common.md` | Appended to every stage prompt, under "Project rules for every stage". |
| `prompts/<stage>.md` | Appended to that stage's prompt (`spec`, `usecases`, `testcases`, `techspec`, `tests`, `implement`, `verify`, `review-spec`, `review-standards`, `triage`, `merge`, `repair`). |
| `gates.sh` | Functions `local_gate_<name>` (e.g. `local_gate_green`) run after the core gate passes. They can only make a gate stricter. |
| `hooks.sh` | Functions `hook_before_<stage>` / `hook_after_<stage>`, run by the factory around each stage. |
| `out-of-scope.md` | Kinds of review finding this project rejects; the triage agent drops them without a follow-up. |

The repair agent writes here too (never in the core, the skills or `AGENTS.md`): when an agent repeats a
mistake, it adds one rule to `prompts/<stage>.md` and the repair is merged once the whole suite passes.
Rules follow the factory skill `factory-writing-for-agents`: one rule per observed mistake, stated positively.
