# CPU, device and scanout optimization evaluation — Issue #169

## Method and scope

Baseline is main `9a1af782105ae45fe5ec490cd920b7870b5348bd`. Comparisons used stock
headless Chrome 157.0.8103.39 on arm64 macOS 27.2, without a debugger or CPU
profiler. Two independent machines ran in one dedicated Worker, alternating
10 ms guest slices and reversing their order on each slice. Both received the
same installed-game boot, command and mouse inputs. Compilation waits are outside
the active-time measurement. The measured scene covers 24.0002795 guest seconds.
Private media, opcode diagnostics and raw reports remain outside the checkout.

The CPU measurement excludes rasterization and AudioWorklet scheduling; it is
not a visible-fps or transition-latency measurement. The separate renderer
comparison uses original synthetic text and planar graphics, including text and
cursor blink, and compares every output pixel. Each row measures 144 frames
after warm-up, including input/output copies performed by the harness.

## Retained changes

| Technique | Observed result and boundary |
| --- | --- |
| Event-deadline device updates | Device-only trial: 6,417.5 → 5,681.5 active ms, 11.5% lower. Materialize at PCM/timer/IRQ/FDC deadlines, I/O, firmware and slice exit. Preserve per-instruction keyboard/cascade polling and the observer reference path. |
| Register locals and flag liveness | Same-core original INC loop, three 6-million-instruction controls: 121.3/117.8/120.4 → 110.6/113.2/111.8 ms; median 7.1% lower. Installed-game-only trials were neutral (0.2–0.4%); no separate game-wide gain is attributed to this technique. All early exits reconstruct complete flags/registers. |
| Guarded RAM word access | Preflight a word wholly inside conventional RAM once; retain both byte timing events, journaling and the full mapped/boundary path. Incremental game trial: 5,579.6 → 5,538.6 ms (0.7%); small enough to treat cautiously. |
| Hot unsupported operations | Add immediate shifts/rotations except carry rotations, memory single-bit shifts/rotations and FE/FF INC/DEC. Translated instructions rise from 92,346,762 to 93,107,752 of 118,102,981. Incremental active time was neutral; this is a coverage improvement, not a separately demonstrated game speedup. |
| Exact scanout reuse and buffer ownership | Blinking but otherwise unchanged screen: 151.3 → 92.4 ms, 38.9% lower. A VRAM change every frame: 150.9 → 151.3 ms, effectively neutral. Cache keys include all snapshot bytes except the cycle stamp, replaced by effective blink phases. Recycle input allocations and render directly into owned pixels. |

The combined final-artifact game comparison measured 6,416.4 → 5,602.8 active ms,
12.7% lower. It preserves all 118,102,981 instructions, 1,584,018,440 CPU clocks,
131 disk reads, 1,464 FM key edges, 934 sound IRQ edges and 1,152,013 PCM samples.
Complete PCM-bit, framebuffer and CPU-state fingerprints match. The reversed-order
holdout measured 6,462.9 → 5,695.4 ms (11.9% lower), with the same fingerprints.

The frozen baseline Wasm SHA-256 is
`1b28c5973d0213e45b8f01b228246d8581155bb9ea1202afb08703843705136d`.
The frozen final Wasm SHA-256 is
`6d9d2c74eb9bb56278dd041c224a3535ce016fda3bd61077d16c5838531aea57`;
its JIT source SHA-256 is
`351075c346708ce72d5776cf3b2a4cff055c08c954baee5d31bcaf5ea084b836`.

## Rejected or corrected experiments

| Experiment | Evidence and decision |
| --- | --- |
| Cached Rust memory-operation runs | Exact game/PCM results, but 5,903.9 → 6,020.8 ms (2.0% slower). Removed, including temporary exports/counters. Generated memory operations retain their existing bus callbacks. |
| Page-generation guards inside register runs | Exact game/PCM results, but 5,641.5 → 5,985.5 ms (6.1% slower). Added write/version bookkeeping exceeded the saved byte checks. Removed; code-byte validation remains at every original instruction. |
| Ready-entry reuse for tight loops | 5,592.6 → 5,584.8 ms (0.1%), within noise. Removed. Existing bounded Wasm dispatcher/function-table chaining remains; cross-block traces and direct successor links remain future work requiring stronger evidence. |
| First device scheduler | Faster but divergent CPU/PCM. A barrier replayed a keyboard IRQ between INTA and the final scan read. Fixed before selection; private native instruction-by-instruction comparison then passed the complete opening/command sequence. |
| Initial frame cache | Copying snapshots/pixels into a separate cache cost 4–7% on changing frames. Reworked to own pixels and swap input allocations; final changing-frame cost is neutral. |

## Correctness and remaining limits

New tests compare deferred and original ticks through PIT/OPN/mouse/keyboard
IRQs, zero-clock boundaries, DMA, I/O and every PCM sample. PIT deadlines are
compared with one-clock execution across modes, divisors and reloads. RAM-word
checks cover alignment, cache/page/device/A20 boundaries and fault rollback.
Actual release-Wasm differential tests cover masked shift counts, aliases,
undefined-flag policy, segment overrides, video write engines and every work-cap
prefix through mixed live-carry/overflow consumers. Scanout tests cover arbitrary
output buffers, blink changes, input reuse and invalid-snapshot recovery.

Local focused validation passes native library/video and device regressions,
Clippy, release-Wasm publication checks and Node tests. Complete native and
browser acceptance, plus the required post-merge main audio scenarios, are
reported in Issue #169 and its PR. Audio queue sizes, guest timing and continuity
thresholds are unchanged.

The full interpreter checkpoint remains necessary for restartable faults; this
revision reduces its use through the measured unsupported-opcode additions
instead of weakening rollback. Indirect/far transfers, segment-stack operations,
CL-count shifts, strings, I/O/firmware and protected/paged execution remain
potential targets. General memory-containing generated runs still need an
approach that beats individual callbacks without speculative bus writes.
Dirty-region transport and GPU rendering are not adopted: this round establishes
cheap identical-frame reuse, while the remaining measured CPU cost still exceeds
scanout cost. These measurements do not establish 60 fps on the user's slower PC
or close the original playback issue #146.
