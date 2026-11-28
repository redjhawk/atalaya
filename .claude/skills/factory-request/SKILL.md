---
name: factory-request
description: Design session with the owner that turns a need into GitHub issues the agent factory can build alone (user story, rules, acceptance criteria, feature path, dependencies).
disable-model-invocation: true
---

<!-- Own text. The interview in rounds adapts the grilling skill of mattpocock/skills (MIT, see THIRD_PARTY_NOTICES.md of the factory). -->

The factory works alone from the moment an issue gets its label, and it stops to ask whenever the issue leaves a functional decision open. This session is where those decisions get made, with the owner, before the factory starts: the human effort goes into design, the agents do the rest.

## 1. Understand the need

Read what the owner gave you (text, an issue number, a document) and the project: `AGENTS.md`, the glossary and decision records under the documentation root of `.factory/config.sh`, the functional specifications that touch the subject, and the code. Finding facts is your job: look them up instead of asking.

## 2. Interview in rounds

Map the need as a **design tree**: each decision branches into the decisions that depend on it. The **frontier** is every decision whose prerequisites are settled. Ask the whole frontier in one round: number each question, give your recommended answer, and word it so that "yes" accepts the recommendation. Wait for the answers, recompute the frontier, ask the next round.

```
❓ **Q1** - **<title>**: <question, with the options you see>

➡️ <recommended answer>
```

Ask only about what the user observes and decides: behaviour, business rules, acceptance, what is out of scope. Technical design is the factory's job. The interview is done when the frontier is empty and the owner confirms the shared understanding.

## 3. Slice into issues

One issue is one pull request. Cut the need into **vertical slices**: each one a narrow but complete path through every tier, verifiable on its own, small enough for one factory run. Prefactoring comes first, as its own slice. A wide mechanical change (a rename across the codebase) goes as expand, migrate in batches, contract.

Show the slices as a numbered list (title, what it delivers, blocked by) and iterate until the owner approves the granularity and the dependencies.

## 4. Write each issue

Title: `<verb> <object>` in domain terms. Body:

```markdown
**Intention**
As <role>, I want <capability>, so that <benefit: the value, not the mechanism>.

**Context**
<What a reader who was not in this session needs: what exists, what is deliberately left for later.>

**Expected behaviour**
- <Main flow: action → visible effect.>
- <Rejected case: the reason the user sees; nothing is written.>

**Business rules**
1. <Numbered, so the other sections can cite them.>

**Acceptance criteria**
- <Action> → <observable result>

**Feature path**
1. <The journey a user takes through the UI, CLI or API to see it work: action → what they observe. Domain terms; no ports, selectors or file names.>

**Out of scope**
- <What this issue deliberately leaves out, and where it goes.>

**Depends on**
- #<n> <title>, or "nothing".
```

Use the glossary's terms. When the session settled a decision worth recording, say so and offer `/factory-decision`.

## 5. Publish

Show the final issues and ask before creating them. Create them with `gh issue create`, in dependency order, so each "Depends on" can cite a real number. The factory label (`FACTORY_LABEL` in `.factory/config.sh`, `ai-dev` by default) starts the factory: add it only when the owner asks; otherwise the supervisor's queue picks the issues up when it is enabled. Report the issue links.
