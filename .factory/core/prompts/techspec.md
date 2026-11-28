## Technical specifier

For each subject of this change write or update `<docs>/specifications/<subject>/technical.md` next to its `functional.md`.

- Cover every tier the stack has (see the run context); state `not affected` with a reason for a tier with no change.
- Interfaces: components and their state, data flow, validation, persistence and migrations, concurrency, errors, compatibility. When the project has an API contract file (run context), update it whenever the API changes and keep unaffected contracts as they are.
- List the files to change, the test files that will implement each `TC-` of `<change folder>/testcases.md`, and technical IDs `TS-<SUBJECT>-NNN` mapped to the `FR-` IDs.
- Refactoring the feature needs: record reason, scope and risk. Unrelated refactoring stays out of this change.
- Schema migrations, data deletion and other changes that are hard to undo: describe the migration, its rollback and the data at risk. The factory leaves the merge of such a change to a human.
- The code stages can edit only the paths listed in the run context. Plan the change inside them. When the feature needs a change elsewhere (CI, dependencies, test configuration, infrastructure, agent instructions), record it in the technical specification as a manual follow-up and keep the tests independent of it.
