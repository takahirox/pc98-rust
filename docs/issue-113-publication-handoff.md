# Issue #113 publication handoff

Publication is blocked; implementation files remain in this worktree. Do not
interpret this checkpoint as an updated PR or approval to merge.

- Target: <https://github.com/takahirox/pc98-rust/pull/117> (open, draft).
- Branch: `docs/issue-113-restart-access-audit`, base `main`.
- Last verified GitHub head: `8d8e7c896a4c0569d7305c517d90628ce8f15f66`.
- Assigned implementation checkpoint: `44ef43e3fd11d4003e333961366c2d065e666c4a`.
- Implementation commit: `a89fe005533ba58a25fc001e8a78beef131a69ac`.
- Implementation tree: `fafe448b5f9fbef657723130bd8cb533b5588890`.

The user authorized updating the existing PR. Automatic approval review rejected
the GitHub tree upload, then rejected it again after a publication audit because
it classified the investigation report's component provenance and diagnostic
outcomes as private data. No GitHub tree, commit, ref or PR metadata update was
confirmed. An explicit publication-approval question is pending; do not retry or
use another upload mechanism to bypass this rejection.

The local audit found 20 changed text files relative to the existing PR head,
zero binary changes, no tracked private media/log/dump files and no added private
host paths. The only three newly added SHA-256 values belong to the original MIT
VM86 regression fixture. Its offline builder successfully reproduced the pinned
manifest. Whitespace checks passed. Runtime suites were not repeated in this
publication node; their results below are the upstream implementation evidence.

On resumption, inspect the live PR head before updating it. The remote publication
history differs from the GitWeave checkpoint history, although its current tree
matches the earlier implementation checkpoint. Preserve the remote history and
verify that the resulting implementation files match this worktree. Publish no
private media, private hashes, detailed traces, disassembly, memory dumps or
screenshots. Keep #113 open; do not merge or add automatic closing keywords.

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
- Decode valid 386 extended-copy descriptor limits and enforce DMA boundaries.
- Add original MIT native and production-Wasm regressions with independent
  fail-before/pass-after evidence, fixture preparation and CI integration.
- Record the access audit and bounded technical findings. Earlier stopping
  assumptions were not established environment restrictions or download failures;
  existing operator confirmation and authorization were reused. Component
  provenance limits remain documented separately from private execution.

See [the investigation report](docs/hdd-boot-investigation.md). Private media and
detailed diagnostics remain outside all publication and CI surfaces.

### Validation

The upstream implementation node reports:

- All documented fixture/source/license preparations passed; seven fixture-fetch
  unit tests passed.
- `cargo test --locked`: 535 passed.
- Clippy with warnings denied, formatting, whitespace and documentation-link
  checks passed.
- Production Rust/Wasm build and approved-assets audit passed: ten approved assets.
- Chromium production-Wasm suite: 166 passed across `/` and `/pc98-rust/`, including
  original VM86 and IRET cold/reset coverage; no skipped tests or final-run retries.
- Optional private cold/reset testing established command execution and input
  response without media modification. Full playability remains unclaimed.

Publication checks: outgoing diff/whitespace inspection and offline original MIT
fixture rebuild passed. Runtime suites were not repeated during publication.
GitHub CI for a future updated head remains unverified.

### Required post-merge verification

None.
