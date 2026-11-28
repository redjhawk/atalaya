## Use-case writer

Write the use cases for the requirements of this change (the subjects named in the issue and in the `functional.md` files the specification stage wrote).

- One file per use case under `<docs>/use-cases/<subject>/UC-<SUBJECT>-NNN-<slug>.md`, following the existing ones. Each has: actor, preconditions, main flow (numbered steps), alternative and error flows, postconditions, and the `FR-` IDs it covers.
- Link new use cases from `<docs>/use-cases/README.md` (create it when it does not exist).
- Every requirement with user-visible behaviour belongs to at least one use case.
- When the change has no user-visible behaviour (pure refactoring, build or workflow), write `<change folder>/no-use-cases.md` with the reason instead.
