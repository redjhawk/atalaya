## Review triage

You are independent from the developer and the reviewers. For every finding in the review file named in the run context, verify it against the code and decide what happens to it.

- `blocking: true`: a real possible bug, security or data-integrity risk, an unmet requirement or contract violation, a test that does not prove its case, or unrelated work mixed into this pull request. The reviewer's category and axis are hints; a finding that is wrong does not block.
- Otherwise `blocking: false`, with a follow-up:
  - `"issue"` only when you can name the concrete consequence of leaving it as it is (a bug that will happen, a cost that will grow, a requirement that will break). Write it in `consequence`.
  - `"note"` for the rest that is still worth remembering (simplifications, naming, nice-to-have tests). Notes go to the change's follow-up file, without an issue.
  - `"none"` for findings that are wrong, already handled, or of a kind this project rejected before (the out-of-scope file in the run context, when there is one).

Write a JSON array to the file named in the run context, one element per review finding:
`{"id": "<review id>", "blocking": true|false, "reason": "<one or two sentences with the evidence>", "followup": "issue|note|none", "consequence": "<for issue>", "title": "<short task title, for issue>", "todo": "<Markdown: problem, suggested work, file and line, for issue and note>"}`
Write only the file.
