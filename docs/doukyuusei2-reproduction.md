# Reproduce before further optimization — Issue #146

This investigation starts from `0e8d502`, after PR #148. The user explicitly
requests reproducing the slow image changes and unstable audio before choosing
another optimization. The installed base-game HDD, original floppies, captured
images, profiles and working reports remain local. The emulator's Rust, Wasm,
Worker, renderer and AudioWorklet implementations are unchanged in this work.

## Measurement fault and correction

The pinned Playwright 1.56.1 Firefox is a patched test browser. Its
[Runtime](https://github.com/microsoft/playwright/blob/v1.56.1/browser_patches/firefox/juggler/content/Runtime.js)
and [Worker initialization](https://github.com/microsoft/playwright/blob/v1.56.1/browser_patches/firefox/juggler/content/WorkerMain.js)
create debuggers observing Wasm globals. That forces debuggable baseline
compilation. Upstream's
[later Runtime implementation](https://github.com/microsoft/playwright/blob/v1.64.0/browser_patches/firefox/juggler/content/Runtime.js)
explicitly permits unobserved Wasm and documents why those flags matter.

Starting the patched executable without a driver does not remove the effect:
its frame actors still initialize the Worker agents. A causal desktop control
copies that exact browser into the task's temporary directory and changes only
the `allowUnobservedWasm`/`allowUnobservedAsmJS` flags in the two debugger
constructors. The native executable SHA-256 is identical before and after.
The installed browser cache is untouched; the modified copy is only a diagnostic.

All controls use the same release Wasm, SHA-256
`c424157247e0bb99bebecbe34ea3ae66fbd446395866653ee77ac4c9113061cf`,
on Apple M4 Max, Darwin 27.2.0 arm64, Node 26.12.1:

| Firefox 142.0.1 desktop control | Guest seconds / wall second | Mean host ms per guest slice | Missing source samples |
| --- | ---: | ---: | ---: |
| Patched browser, no driver connected | 0.3493 | 28.496 | 304,576 |
| Same binary, debugger observation flags corrected | 0.9999 | 5.374 | 0 |
| Mozilla stock release, no driver | 1.0000 | 5.270 | 0 |

These are short causal controls, not a general browser ranking. The stock release
comes from Mozilla's [official Firefox 142.0.1 macOS archive](https://archive.mozilla.org/pub/firefox/releases/142.0.1/mac/en-US/Firefox%20142.0.1.dmg);
the downloaded 139,569,965-byte DMG matches the release's
[SHA-512 manifest](https://archive.mozilla.org/pub/firefox/releases/142.0.1/SHA512SUMS):
`568b32cb30de13fa779ca91ee7645f1ec942bfc8c8d976f0b21a071bf282ff670461af5078d57a19acf3071a3db910cffd5c47eaeebeb2771c1726e340cfc891`.

The historical Firefox figures in the [runtime](runtime-performance.md),
[renderer](render-worker.md) and [second game evaluation](doukyuusei2-performance-round2.md)
reports therefore describe induced debug execution. The earlier explanation
that ordinary Firefox lacks the required interpreter capacity is withdrawn.
The bounded starvation-preview behavior remains valid under that induced load;
an ordinary-Firefox improvement from PR #148 was not established.

The three old measurement tools now reject Firefox unless `--debugTier=1`
explicitly requests that condition, and their reports label it. Chromium
measurements are unaffected. No dependency upgrade or browser modification is
required by the normal reproduction tool.

## Normal startup and playback control

Build once using the existing README commands and use the clean installed HDD
from the [installation procedure](doukyuusei2-performance.md). Run one browser
at a time with no builds or regression suites competing for CPU:

```sh
node tools/reproduce_doukyuusei2.mjs --browser=chrome --executable="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-146/chrome-normal.json
node tools/reproduce_doukyuusei2.mjs --browser=firefox --executable="/Applications/Firefox.app/Contents/MacOS/firefox" --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-146/firefox-normal.json
```

Pass the actual stock executable's absolute path. The tool rejects known
Playwright-patched Firefox layouts. Both media and raw report destinations must
be outside the checkout. Reports use exclusive creation so an earlier result
cannot be silently overwritten. The tool downloads no browser or game.

The tool launches an owned visible browser with a fresh temporary profile.
Automatic audio activation is enabled in that profile; Firefox welcome tabs are
disabled so onboarding cannot hide and pause the emulation tab. No automation
protocol or Worker debugger is attached. The loopback server on port 8079 serves
frozen runtime bytes and the supplied HDD through a temporary private route;
the game never enters `web/`, version control or publication.

The served local client has two bounded input/audio getters appended. Their
source and served hashes are recorded separately. Worker, renderer, worklet and
Wasm bytes stay unchanged. The controller imports the HDD through the ordinary
file handler, resets, enables sound, waits for DOS in real time, types the launch
commands and selects color mode. It accepts the default name through guest PPI
mouse input, then sends 48 clicks with 100 ms down and 500 ms up. It returns to
the main classroom menu, selects Listen and its first topic, then sends another 48 clicks
to exercise the flashback CG and return to the classroom. `--clicks=1..240`
changes this second observation length. Guest time is never advanced manually
and PCM is never discarded to accelerate setup. This control bypasses the front-end's
relative-pointer transform for calibrated guest input; direct screen-pointer
behavior is investigated separately.

Records begin at audio-enabled boot and include phase/click times, guest time,
PCM production/consumption, queue bounds, music/IRQ/disk counters, frame counts,
and nine small CG-area patch fingerprints sampled every 250 ms. Pixels and audio
samples are not exported. A hidden/paused or faulted playback window is invalid;
its error is retained rather than reported as healthy throughput.

Device-output statistics are recorded separately when supported. Modern
`AudioContext.playbackStats` uses seconds. The older `playoutStats` values are
converted from milliseconds, following the
[Web Audio draft](https://webaudio.github.io/web-audio-api/#AudioPlaybackStats)
and [original playout proposal](https://wicg.github.io/audio-context-playout-stats/).
Unsupported output metrics stay unknown. Queue and device statistics cover
delivery; pitch, timbre and synthesis fidelity require an appropriate reference.

## Findings so far

### Repeated Chrome failure with debugging enabled

Installed Chrome 157.0.8103.38 reproduces delivery gaps when the emulator Worker's
`Debugger` domain is enabled **before** ordinary audio-enabled boot and scene
loading. Two fresh pages follow the same launch/name/prologue input sequence:

| Chrome condition | Wall s | Guest s / wall s | Missing source samples | Presented fps | Mean host slice ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Debugger enabled, first run | 61.114 | 0.9366 | 185,031 | 12.94 | 9.983 |
| Debugger enabled, repeat | 60.965 | 0.9351 | 189,607 | 12.94 | 10.010 |
| Debugger disabled afterward, first run | 9.806 | 1.0005 | 0 | 24.99 | 4.387 |
| Debugger disabled afterward, repeat | 9.782 | 0.9998 | 0 | 24.94 | 4.487 |

The missing samples represent about 3.85/3.95 seconds of source starvation.
AudioContext device-output underruns remain zero: its graph continues rendering
the worklet's silence. That distinction matters when diagnosing audible gaps.
These are time-paced operations, so their guest progress and music counts vary.
Guest clock configuration and emulator bytes are unchanged.

A separate attach/on/off control with a stationary classroom scene has no
starvation: merely attaching a protocol session leaves slice cost near 3.77 ms;
enabling the debugger raises it to 8.28 ms, and disabling it restores 3.72 ms.
The heavier boot/scene path is needed to exceed the producer's budget here.

The [V8 compilation documentation](https://v8.dev/docs/wasm-compilation-pipeline#debugging)
explains the replacement of optimized Wasm code with baseline code during
debugging. It also documents that starting a Performance recording optimizes
Wasm again. A CPU profile can therefore hide this particular failure; its timings
must be kept separate from the debugger-on reproduction.

This is a repeatable **conditional** Chrome failure. The user subsequently
confirmed that DevTools are closed when the reported problem occurs, so this
condition does not match the original report. CPU prototypes that improved only
this condition are not accepted as a fix for the user's symptom. Investigation
continues with ordinary debugger-free Chrome.

### Normal execution and unresolved observations

Earlier interactive Chrome controls ran from normal audio-enabled boot for
674.5 seconds and 457.0 seconds, through the classroom prologue. Both maintained
real time with zero missing source samples. The second also recorded zero
device underruns. These are exploratory observations, not proof that the user's
original Chrome symptom is absent.

The initial normal-startup controller observes about 64 seconds per run,
including launch, name entry, background/portrait loading and music. Initial
normal desktop results (same unchanged release Wasm):

| Browser / run | Wall s | Guest s / wall s | Missing source samples | Presented fps | Device underrun s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Chrome, first | 63.704 | 1.000100 | 0 | 26.074 | 0 |
| Chrome, repeat | 63.780 | 1.000007 | 0 | 25.651 | 0 |
| Stock Firefox, first | 63.824 | 0.999946 | 0 | 26.228 | Unsupported |
| Stock Firefox, repeat | 63.838 | 1.000040 | 0 | 25.706 | Unsupported |

All four end with 973 HDD reads and 2,172 FM key edges. They reach the classroom
menu; repeated clicks at one location do not establish progress into later scenes.
The first three use harness SHA-256
`e4fb1744dd5b58e78500f47d0018f1c5be4b0cbf1d3e8ebb256ea75ed17bed98`;
the Chrome repeat uses
`4984a35a78b12da38f3613b241d284d6d00ee1a668e2416a35a07c6ec8495148`
with a final failure check added. Subsequent reporting adds overflow, dropped-sample
and sequence/checksum counters; emulator execution is unchanged.

A longer debugger-free Chrome control explicitly selects the first Listen topic
and continues 120 clicks. Local snapshots confirm the flashback CG and the return
to the classroom, rather than inferring scene progress from repeated menu clicks.
It observes 138.628 wall seconds at 1.000097 guest seconds per wall second, 25.146
presented fps, zero missing source samples and zero device underruns. The queue
stays at 1,920–2,401 samples; overflow, dropped-sample, sequence and checksum
counters remain zero. It ends with 1,069 HDD reads and 10,404 FM key edges.

Its sampled image/read timeline is:

| Wall s | HDD reads | Observation |
| --- | ---: | --- |
| 76.535 | 1,030 | Reads increase from 973 for the flashback |
| 77.037 | 1,030 | All nine CG patches change |
| 81.787 | 1,055 | Reads resume for the return |
| 82.285 | 1,066 | All nine CG patches change |
| 82.535 | 1,069 | Further reads and six changed patches |

Sampling is every 250 ms; these are image/read observations, not precise
input-to-completed-load latencies. The local diagnostic additionally captures
snapshots and has harness SHA-256
`1e0ebb42585a9fa5f3ea6fd37226ec63c686ca7e8ea4065f828514c1b9d89e9c`.
The final reproduction tool includes the same topic-selection input path without
capturing pixels. It still does not establish synthesis fidelity or reproduce the
user's original Chrome complaint.

Final-tool controls include the topic selection and 48 further clicks:

| Browser | Wall s | Guest s / wall s | Missing source samples | Presented fps | Device underrun s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Chrome | 95.020 | 1.000113 | 0 | 25.153 | 0 |
| Stock Firefox | 95.038 | 1.000031 | 0 | 26.095 | Unsupported |

Both end with 1,069 HDD reads; their queues stay at 1,920–2,401 samples, and all
four overflow/drop/sequence/checksum counters remain zero. FM key edges are
5,564/5,548 respectively; these are time-paced controls, not identical fixed-work
runs. Both use the final harness SHA-256
`f5be777b3f28e72dd2fa6d9aeb5ab3573a65897c8eddc5e4106e54340acd849a`.
Chrome again changes all nine CG patches near 77.037 and 82.287 wall seconds.

The first new-tool pilot was invalid: Firefox opened a welcome tab,
`document.hidden` became true and the application correctly paused. The tool now prevents that onboarding condition
and reports loss of normal playback promptly.

One earlier stock-Firefox game window missed 15,946 samples around 53–57 wall
seconds; subsequent windows have not reproduced that short gap. A profiled
stock attempt produced no shutdown profile and is excluded from timing claims.
The normal-browser controls therefore supersede the large sustained deficit
previously attributed to Firefox, while preserving the isolated gap as an
unresolved observation.

An initial black canvas after pointer input has not reproduced in the follow-up
controls. Direct and presented pixel counts agreed in a repeated immediate-click
test. It remains an unconfirmed observation, not the basis for an emulator change.

No additional emulator optimization is accepted from these observations. The
reported Chrome failure still needs matching reproduction conditions. Image
fingerprint changes record visible responses, not completed scene loads; dialogue,
menus and idle clicks can leave the CG patches unchanged. Issue #146 remains open.


## Validation of this tooling change

Four JavaScript syntax checks pass. All three old tools reject default Firefox
measurement before starting a server/browser. Five new-control argument checks
reject an invalid click count, checkout-local media, checkout-local output,
existing output, and the known patched Firefox executable. The normal desktop
controls above exercise the import, launch, input, sampling and report path.
The initial welcome-tab pilot exercises failure retention and is excluded from
healthy-playback results.

Relative link destinations, whitespace and the publication audit pass; the site
still contains only the 27 approved assets and the unchanged 539,950-byte release
Wasm. Native/runtime regression suites and a new Wasm build are not repeated for
these opt-in tooling/documentation changes. No game captures, recordings, raw
reports, profiles, browser copies, installed HDD or proprietary media enter the
commit or publication tree. Required post-merge verification: none.
