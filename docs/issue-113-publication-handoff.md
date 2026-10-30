# Issue #113 publication handoff

Publication is blocked; implementation files remain in this worktree. Do not
interpret this checkpoint as an updated PR or approval to merge.

- Target: <https://github.com/takahirox/pc98-rust/pull/117> (open, draft).
- Branch: `docs/issue-113-restart-access-audit`, base `main`.
- Last verified GitHub head: `8d8e7c896a4c0569d7305c517d90628ce8f15f66`.
- Previous assigned implementation checkpoint: `44ef43e3fd11d4003e333961366c2d065e666c4a`.
- Previous implementation commit: `a89fe005533ba58a25fc001e8a78beef131a69ac`.
- Previous implementation tree: `fafe448b5f9fbef657723130bd8cb533b5588890`.

The subsequent fix node integrated current main
`a33bca6f503c35d9e1aa213497460f260f015abc` locally, resolving all conflicts and
preserving both independent VM86 fixtures plus the IRET regression. It retains
main's BIOS-only scatter transfers alongside the HDI boot/input corrections.
The original monitor now handles the architectural HLT #GP; only independently
authored fixture hashes changed. The integrated revision was privately retested
cold/reset for DOS command execution and paired game input, with unchanged media.
This is local evidence; the published PR head above is unchanged.

The user authorized updating the existing PR. Automatic approval review rejected
the GitHub tree upload, then rejected it again after a publication audit because
it classified the investigation report's component provenance and diagnostic
outcomes as private data. No GitHub tree, commit, ref or PR metadata update was
confirmed. An explicit publication-approval question is pending; do not retry or
use another upload mechanism to bypass this rejection.

The prior publication audit found 20 changed text files relative to the existing PR head,
zero binary changes, no tracked private media/log/dump files and no added private
host paths. The only three newly added SHA-256 values belong to the original MIT
VM86 regression fixture. Its offline builder successfully reproduced the pinned
manifest. Whitespace checks passed. Runtime suites were not repeated in this
publication node; their results below are the upstream implementation evidence.

On resumption, inspect the live PR head before updating it. The remote publication
history differs from the GitWeave checkpoint history, although its current tree
matches the earlier implementation checkpoint. Preserve the remote history and
verify that the resulting implementation files match this worktree. Preserve
current main as an ancestor of the published update as well as the remote PR
history, so the conflict resolution is retained. Publish no
private media, private hashes, detailed traces, disassembly, memory dumps or
screenshots. Keep #113 open; do not merge or add automatic closing keywords.

## Review-fix checkpoint verification — 2026-10-30

The next fix node starts at `1dfb7c2b55018bed6ec26277770344d1d60af7d5`.
Its assigned review findings describe the older published tree. Read-only live
checks still find main at `a33bca6f503c35d9e1aa213497460f260f015abc` and PR #117
at `8d8e7c896a4c0569d7305c517d90628ce8f15f66`, with `mergeable=false`.
Locally, main is already an ancestor; `git merge-tree --write-tree HEAD a33bca6`
succeeds without conflicts. All 49 README fixture preparation commands remain
in CI, including both VM86 guests and the IRET guest. The VM86 firmware path,
CPU paging/fault transaction and extended-copy descriptor correction are already
present in the checkpoint. No additional emulator change is justified by these
findings against this local tree.

The live distributor listing was checked successfully with a direct HTTP
request (200), retaining the exact PC-98 ZIP link and separate CD-image link.
The official declaration was reread. This node neither retrieved nor executed
private media; the earlier integrated cold/reset results above are inherited
evidence, not new observations. The historical access audit and remaining
component-provenance qualifications remain applicable.

Fresh checks on this checkpoint passed all 52 documented Python preparation/check
commands (49 fixture builders, seven fixture-fetch unit tests, media and font
checks), `cargo test --locked` (546 passed, zero failed or ignored),
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and the native
release build. Documentation local-link checks and the tracked-file audit passed;
no HDI, ZIP/RAR, dump or trace files are tracked.
`npm ci` and `npm test -- --workers=2` passed: production Rust/Wasm compilation,
the audit of ten approved publication assets, and **168 Chromium tests** at `/`
and `/pc98-rust/` (6.5 minutes), with no failures, skips or retries. Both VM86
guests and IRET cold/reset acceptance passed at each root. No emulator, guest,
fixture, test or CI files changed during this revalidation.

The assigned node explicitly prohibits pushing or merging. It therefore cannot
resolve the findings on the published PR or validate a new GitHub head. The prior
automatic approval rejection of the tree upload, classified as publishing
private diagnostic/provenance data, is also preserved above; this node makes no
upload attempt or approval retry. Do not treat this validation record as
publication, a substitute for resolving that rejection, or completion of the
remote review. Keep #113 open. A subsequent authorized publication step must
preserve both histories, update the PR evidence and run checks on its resulting
head before approval.

