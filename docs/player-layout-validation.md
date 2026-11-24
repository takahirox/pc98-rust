# Browser player redesign

[Issue #233](https://github.com/takahirox/pc98-rust/issues/233) uses the
[snes-rust player](https://takahirox.github.io/snes-rust/) as the reference for
light controls, a dark screen surround, compact playback and a contextual
media picker. PC-98 media occupies three independently addressable slots.
The desktop display uses the available viewport height, with a minimum size
for short windows. Portrait puts playback directly below the display. Short
landscape windows use a height-limited display beside the drive panel.

The stage keeps its dimensions when Help opens, filenames change, or the guest
reports errors. The 4:3 CRT image and 8:5 Original pixels image fit inside it.
Sharp CRT is the initial preference. Native popovers supply Escape, light
dismissal and focus return. The picker holds its destination through the local
file chooser. The same keyboard panel moves into fullscreen when needed. Short windows have
44 px scroll buttons beside the keys so swiping across guest keys is unnecessary.
Settings shows the active boot image and the source of the next Reset.
Ap mouse input is available there while Ap is selected.

## Release-Wasm screenshots

All captures use the checksum-pinned bundled FreeDOS(98) desktop after boot,
with no third-party game image or user media. The screenshots show the previous
player at the upstream checkout and the updated player on the same release
Wasm build.

| Viewport | Previous | Updated |
| --- | --- | --- |
| Desktop 1280 × 720 | [Before](player-screenshots/before-desktop.png) | [After](player-screenshots/after-desktop.png) |
| Desktop 1440 × 900 | [Before](player-screenshots/before-wide.png) | [After](player-screenshots/after-wide.png) |
| Phone portrait 390 × 844 | [Before](player-screenshots/before-portrait.png) | [After](player-screenshots/after-portrait.png) |
| Phone landscape 844 × 390 | [Before](player-screenshots/before-landscape.png) | [After](player-screenshots/after-landscape.png) |
| Narrow portrait 320 × 700 | [Before](player-screenshots/before-narrow.png) | [After](player-screenshots/after-narrow.png) |

## Automated coverage

The browser suite uses the release-Wasm app at `/` and `/pc98-rust/`.
[Player layout tests](../tests/browser/player-layout.spec.js) exercise screen
ratios, no horizontal overflow, 44 px touch controls, stable stage geometry,
keyboard-only picker navigation and focus, chooser cancellation, boot source
and fullscreen touch keys. Existing
[media routing](../tests/browser/media-routing.spec.js),
[FreeDOS desktop](../tests/browser/desktop.spec.js),
[large disk](../tests/browser/large-hdi.spec.js),
[presentation](../tests/browser/presentation.spec.js) and
[serial audio](../tests/browser/audio-continuity.spec.js) tests reach the new
controls through the real picker and settings flows. They continue to assert
actual guest output, runtime state, exported bytes and performance budgets.

The test launcher emulates touch and viewport dimensions. Physical phone
hardware, platform screen readers and OS-level chooser dialogs are outside
that automated coverage. The cancellation check observes the browser’s native
input `cancel` event from the intercepted chooser.

## Check results

- Native all-features tests passed with the CI test profile and checksum-pinned
  guest fixtures. Rustfmt, Clippy, 49 direct-block checks and 61 JavaScript checks
  passed. The release-Wasm audit validated all 32 approved publication assets.
- Serial ADV98 playback passed at both serving roots, with the default JIT and
  48 kHz stereo at `/` and the interpreter with 44.1 kHz stereo at
  `/pc98-rust/`. The four display modes presented 59.80–60.08 frames per second,
  with measured guest time at 0.997–1.002× wall time. The measurement windows
  recorded zero audio underflows and drops.
- The updated 1280 × 720 and 1440 × 900 layouts place all playback and secondary
  buttons within the first viewport. The 844 × 390 touch viewport also fits the
  media panel and both control rows. The 390 px and 320 px portrait layouts
  place playback in two rows immediately below the screen.
- The functional run recorded 305 passes and two suite-defined skips. A
  local startup readiness timeout and two assertions still looking for the
  reload message in the status line were rerun with the final code. All eight
  targeted checks passed across both roots, including the current alert,
  trusted chooser cancellation and fullscreen keyboard scrolling.
- All five large-media cases passed with the suite's three expected skips.
  Coverage included 512 MiB import, repeat export and reimport at both roots,
  occupied 512 MiB export, high-file FAT16 writes and 1 GiB guest I/O.
- The pinned Chromium download stalled on this host. The browser runs used
  installed Chrome 158 with Playwright's Chrome channel and the repository's
  serving roots, assertions, timeouts and performance thresholds. The
  large-media run used a disposable RAM-backed directory for test files.
