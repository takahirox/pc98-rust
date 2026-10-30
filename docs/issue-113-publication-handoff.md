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
