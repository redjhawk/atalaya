# Domain documentation

Atalaya uses a single-context domain model:

- `docs/GLOSSARY.md` defines project vocabulary. The skills call it `GLOSSARY.md`; in this repository it lives under `docs/`.
- `SCOPE.md` at the repository root defines what is in and out of scope.
- `docs/adr/` contains architecture decision records.
- Component documentation lives beside component code in `apps/locator/`, `agent/` and `server/`.

Agents must update the glossary and add an ADR for decisions that change interfaces, deployment or security boundaries.

Every ADR opens with its named counterfactual: "Without this ADR, a competent agent would X", where X is a concrete, plausible action that neither the compiler, the tests, nor a reading of the code would stop. If no such X can be named, do not write the ADR.
