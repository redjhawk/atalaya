# Third-party notices

## mattpocock/skills

Source: https://github.com/mattpocock/skills, commit `6f7f783` (v1.3.2). License: MIT.

Copied verbatim (with a provenance comment):
- `skills/factory-writing-for-agents/SKILL.md` (from `skills/productivity/writing-for-agents/SKILL.md`; name and description changed)
- `skills/factory-writing-for-agents/SKILL-MECHANICS.md`
- `skills/factory-testing-rules/tests.md` and `mocking.md` (from `skills/engineering/tdd/`)

Adapted (rewritten in this project's words, same ideas):
- `skills/factory-testing-rules/SKILL.md`: summary of `tdd/tests.md` and `tdd/mocking.md`
- `core/prompts/tests.md`: the good-test rules
- `core/prompts/review-standards.md`: the code-smell baseline of `engineering/code-review` (after Fowler, *Refactoring*, ch. 3)
- the two-axis review (`core/prompts/review-spec.md`, `review-standards.md`): the idea of `engineering/code-review`
- `skills/factory-request/SKILL.md`: the interview in rounds of `productivity/grilling` and the vertical slicing of `engineering/to-tickets`
- `skills/factory-decision/SKILL.md`: the one-paragraph ADR format of `engineering/domain-modeling/ADR-FORMAT.md`
- `core/bin/factory.sh` (`pr_body`): the "one-way door" and "blast radius" of `engineering/pr`

```
MIT License

Copyright (c) 2026 Matt Pocock

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Loulen/prompt-driven-software-factory (PDSF)

Source: https://github.com/Loulen/prompt-driven-software-factory, commit `9c19385`. **No license.**
No text or code of PDSF is copied here. Its ideas are reimplemented in this project's own words and code:
the feature path driven by an agent on the running system (`verify` stage), the named-counterfactual rule
for decision records, the single writer of the glossary and the decisions, the health check (`factory doctor`)
and the business-story layout of `factory-request`. Ideas are not covered by copyright; if you want to
quote PDSF's text, ask its authors for a license first.

## pricetracker

The factory scripts (`core/bin/factory.sh`, `lib.sh`, `gates.sh`, `repair.sh`, `supervisor.mjs`), the stage
prompts and the workflows generalise the agent factory of the redjhawk/pricetracker repository, by the same
owner, released here under this project's MIT license.
