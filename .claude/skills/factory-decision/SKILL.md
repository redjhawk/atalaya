---
name: factory-decision
description: Record a design decision (ADR) or a domain term (glossary) for the agent factory, only when it prevents a concrete mistake.
disable-model-invocation: true
---

<!-- Own text. The one-paragraph ADR format follows the domain-modeling skill of mattpocock/skills (MIT, see THIRD_PARTY_NOTICES.md of the factory). -->

Decision records and the glossary are read by every stage of the factory, on every issue. Each line costs attention on every run, so a record earns its place only by preventing a mistake.

The documentation root is `FACTORY_DOCS` in `.factory/config.sh` (`docs` by default). Records live in `<docs>/adr/NNNN-<slug>.md`; terms in `<docs>/GLOSSARY.md`. Two writers only: this session, with the owner, and the factory's specification stage. Other stages read them.

## Decision record

1. Finish this sentence with a concrete mistake: "Without this record, a competent agent would …". Then check that neither the compiler, the tests nor the code itself would stop that mistake. If you cannot finish it, or something else already stops the mistake, there is nothing to record: say so and stop. If the decision concerns a single place in the code, propose a comment there instead.
2. Take the next free number in `<docs>/adr/` (create the folder on the first record).
3. Write one paragraph that starts with the sentence from step 1, then states the decision and why. Add the measurement or fact that ruled out an alternative when there is one. Add `Status: superseded by NNNN` to an older record this one replaces, rather than deleting it.

```md
# <Short title of the decision>

Without this record, a competent agent would <mistake>. We <decision> because <reason>; <the fact that ruled out the alternative>.
```

## Glossary term

Add a term when two names for one concept would lead agents to build two things. One line: `**<Term>**: <meaning>. Avoid: <synonyms>.` Keep the file sorted.

Show the text to the owner and write it once they approve.
