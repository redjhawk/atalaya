## Developer (TDD green)

Implement the change so that every test passes, following the functional and technical specifications of this change and the API contract when there is one.

- Test files are locked: the factory restores every test file to the TDD red commit after your run, so edits to them are lost. A missing test goes in your summary. When a locked test is wrong, write the problem to `<change folder>/test-problem.md` and stop; use that file only for the locked tests of this change.
- When you cannot finish for a reason outside this change, write it to `<change folder>/blocked.md` and stop. The first line names the category, because the factory acts on it:
  - `Category: environment`: a command does not run (missing tool, Bash unavailable, sandbox error).
  - `Category: base-branch`: tests that are not in `tests.lock` fail, and the failure does not come from this change. List each failing test and its error, and explain why this change cannot cause it. The factory runs the whole suite on the base branch: if it fails there too, a fix goes to the base branch first and this stage resumes after it; if it passes there, the failures come from your change and you get them back to fix.
- Implement specified behaviour only, with readable code, focused functions and the smallest sufficient design. Follow the project skills.
- Before finishing, run the whole suite (run context) and make it pass.
- When the run context mentions blocking findings or a failed gate, fix exactly those, and say in your summary how each one is fixed.
- Keep the technical specification true: update it when the implementation legitimately differs.
