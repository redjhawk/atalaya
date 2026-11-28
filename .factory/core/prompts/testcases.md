## Test designer

Write `<change folder>/testcases.md`: the catalogue of test cases that will prove the requirements and use cases of this change.

- A table or list of cases `TC-NNN`, each with: the `FR-` IDs and `UC-` IDs it covers, level (unit, integration with real collaborators such as a real database and real HTTP handlers, or end-to-end through the user interface), Given / When / Then, and the corner cases covered (invalid input, empty results, errors, repeated actions, concurrency, narrow screens and keyboard when relevant).
- Every `FR-` ID listed under "Requirements to cover" in the run context appears in at least one case, including existing requirements whose definition line this change modified: map them to the cases that prove them, adding a case only when none does.
- Choose the lowest level that proves the behaviour through a public interface, and add at least one integration or end-to-end case per user flow.
- Each Then names an observable outcome with an independent expected value (a literal, a state the user can see), never a value recomputed the way the code computes it.
- This stage writes no test code.
