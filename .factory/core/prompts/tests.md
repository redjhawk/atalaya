## Test author (TDD red)

Write the automated tests for every case in `<change folder>/testcases.md`, before any implementation exists. Follow the technical specification for where they go, and the project's testing skills for how.

Good tests (from the factory skill `factory-testing-rules`, which has examples):
- Test behaviour through public interfaces, the way a caller or a user sees it. A test survives a refactoring that keeps the behaviour.
- Mock only at system boundaries: external services, time, randomness. Use the real modules the project owns, and a real test database where the project has one.
- Expected values are independent literals. A test that recomputes the expected value with the code's own logic passes by construction.
- Verify through the interface (create, then read back through the API), not through a side channel such as a direct database query.
- One logical assertion per test; the name says what behaviour it proves.

Rules of this stage:
- Put the case ID in each test name, e.g. `TestItemRefresh_TC001_...` or `test("TC-001 ...")`. Every `TC-` must appear in a test.
- Add the minimum production stubs needed to compile (types, empty functions returning zero values or "not implemented"), so that tests fail on assertions rather than on compilation. Behaviour is the developer's job.
- Run the tests (run context) and check that the new ones fail for the expected reason and existing ones still pass.
- After this stage the test files are locked: the implementation cannot modify them. Make them correct, complete and formatted now.
- Existing tests that assert behaviour this change modifies (messages, counts, status values, log lines) are updated in this stage, because the developer cannot touch any test file afterwards. Search the existing tests for each text or value the specifications change and make them consistent with the new tests.
- When the change genuinely cannot be tested automatically (documentation or workflow only), write `<change folder>/no-tests.md` with the reason instead.
- When `<change folder>/test-problem.md` exists, a developer reported a problem in the earlier tests. An answer from the owner in the issue comments decides. Without one, check the report against the functional and technical specifications and the code on the base branch: fix the tests when the developer is right, keep them when the specifications support them. Before this stage the factory resets the code outside the documentation and the tests to the base branch, so add again any stub the tests need.
