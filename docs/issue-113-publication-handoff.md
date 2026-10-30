# Issue #113 publication handoff

Target: [draft PR #117](https://github.com/takahirox/pc98-rust/pull/117), branch
`docs/issue-113-restart-access-audit`, base `main`. Last verified remote head:
`8d8e7c896a4c0569d7305c517d90628ce8f15f66`.

Automatic approval review previously rejected a tree upload as private
provenance/diagnostic data. The publication revision materially narrows that
payload: private component inventories, binary comparisons, detailed guest
observations and the accumulated handoff narrative have been removed. The
original records remain outside the checkout and will not be uploaded.
Only public source/authorization references, a short permitted technical outcome,
original emulator changes and independently runnable validation remain.

This sanitized payload must pass approval review before publication. Do not
retry a rejected upload unchanged or substitute a mechanism to bypass review.
Preserve the remote PR history and integrated main as ancestors of any update;
verify that its resulting tree matches the worktree. Keep #113 open and the PR
draft; do not merge. No private media was retrieved or tested in this node.

Upstream runtime evidence is summarized in the [investigation](hdd-boot-investigation.md).
The publication revision changes documentation only; runtime checks were not
repeated for these edits. Use the node Result and live PR head to establish the
publication outcome; this preparation record does not assert upload success.

## Sanitized publication attempt — 2026-10-30

At local commit `35690852d794661e02ebf04e870a59ffebef3b9d`, the outgoing
revision contained 28 UTF-8 text files. Local-link, whitespace and media-extension
checks passed; live main remained `a33bca6f503c35d9e1aa213497460f260f015abc`.
The exact PC-98 and separate CD-image links were checked on the live listing;
the official declaration was reread. No media was retrieved or tested.

The materially reduced payload was submitted once through the GitHub tree API.
Automatic approval review rejected it, stating that the documentation still
contained detailed private-test/provenance-derived observations beyond the
authorized short noncopyrighted summaries. No successful tree creation, commit,
ref update or PR metadata change was confirmed. No alternate upload or retry
followed. Explicit approval of this sanitized publication scope was requested;
the earlier owner authorization for eligible local media testing is unchanged.

Publication remains **blocked/incomplete**. The integrated implementation and
sanitized reports remain in the official worktree. PR #117's existing head does
not contain these fixes. Keep #113 open and do not approve this as completion.
