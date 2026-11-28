---
name: factory-testing-rules
description: What makes a test prove behaviour, and where to mock. Use when writing, designing or reviewing tests, test cases or mocks.
---

<!-- The rules below summarise tests.md and mocking.md, copied from mattpocock/skills (MIT, see LICENSE.md). -->

A good test proves behaviour a caller or a user cares about, through a public interface, and survives any refactoring that keeps that behaviour.

- **Interface, not implementation.** Call the public API; assert on what it returns or what a user sees. Mocking internal collaborators, testing private functions, and asserting call counts or call order couple the test to the implementation.
- **Mock at system boundaries only**: external services, time, randomness, sometimes the file system. Use the real modules the project owns, and a test database where the project has one.
- **Independent expected values.** Write the expected value as a literal or a state you know. A test that recomputes it with the code's own logic is *tautological*: it passes by construction.
- **Verify through the interface.** After creating something, read it back through the API, not through a direct database query or another side channel.
- **One logical assertion per test**, and a name that says *what* behaviour it proves, not *how*.

Examples of each rule, good and bad: [tests.md](tests.md). Designing code so that boundaries are easy to mock (dependency injection, one function per external operation): [mocking.md](mocking.md).
