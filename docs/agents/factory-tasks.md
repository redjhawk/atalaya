# Factory tasks

Tasks the Atalaya factory runs. Each task answers the five questions of the `software-factory` skill. Add one automated trigger at a time, with human review on its output, and widen it only once it has earned trust.

## Ticketing

Turns a spec into tracer-bullet tickets. Wired up in `.github/workflows/factory-tickets.yml`.

1. **What triggers it?** A human adds the `factory:tickets` label to a spec issue written by the repository owner. No clock and no other run triggers it.
2. **What does it do?** It reads the spec issue body and only the comments written by the repository owner (the repository is public, so anyone else's comment could carry instructions), `SCOPE.md`, `GLOSSARY.md` and the ADRs, then follows the `to-tickets` skill without the interview. It writes only to the ticket tracker: up to 12 tickets as sub-issues of the spec, with blocking links and a triage label each. It cannot push code: its token has read access to contents and write access to issues only.
3. **What does it trigger next?** Nothing. Tickets are created with the default Actions token, which starts no further runs. Implementation is not automated yet.
4. **Where does a human re-enter?** After the run: the human reads the tickets, edits or closes them, and decides which to hand to an agent. A spec that crosses `SCOPE.md` or contradicts an ADR gets `needs-info` and no tickets.
5. **What stops two runs doing the same work?** One run per spec issue at a time (workflow concurrency). A run that finds existing sub-issues creates nothing. The workflow removes the trigger label when the run ends, even when it fails, so labelling again retries. A failed run comments on the spec with a link to the run; if it left some tickets behind, the retry creates nothing, and a human finishes or deletes them first.

The task needs one repository secret set by a human: `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY`. Without it every run fails and nothing is created.

## Not automated yet

- **Implement**: turning a `ready-for-agent` ticket into a PR. It needs the automated review below as its brake before it gets a trigger.
- **Review**: reading a PR diff cold against the ticket, `SCOPE.md` and the ADRs. Required by `AGENTS.md`, not built.
