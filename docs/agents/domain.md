# Domain documentation

Atalaya uses a single-context domain model:

- `GLOSSARY.md` at the repository root defines project vocabulary.
- `docs/adr/` contains architecture decision records.
- Component documentation lives beside component code in `apps/locator/`, `agent/` and `server/`.

Agents must update the glossary and add an ADR for decisions that change interfaces, deployment or security boundaries.
