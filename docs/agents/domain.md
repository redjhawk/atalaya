# Domain documentation

Atalaya uses a single-context domain model:

- `GLOSSARY.md` at the repository root defines project vocabulary.
- `docs/adr/` contains architecture decision records.
- Component documentation lives beside component code in `apps/locator/`, `agent/` and `server/`.

Agents must update the glossary and add an ADR for decisions that change interfaces, deployment or security boundaries.

Every ADR opens with its named counterfactual: "Without this ADR, a competent agent would X", where X is a concrete, plausible action that neither the compiler, the tests, nor a reading of the code would stop. If no such X can be named, do not write the ADR.