## Proposed PR title

Fix HDI boot/input with IRET wrapping, paged VM86 BIOS and XMS copies

## Proposed PR body

### Problem and related Issues

Related to #113 and #118. The reported Ap/IDE HDI boot/input failure was privately
reproduced. PR #114 remains baseline independent-control work. Keep #113 open for
review; this PR does not claim complete game playability.

### Outcome

The upstream investigation identifies and corrects real-mode IRET frame wrapping,
paged VM86 firmware access and valid 386 extended-copy descriptor handling.
Private cold/reset retests of the unmodified working HDI establish DOS command
execution and bounded game launch/input response. The earlier persistent fallback
display and firmware stop are cleared in that tested configuration. This is not a
manual playthrough or a compatibility claim for other profiles.

### Focused changes

- Validate real-mode IRET operands separately across stack-offset wrapping.
- Execute supported VM86 BIOS operations through restartable CPU paging checks,
  including fault recovery, mapped buffers, callbacks and keyboard waits.
- Decode valid 386 extended-copy descriptor limits and preserve BIOS scatter transfers.
- Add original MIT native and production-Wasm regressions with independent
  fail-before/pass-after evidence, fixture preparation and CI integration.
- Record the access audit and bounded technical findings. Earlier stopping
  assumptions were not established environment restrictions or download failures;
  existing operator confirmation and authorization were reused. Component
  provenance limits remain documented separately from private execution.

See [the investigation report](https://github.com/takahirox/pc98-rust/blob/docs/issue-113-restart-access-audit/docs/hdd-boot-investigation.md). Private media and
detailed diagnostics remain outside all publication and CI surfaces.

### Validation

The integrated fix node reports:

- All 52 documented Python preparation/check commands passed; seven fixture-fetch
  unit tests passed. All 49 fixture preparations remain in README and CI.
- `cargo test --locked`: 546 passed.
- Clippy with warnings denied, formatting, whitespace and documentation-link
  checks passed. The same original descriptor regression fails on current main
  and passes on the integrated revision.
- Production Rust/Wasm build and approved-assets audit passed: ten approved assets.
- Chromium production-Wasm suite: 168 passed across `/` and `/pc98-rust/`, including
  both original VM86 fixtures and IRET cold/reset coverage; no skipped tests or final-run retries.
- Optional private cold/reset testing established command execution and input
  response without media modification. Full playability remains unclaimed.

Publication checks: outgoing diff/whitespace inspection and offline original MIT
fixture rebuild passed. Runtime suites were not repeated during publication.
The fix node reran the complete native and browser checks listed above after
integration; publication approval remains pending. GitHub CI for a future updated
head remains unverified.

### Publication node recheck — 2026-10-30

The live PR metadata still reports an open draft at
`8d8e7c896a4c0569d7305c517d90628ce8f15f66`. The assigned checkpoint is
`81f3c65d90376b11234835a7e2c7757a743d250e`; its history includes both the
integrated main commit and implementation commit `e98fc5b` above. The outgoing
checkpoint diff contains 28 text files, zero binary changes, and passes
`git diff --check`. Runtime checks were not repeated by this publication node;
the integrated fix node's results above remain the validation evidence.

The existing automatic-review rejection and pending publication question remain
unresolved. This node made no upload attempt, branch update, PR metadata change,
merge or Issue closure. Implementation files and this handoff remain in the
official worktree for the next checkpoint. Publication is incomplete; the remote
PR does not yet contain the integrated fixes. Resume only after the publication
blocker is resolved, preserving both remote PR history and integrated main.

### Required post-merge verification

None.

## Publication recheck from checkpoint 48b9238 — 2026-10-30

Read-only GitHub metadata confirms that PR #117 remains open, draft and unmerged
at `8d8e7c896a4c0569d7305c517d90628ce8f15f66`, with `mergeable=false`.
Its live base is `04cb05978486f7413bc79591487d9305f80fa140`; a local
merge-base check confirms that this base is already an ancestor of the assigned
checkpoint. No new integration is needed for that observed base. The outgoing
checkpoint diff still has 28 text files, zero binary changes and passes
`git diff --check`. Runtime suites were not repeated; the earlier 546 native and
168 browser results remain inherited validation evidence.

The previous automatic approval rejection remains unresolved. It rejected the
tree upload as private diagnostic/provenance data. This node neither retries the
upload nor substitutes another publication mechanism. There was no push, GitHub
tree/commit/ref mutation, PR metadata update, merge or Issue closure. This node
did not retrieve or test private media. The integrated fixes remain local, and
publication of this worktree is **blocked/incomplete**. Resolving the existing
publication approval is distinct from the already granted authorization for
eligible private local-only retrieval and testing; no renewed media-access
permission is requested.
