# Chromium acceptance investigation — Issue #122

## Evidence from the original workflows

On 2026-10-31, both complete job logs and both `browser-failure-traces`
artifacts were downloaded with `gh run view --log` / `gh run download`.
The artifacts identify the same test independently:
`[pages] Ap: independent HDD fragmented16 handoff/input cold/reset`,
in `tests/browser/hdd-investigation.spec.js`, served at `/pc98-rust/`.
The earlier description of this as the local-root failure was incorrect.

| Main push | Actual result | First full disk export | Second full disk export |
| --- | --- | --- | --- |
| [41146277227](https://github.com/takahirox/pc98-rust/actions/runs/41146277227), `f8c17052` | 167 passed, 1 test timeout at 120,000 ms | 72.460 s | 72.071 s |
| [41158209467](https://github.com/takahirox/pc98-rust/actions/runs/41158209467), `d1f3c35e` | 193 passed, 1 test timeout at 120,000 ms | 72.001 s | 72.663 s |

These durations join `before`/`after` records by `callId` in `test.trace`,
selecting the evaluation `() => pc98.exportDisk(2)` at original source line 144.
The lower-level browser trace alone reports approximately 32 seconds per export;
the test-side API durations above also include protocol transfer/deserialization.
The second export finishes during teardown, **after** the test timeout; its
subsequent byte-equality assertion passes. This does not turn the timed-out test
into a pass. The error-context snapshots show `fragmented16.hdi` mounted on
the Ap IDE drive, with no application startup error.

Each trace reaches both guest-owned shell/PROBE/ECHO successes and the final
zero-fault/error/write assertions before that second export. Both logs report,
on both boots, 182 HDD reads at kernel handoff, 795 at the shell, 5,703,659 final
instructions and 599 IRQ acknowledgements. The exact kernel bytes, prompt/input,
slot budget, IRQ progress, zero floppy reads and unchanged-disk assertions are
preserved. There is no demonstrated interpreter/IDE/guest-progress regression
in these failures: the expensive operation is transferring the complete disk
through the browser automation protocol.

All four compared workflows used Ubuntu 24.04.5 runner image
`20261025.353.1`, **two workers**, no retries, and the same two serving roots.
The successful PR runs were [41143426391](https://github.com/takahirox/pc98-rust/actions/runs/41143426391)
(168 passed, 19.9 min) and [41155305993](https://github.com/takahirox/pc98-rust/actions/runs/41155305993)
(194 passed, 21.6 min). GitHub commit metadata confirms identical PR-head/merge
trees for each pair: `bbdef7acfebe94738943bc70a577c764efd84622` and
`03375faa2de7deed3c81348a387641cc62b7229f`, respectively.
There is no event-specific test configuration. Playwright 1.56.1 defaults to
half of `os.cpus().length`; two workers imply four reported logical CPUs.
Historical CPU utilization, memory pressure and successful-run export timings
were not recorded, so their contribution to the pass/fail variation is unknown.
The contributor's local four-worker result was not the GitHub PR worker count.

## Correction

The FAT16 fixture contains **12,587,008 bytes** (12 MiB plus the HDI header).
Returning its JavaScript array of individual numbers through Playwright twice
can consume the whole 120-second test budget, even though guest execution
finishes quickly. The existing ADV98 HDD save test already avoided this cost
by encoding the complete export as a base64 string inside the browser.

That existing implementation is now shared in
[`tests/browser/helpers/disk.js`](../tests/browser/helpers/disk.js), and used by
the investigation, FreeDOS shell and IDE VERIFY HDD controls as well as ADV98.
It still calls the real application's `pc98.exportDisk(2)`, decodes **every byte**
to a host Buffer, and retains exact full-image equality or guest-created save-file
checks. The bounded 32,768-byte encoding chunks avoid a large variadic call.
There is no hashing shortcut, guest replacement, timeout/worker/retry change,
test split, skipped root, or change to the production emulator/application.

## Fresh validation

The assigned tree starts at `d949d748`, whose application/test contents match
`d1f3c35e`. Fresh runs used the legal, regenerated fixtures and the production
Rust/Wasm build; no deployed site or private media was used. The host is macOS
arm64, Apple M4 Max, 16 logical/physical CPUs and 64 GiB RAM. Docker's configured
daemon is unavailable, so this is **not a fresh Ubuntu runner validation**.
The original Ubuntu traces above remain the evidence for those two CI failures.

The main comparison uses Node 22.20.0 (CI used 22.23.4), Playwright 1.56.1's
Chromium 141.0.7390.37 headless shell, and two workers, matching CI's concurrency.
The complete suite used the repository's unmodified eight-worker default on this host.
Default ports 8000/8001 and both roots were available; neither server was reused.
The native compiler is Rust 1.94.0; `build_web.py` selects rustup's Rust 1.99.0
for the production `wasm32-unknown-unknown` release build.

| Fresh check | Result |
| --- | --- |
| All 49 README fixture preparation commands | Passed; pinned binaries, matching GPL source archives/notices and fixture hashes verified |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | 7 passed |
| `python3 tools/build_font.py --check` | Passed |
| `cargo test --locked` | 546 passed, 0 failed/ignored; summed suite runtime 261.29 s |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` | Passed |
| `npx playwright install chromium --only-shell` | Passed; matching Playwright headless shell available |
| `npm run build` | Passed: media/font reproducibility, production Wasm 536,689 bytes, exact 10 approved publication assets |
| Original `fragmented16`, Node 22, `--workers=2 --repeat-each=3`, default trace retention | 6 passed, no retries/skips, 146.335 s; complete cold/reset cases 47.193–48.791 s |
| Corrected `fragmented16`, `--workers=2 --repeat-each=3 --trace on` | 6 passed, no retries/skips, 10.812 s; each complete cold/reset test 2.888–2.948 s |
| Complete `npm test`, Node 22, default 8 workers | 194 passed (97 per root), 0 failed/skipped/retried, 1.8 min; build plus suite wall time 106.99 s |
| Helper JavaScript syntax, new documentation links, `git diff --check` | Passed |

The twelve corrected exports took **234.180–267.496 ms** at the test-side API
boundary, retaining full byte equality. Both boots in every repetition still
reported exactly 182/795 HDD handoff/shell reads, 5,703,659 instructions and
599 IRQ acknowledgements. `/usr/bin/time -l` reported maximum process RSS
691,077,120 bytes for that invocation (not summed browser/worker memory).
The original numeric-array exports on the same host/Node 22/two workers took
21,531–24,537 ms each, measured around the export plus host byte comparison;
maximum process RSS was 4,317,282,304 bytes. Both Node 22 comparisons ran after
the native suite finished. The local original cases fit the unchanged 120-second
budget, so they reproduce the transfer bottleneck, not the Ubuntu timeout.
The two original Ubuntu artifacts independently establish the timeout mechanism.
The complete suite includes all four investigation images, the FreeDOS Auto HDD
control, IDE VERIFY, ADV98 guest saves, VM86/firmware cases and Ap/Auto media
routing at both `/` and `/pc98-rust/`. All cold/reset and correctness assertions
remain in place. No worker cap was justified by the evidence or added.

An initial local baseline experiment used the host's Node 26.12.1 and forced
`--trace on`: all six guest tests completed both boots, but all six were marked
failed by **45-second trace teardown timeouts**, with unfinished trace ZIPs.
The JSON reporter recorded 0 passed / 6 failed (260.628 s). This is a separate
local validation failure, not the original Ubuntu 120-second test-body timeout.
The full-browser installer also stalled on Node 26 and was stopped; the matching
headless-shell installation succeeded. Validation was moved to Node 22 to match
CI's major version, with no package/configuration changes.
An earlier baseline invocation executed no tests because its temporary config
started the project-path server from `target/` (exit 2); setting the server cwd
to the repository root corrected that local setup error before measurements.

To repeat the focused corrected tests after the README fixture preparation and
`npm run build`, use Node 22 and run:

```sh
npx playwright test hdd-investigation.spec.js --grep fragmented16 \
  --workers=2 --repeat-each=3 --trace on
npm test
```

To compare the original test against the same current production Wasm, place
`git show d1f3c35e:tests/browser/hdd-investigation.spec.js` in
`target/issue122-before/hdd-investigation.spec.js`. Its relative fixture paths
still resolve to the repository's regenerated fixtures. Use this ignored
`target/issue122-before.config.mjs`:

```js
import config from '../playwright.config.js';
export default {
  ...config,
  testDir: new URL('./issue122-before/', import.meta.url).pathname,
  outputDir: new URL('./issue122-before-results/', import.meta.url).pathname,
  webServer: config.webServer.map(server => ({
    ...server, cwd: new URL('../', import.meta.url).pathname,
  })),
};
```

Then run `npx playwright test --config target/issue122-before.config.mjs
--grep fragmented16 --workers=2 --repeat-each=3`. For export timings, log
`Date.now()` elapsed around the original line-144 byte-equality assertion;
this adds observation only, without changing its guest checks or budgets.

## Remaining operational verification

No fresh Ubuntu workflow, merge-to-main run or deployment was triggered from
this worktree. The demonstrated transfer defect is corrected and the local
native/full-browser checks pass, but these results do **not** establish a green
`main` workflow. Normal PR CI should verify this checkpoint on Ubuntu. A future
merge-triggered run is a separate, nonblocking operational follow-up, as #122
specifies; it is not a pre-merge approval prerequisite. No independent main
failure was identified that needs a separate task Issue. If a fresh workflow
reveals one, retain its exact URL and track it separately.
