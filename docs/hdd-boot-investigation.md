# FreeDOS(98) HDD investigation — Issues #111 and #113

## Current restart — 2026-10-30: blocked/incomplete

The current [Issue #113 objective](https://github.com/takahirox/pc98-rust/issues/113)
requires addressing the actual HDI boot/input failure. Merged
[PR #114](https://github.com/takahirox/pc98-rust/pull/114) supplies existing
independent controls only. Neither those passing controls nor this access audit
completes the restart. **The reported hang remains unresolved; keep #113 open.**

The assigned checkpoint is `3636778cb3f18a7e29233a3b2b3ff3116c5a6969`.
Read-only `git ls-remote https://github.com/takahirox/pc98-rust.git refs/heads/main`
confirmed current main at `04cb05978486f7413bc79591487d9305f80fa140`;
`git diff 04cb059 HEAD --stat` was empty before this restart. PR #114's merge
`83f120d38681d22fe8f87fb64ba31b39bea75e19` is already in this history.
No upstream technical diagnosis was supplied.

### Access audit and exact missing fact

The live Issue and its comments were read, including the
[existing owner authorization](https://github.com/takahirox/pc98-rust/issues/113#issuecomment-6513464041).
Owner permission for eligible private local testing is established and was not
requested again. The earlier Run's actual recorded reason for stopping was
unestablished age eligibility and environment permission, as recorded in its
[access decision](#source-verification-and-access-decision-2026-10-30).
It did not attempt an archive request. That record demonstrates neither a
download failure nor an applicable environment prohibition.

This restart reread the live [listing](https://alicefree.fastlast.org/) and
inspected its HTML links without requesting archive media. The PC-98 entry still
targets <https://d1.alicefree.fastlast.org/d/alice_yakata3.zip>; the separate CD
entry targets <https://d1.alicefree.fastlast.org/d/zip/alice_yakata3_CD.zip>.
The listing asks under-18s not to use its files. The current
[official declaration](https://www.alicesoft.com/about/#cont08) still lists the
game, retains copyright, and prohibits supplying AliceSoft-brand software to
under-18s. It identifies its latest revision as 2019-01-21. The other conditional
distribution requirements remain applicable; the declaration does not establish
licenses for unidentified bundled OS, driver, utility or font components.

No explicit ban on this private compatibility investigation was found in the
supplied execution instructions or repository guidance. Local shell/filesystem
access and outbound requests to the public listing and GitHub worked. This is
not evidence of archive availability: no archive request, including HEAD or
range retrieval, was attempted. Unknown bundled licenses are an inspection task,
not an identified licensing violation or a proven reason that the earlier Run
could not download.

**The unresolved fact is whether the human operator on whose behalf retrieval
would run is at least 18.** Neither the live owner comment nor the execution
inputs establish that fact. An age-eligibility question was submitted in this
Run; no affirmative answer is recorded at this checkpoint. The Issue explicitly
requires eligibility before retrieval and forbids fabricating it. An affirmative
operator eligibility statement would resolve this particular constraint without
repeating owner permission. It would not establish bundled-component rights;
those must then be inspected separately before guest execution. This records a
missing eligibility fact, not a claim that the operator is under 18 or that the
environment prohibits testing.

No ZIP/HDI was retrieved, extracted or tested in this restart. Archive integrity,
HDI identity/geometry, actual kernel/drivers, symptom reproduction, first
architectural divergence, correction and affected-image retest remain pending.
No emulator change or new regression is justified by the present evidence.
Once eligibility is established, resume with private extraction/license inventory
outside the checkout, an intact original and separate working HDI, then bounded
Ap/IDE diagnostics and keyboard retesting against the existing controls. Keep
all media, hashes and raw output local; headless output must be redirected to
private files, not returned as tool output or sent to model services.

### Checks in this restart

`cargo build --locked --release --bin headless` passed, preparing the existing
runner without private media. Documentation links, Markdown structure and
`git diff --check` were checked. No emulator, fixture, build or dependency source
changed. Native test suites, fixture downloads, production-Wasm/browser acceptance
and private-media tests were not rerun; historical results below are not new
validation. This documentation checkpoint is a blocked handoff, not completion
or a compatibility fix. No GitHub mutation is part of this Run.

## PR #114 baseline: independent alternatives, private image not tested

The remaining sections preserve historical #113/#111 evidence and validation.
Their statements about completed bounded investigation and review readiness
describe PR #114's earlier scope, superseded by the current restart objective;
they do not discharge the pending work above or resolve the reported hang.

### Source verification and access decision (2026-10-30)

The live [distribution listing](https://alicefree.fastlast.org/) was read before
any media retrieval. Its HTML links were inspected without following the archive
links:

| Listing entry | Exact ZIP target |
| --- | --- |
| アリスの館3(PC-98版) | <https://d1.alicefree.fastlast.org/d/alice_yakata3.zip> |
| ALICEの館3(CDディスクイメージ) | <https://d1.alicefree.fastlast.org/d/zip/alice_yakata3_CD.zip> |

The PC-98 link matches the user's supplied URL. These are separate archives;
the CD image is not a substitute. The listing asks under-18s not to use its files
and directs users to the official declaration. AliceSoft is the rights holder;
this download listing is a separate distributor, not AliceSoft's own download
service.

The current [official declaration](https://www.alicesoft.com/about/#cont08)
lists アリスの館3 and identifies its latest revision as 2019-01-21 (seventh
edition). Copyright is retained. Its conditions include no provision of
AliceSoft-brand software to under-18s, inclusion of the declaration on
redistribution, no distribution for monetary profit, compliance with current
terms, and no separate distribution of extracted CG/music. Specified OS/emulator
format conversions are permitted under the conditions; this is not a blanket
license for every component of a compiled HDD.

This execution has no independently established age eligibility or environment
permission for receiving the age-restricted media. It therefore **did not request
or download either ZIP, inspect a ZIP directory, extract an HDI, or run the
private image**. This is an access decision, not a server error or proof that the
archive is unavailable to an eligible operator. No age assertion or gate bypass
was attempted. Eligibility checks and private access remain optional, as the
Issue specifies; no human playtest or private media is required for CI.

The user's extraction assertion remains unverified: ZIP integrity, exact member
path, `aliceY3.hdi` identity, size, geometry, partition/BPB/FAT, FreeDOS version,
drivers, utilities and fonts are unknown. The listing alone establishes none of
these. Game distribution permission does not establish the rights for bundled
OS/driver/font/utility components. Those need a separate inventory and license
review before private use in an eligible environment. No private hashes, guest
bytes, traces, screenshots or media are included in this checkpoint.

### Independent control at the selected baseline

The assigned upstream checkpoint is
`70a9b585c02a22459a2bb70f0e38e4bae12cb09c`. Read-only `git ls-remote` confirmed
live `main` at `5f5c4a2f610d528bed1f954ab9b45f1a5245e625` (PR #112);
`git diff 5f5c4a2 HEAD --stat` was empty before this work. No upstream technical
diagnosis was provided. Tests use **PC-9821Ap DOS**, mounting the legal
`fixtures/freedos/shell.hdi` as IDE drive 2, not as floppy.

The existing [preparation script](../tools/prepare_freedos_hdd_fixture.py),
[manifest](../fixtures/freedos-hdd-manifest.json) and
[license audit](licenses.md#freedos98-downloaded-test-guest-gplv2-or-later)
define the control. Its pinned 2022 FreeDOS kernel/boot and FreeCOM inputs retain
matching complete GPL sources and notices. Original MIT IPL/PROBE/configuration
files supply the rest; private media and third-party memory managers are absent.
The control's geometry, FAT, handoff and raw-VRAM invariants are recorded in
[the original observations](#independent-hdd-control-and-observed-handoff).

The focused native cold/reset rerun observed 1,973 raw NUL cells at handoff on
both boots, with entry observed in scheduler slot 4,063. Both boots reached the
shell and executed PROBE/ECHO successfully. A separate unchanged-interpreter run
with `target/debug/headless fixtures/freedos/shell.hdi 8000000 --progress --ap`
reported 316 IDE reads, zero writes, zero instruction faults, 800 acknowledged
IRQs, and 565 unique entries in the final 4,096-attempt window. Its final viewport
had zero literal `3F` cells and one ANK fallback cell. These reproduce the legal
control's previous observations, without reproducing the private-image hang.

Validation in the initial provenance pass (macOS arm64; revision results below):

| Check | Result |
| --- | --- |
| All 45 README fixture preparation commands | Passed, including pinned HDD hash, complete GPL sources/notices and unchanged ADV98 HDD hash |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | 7 passed |
| `cargo test --locked` | 520 passed, zero failed/ignored, including HDD cold/reset and original headless loop/fault/IDE/VRAM controls |
| Focused native HDD rerun with `--nocapture` | Passed both boots, exact kernel loading and actual PROBE/ECHO execution; handoff observations above |
| Eight-million-slot `headless --progress --ap` control | Passed to budget, no host error; aggregate observations above; command responsiveness established by acceptance tests, not budget exhaustion |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` and `npx playwright install chromium` | Passed |
| `npm run build` | Passed: reproducible original media/font, locked production Rust/Wasm build, exact ten approved assets; Wasm 520,085 bytes |
| `npx playwright test --config target/issue-113-playwright.config.mjs freedos-hdd.spec.js` | 2 passed (30.4 seconds), real production Wasm, cold/reset at both `/` and `/pc98-rust/`; handoff after 4,062 slots followed by responsive PROBE/ECHO shell |
| Added relative links/heading anchors and `git diff --check` | Passed |
| Private ZIP/HDI, reference emulator, game launch/input | Not obtained or tested |

Port 8000 was occupied. The ignored local Playwright configuration imports the
repository configuration and substitutes ports 60711/60712, absolute test/output
paths and two workers. Its second server exposes `web/` through a temporary
`pc98-rust` directory symlink, preserving the project path. The existing listener
was not reused or terminated. Only the two focused HDD browser cases were rerun;
the full 154-case browser result below belongs to #111, not this continuation.
That initial pass changed no emulator, guest, test, build or dependency source;
the independent experiments added during review are recorded below.

A broader documentation-anchor scan found an existing README #105 audit link
missing the `historical-` prefix now present in its target heading. Both the link
and heading predate this checkpoint; this unrelated mismatch was left unchanged.
All links added here were checked separately.

### Review revision: bounded independent alternatives (2026-10-30)

The review checkpoint is `aa5e8b397bda3889cb582057e3b8c65279639346`;
its emulator sources still match PR #112. The following experiments complete
additional independently reproducible investigation without private media.
**They are negative compatibility results and intentional guest controls, not
reproduction or correction of the reported HDI hang.** No CPU/device execution
code changed; no demonstrated emulator defect justified such a change.

[Preparation](../tools/prepare_hdd_investigation.py) reuses the checksum-verified
2022 FreeDOS kernel, FreeCOM and matching complete GPL sources/notices. It
assembles the unmodified GPL HDD boot source for FAT12/FAT16. The original MIT
IPL, PROBE, [POSTWAIT](../guest/postwait.asm), configurations and FAT allocation
are the only other inputs. [The manifest](../fixtures/hdd-investigation-manifest.json)
pins all files/images. A separate reader walks every generated FAT chain,
rejects cycles/aliasing/out-of-range clusters, compares contents with inputs and
checks both FAT copies. The established shell and ADV98 HDD hashes stay unchanged.
No extra driver, memory manager or game content is required. See
[redistribution requirements](licenses.md#independent-hdd-shell-control-issue-111).

| Hypothesis under test | Independent experiment and result |
| --- | --- |
| Contiguous FAT12 loading hides a FAT-chain or disk-addressing problem | `fragmented12.hdi`: same 64×8×16 geometry, cylinder-1 partition and FAT12 BPB; allocate file chains with coprime stride 37. Kernel and shell fragments cross heads/cylinders and FAT sectors. All 49,938 kernel bytes match before entry; actual PROBE/ECHO commands pass cold/reset. This layout does not produce a post-handoff hang. |
| Different FAT width, partition origin or cylinder high byte breaks loading/boot | `fragmented16.hdi`: 512×2×24 geometry, cylinder-2 partition (LBA 96), 24,480 volume sectors, 2 sectors/cluster, two 48-sector FAT16 copies, 512 root entries; data offset 130. Coprime stride 257 places kernel fragments beyond cylinder 255. Native bounded BIOS history asserts actual reads there. Exact kernel and commands pass cold/reset. This rules out a general failure of these tested layouts, not all possible HDI geometries. |
| A persistent NUL fallback display itself prevents input/forward progress | `wait.hdi`: real DOS invokes original POSTWAIT.COM before the AUTOEXEC greeting. It writes 2,048 NUL words and visible attributes, writes only `POSTWAIT READY`, and polls IRQ-fed BIOS input. After another 100,000 slots, 1,986 visible cells remain NUL, literal `3F` count is zero, IRQ0 ticks and IRQ acknowledgements advance. Actual Enter is consumed; DOS returns and executes PROBE/ECHO while many fallback cells remain. Persistent fallback is compatible with a responsive guest. |
| No host error and no Enter response necessarily imply a CPU/disk failure | `maskwait.hdi`: identical probe with argument M; guest OUT masks only IRQ1 for 64 IRQ0 ticks. At 150,000 slots into the wait, Enter is still in the keyboard FIFO, PIC IRR1=1, ISR1=0, BIOS key count=0, and 1,986 NUL cells remain. CPU polling/IRQ0 continue with zero faults. The guest restores the saved mask itself after 64 ticks, consumes the **same pending Enter**, returns to DOS and executes PROBE/ECHO. This intentionally constructed negative control distinguishes deferred keyboard delivery from halted execution; it is not evidence that the private image masks IRQ1. |

The [native experiments](../tests/hdd_investigation.rs) use the existing
read-only bounded progress reporter; [production-Wasm experiments](../tests/browser/hdd-investigation.spec.js)
run the same disk guests and verify their outputs.
Native sampling covers handoff, wait/input transitions, shell and returned commands, with
at most a 4,096-attempt address window and 16 BIOS disk requests retained. There
is no growing trace or guest disassembly. Hosts mount/step, read and supply normal
scancodes only; the guest owns PIC writes, timer observations and all result
bytes. POSTWAIT state resides in its executing COM segment, not a host-patched
kernel or BIOS work area. Both native and production Wasm repeat cold/reset and
require fault-free execution, real DOS PROBE/ECHO output and an unchanged disk,
within **8,000,000 total slots per boot including input**.

Reproduce from the normal README fixture prerequisites:

```sh
python3 tools/prepare_freedos.py
python3 tools/prepare_freedos_hdd_fixture.py
python3 tools/prepare_hdd_investigation.py
cargo test --locked --test hdd_investigation -- --nocapture --test-threads=1
cargo run --locked --bin headless -- fixtures/freedos/fragmented16.hdi 8000000 --progress --ap
npm run build
npx playwright test hdd-investigation.spec.js freedos-hdd.spec.js
```

The native test also runs the unchanged `shell.hdi` with the same sampler and
input schedule, making its baseline directly comparable. Deterministic results
below repeated on cold/reset; slots count **before** the kernel's first instruction:

| Control | Handoff slots / IDE reads | Shell observed slots / IDE reads | PROBE/ECHO returned slots / IRQ acknowledgements |
| --- | --- | --- | --- |
| Existing contiguous FAT12 shell | 4,062 / 122 | 4,297,062 / 316 | 5,196,062 / 548 |
| Fragmented FAT12 | 4,062 / 122 | 4,346,062 / 322 | 5,244,062 / 553 |
| Fragmented FAT16 | 6,659 / 182 | 4,805,659 / 795 | 5,703,659 / 599 |
| POSTWAIT, keyboard enabled | 4,062 / 122 | 4,526,959 / 320 | 5,430,959 / 574 |
| POSTWAIT, keyboard temporarily masked | 4,062 / 122 | 5,059,655 / 320 | 5,963,655 / 627 |

At **all** handoffs: CS:IP=0060:0000, exact kernel bytes, boot drive 80h,
partition index 0, PE/VM/PG clear, A20 masked, 1,973 NUL cells, no literal `3F`,
PIC master/slave mask=7C/F7, IRR/ISR clear, PIT reload=19,968 and zero instruction
faults. The FAT16 boot source legitimately relocates its stack to 3800:2000;
FAT12 uses 1E80:0100. An initial test incorrectly assumed the FAT12 stack for
both; checking the pinned GPL source corrected the test, not the emulator.
FAT16's extra reads and slots reflect its larger root/FAT and relocation path.

At returned commands, all controls remain real mode, PIC masks return to 7C/F7,
IRR/ISR are clear and PIT reload remains 19,968; IDE is idle (status 50h,
error 00h, no pending sectors/INTRQ), no disk writes or floppy reads occur.
The ordinary and fragmented shell controls have one ANK fallback cell and zero
literal `3F` cells. The POSTWAIT controls still have 1,690 fallback cells
(including 1,689 raw NUL words) while executing commands successfully. This
separates renderer classification from command responsiveness without replacing
glyphs or fabricating text/guest control flow.

The masked checkpoint is particularly diagnostic: mode=real, IF set, not HLT or
shutdown, zero faults; master PIC **7E/02/00** versus the working shell's
**7C/00/00** (mask/IRR/ISR), slave F7/00/00. From POSTWAIT readiness to the
pending-Enter sample, IRQ acknowledgements grow 433→448, BIOS IRQ0 entries
433→448, BIOS keyboard IRQ1 entries stay absent, IDE reads remain 319 and BIOS
INT 1Bh count remains 317. The final 4,096 attempts have 12 unique addresses
(4,084 repeats), versus 2,025 unique addresses at the ordinary shell sample.
The difference is intentional guest polling and a guest IRQ mask, with a clear
recovery path. Repetition and a static display alone still do not classify a hang.

### Revision validation and remaining limits

Validation results for this review revision are recorded after the experiments:

| Check | Result |
| --- | --- |
| All 46 README fixture preparations | Passed; pinned input/source/license checks, all four new image/FAT audits and unchanged existing shell/ADV98 hashes |
| `cargo test --locked` | 523 passed, zero failed/ignored (includes two new experiments and the reused reporter unit check) |
| Final `cargo test --locked --test hdd_investigation -- --nocapture --test-threads=1` | 3 passed; all five controls cold/reset, at most 5,963,655 slots including commands |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | Passed |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | 7 passed |
| Eight-million-slot FAT16 `headless --progress --ap` | Passed to budget; aggregate observations below |
| Production Rust/Wasm build and approved-assets audit | Passed, unchanged 520,085-byte Wasm, exactly ten approved assets |
| `npm ci`, `npx playwright install chromium` | Passed |
| `npx playwright test --config target/issue-113-playwright.config.mjs` | 162 passed (7.0 minutes), complete real-Wasm suite at `/` and `/pc98-rust/`, including all four new cold/reset controls |
| Added relative links/anchors, manifest source/image hashes, `git diff --check`, tracked-media audit | Passed; all HDIs ignored, no tracked HDI/ZIP, no private media acquired |
| Private ZIP/HDI or gameplay | Not obtained or tested |

All four new browser controls reproduce the native cold/reset instruction, IDE
read and IRQ acknowledgement totals in the table above after their commands
return. Both serving paths use the real production Rust/Wasm artifact.

An additional eight-million-slot `fragmented16.hdi --progress --ap` run reached
the shell: zero faults, 800 IRQ acknowledgements, 795 IDE reads, no writes,
real mode and one fallback cell. Its last 4,096 attempts contain 565 unique
entries, matching the working shell control's window count. Budget exhaustion
alone is not the success criterion; the input experiments establish actual
command execution.

Initial browser development checks caught an ambiguous drive-option selector
and a prompt assertion that ran before FreeCOM finished printing. The tests
now explicitly select IDE by value and wait within the instruction budget for
both the ECHO output line and returned prompt; no execution code changed.
The initial full browser run was stopped to apply that test correction; the
final complete run is the result recorded above. As in the earlier pass, the
local configuration changes only ports/serving directories/paths and worker
count because port 8000 is occupied.

The new controls narrow the investigation: tested FAT widths/fragmentation and
high-cylinder disk reads are functional, and persistent NUL fallback does not
by itself prevent keyboard/guest progress. No observed emulator defect requires
a fail-before/pass-after correction; these tests are **pass-before/pass-after
experiments on the unchanged interpreter**, with an intentional guest mask as
the negative control. Protected/VM86/paging transitions and external drivers are
not exercised by these real-mode alternatives; their existing separate tests
cannot establish what the private image uses.

The exact private HDI's geometry, kernel/version, driver initialization and first
divergence remain unknown. Its hang, shell reachability and game interaction are
**not independently reproduced or retested**. Optional legitimate private access
can compare its first differing bounded checkpoint with these controls; it is
not required for CI or review. These unknowns limit the compatibility conclusions;
they are not unfinished mandatory work when private media is unavailable. No
independent large missing feature was found that would justify a speculative
task. **Leave #113 open** through a non-closing reference in PR #114 and make no
game-playability claim. No human confirmation or post-merge check is required.

### Mandatory acceptance disposition

This accounts for all seven pre-merge criteria in #113, in their original order.
The completed work includes the four new independent alternatives above, beyond
the shell baseline and telemetry already delivered by #112. It is sufficient to
review and publish this bounded investigation without claiming that the private
HDI now boots.

| Issue #113 criterion | Disposition and evidence |
| --- | --- |
| 1. Exact archive provenance, current terms and access decision | Complete. [Source verification and access decision](#source-verification-and-access-decision-2026-10-30) records the exact PC-98 and separate CD-image URLs, official conditional terms, absent established eligibility/environment permission, and the decision not to retrieve media. Bundled component rights remain unverified, separately from game rights. Private inspection is optional. |
| 2. Bounded progress investigation and independent checks | Complete. [Independent alternatives](#review-revision-bounded-independent-alternatives-2026-10-30) reuse the existing reporter to compare instruction repetition/faults, PIC/PIT/BIOS/IDE and raw NUL/literal `3F`/fallback counts. POSTWAIT provides persistent-display and deferred-input negative controls without host errors or private traces. |
| 3. Legal HDD baseline, handoff, kernel bytes and command execution cold/reset | Complete. The unchanged `shell.hdi` and all four alternatives assert exact kernel bytes before entry, the handoff and real PROBE/ECHO execution with returned FreeCOM prompts. Native and production-Wasm cold/reset runs at both serving paths pass; deterministic results are tabulated above. Baseline success is not called a bug fix. |
| 4. Minimal fail-before/pass-after regression for every corrected emulator defect | Conditional; no correction warranted by the observations. All controls pass on the unchanged interpreter; no emulator defect was diagnosed or corrected. The intentional guest IRQ mask has a measured recovery path and is not a defect. Inventing a failure or speculative execution workaround would not satisfy this criterion. A demonstrated future defect will require its own independent regression. |
| 5. Unreproduced behavior and optional private retest | Complete reporting. ZIP/HDI identity, components and first divergence remain unknown; the actual hang, shell and game were not tested. The independent controls establish only the results above. No commercial media, manual playthrough or age confirmation is required to accept them. |
| 6. Fixture/license preparation and native, production-Wasm, asset and browser checks | Complete. [Revision validation](#revision-validation-and-remaining-limits) records all 46 preparations, 523 native tests, Clippy, formatting, production build/approved-assets audit and 162 real-Wasm Chromium tests across `/` and `/pc98-rust/`. The subsequent review independently passed fixture preparation, 523 native tests, Clippy, formatting, production build/asset audit and the ten focused HDD Chromium cases across both paths. No private media is tracked or required. |
| 7. Honest completion and Issue lifecycle | Complete for this bounded investigation: ready for review. PR #114 uses a non-closing reference to #113, so merging it leaves the Issue open for the untested, unresolved private-image hang. Completion of the bounded experiments is not resolution of that compatibility report. There is no evidenced independent large missing feature requiring another task and no administrative closure requiring a continuation at this checkpoint. |

There is no outstanding lawful agent-executable investigation justified by these
observations in this bounded scope. Private retrieval, gameplay and finding a
defect are not additional acceptance gates. The negative results narrow the
tested hypotheses without establishing the private image's cause. This report
and the passing controls therefore support a ready investigation PR while the
reported compatibility problem remains open.

The completion revision changes only this report. Its checks passed for all
17 relative links/heading anchors, all seven criterion rows, the unchanged
original guest source hashes/four-control manifest and `git diff --check`.
Runtime suites were not rerun for this documentation-only revision; the
implementation and subsequent review results above remain the execution evidence.

## Issue #111 checkpoint (historical)

This checkpoint adds reproducible HDD shell acceptance and bounded observations.
**It does not resolve the reported private-image hang.** No underlying CPU, BIOS,
IDE or rendering defect causing that hang was independently established, so no
speculative execution workaround or glyph replacement is included. Issue #111
must remain open; game boot and playability are **not confirmed**.

## Baseline and lawful-access decision

Investigated on 2026-10-30. The selected worktree baseline is
`a5ae2f35fb2d8a455acafade50dd96d1f51831d4`; its emulator sources match main's
PR #110 merge, `a64ad1efdf09501f0f30f040fffd85ea6dc37e7e`. Use the explicit
**PC-9821Ap DOS** profile. No diagnosis was supplied by an upstream node.

Before any game-media retrieval, the public text of
[AliceSoft's declaration](https://www.alicesoft.com/about/#cont08) was examined.
It lists アリスの館3, retains copyright, prohibits providing AliceSoft-brand
software to under-18s, requires the declaration with redistribution, prohibits
profit-making distribution and separate extracted CG/music distribution, and
requires compliance with the current terms. It permits conversions for newer
OS/emulator formats under its conditions. AliceSoft says it does not itself
supply these downloads. This is conditional permission for the listed software,
not evidence clearing an arbitrary compiled HDD and its other components.

The [patton project overview](https://github.com/kichikuou/patton#readme) describes
constructing an HDD from user-supplied game files and a separate FreeDOS base.
Its [asset recipe](https://github.com/kichikuou/patton/blob/main/assets/README.md)
identifies a 12 MB **NHD**, the 2022 FreeDOS distribution, `FDXMS286.SYS`, and a
separately obtained `POWEROFF.COM`. That describes a different compilation; it
neither identifies `aliceY3.hdi` nor clears every possible bundle component.
Only these provenance documents were inspected, not its emulator implementation
or binary assets. Public web and GitHub filename searches returned no matching
provenance for `aliceY3.hdi`.

| Component | Evidence and decision |
| --- | --- |
| Exact `aliceY3.hdi` | No supplied image/hash, verified distributor, inventory, build recipe or applicable access/age conditions. **Not obtained; not retested.** Geometry, FAT, FreeDOS version and equivalence to the user's image remain unknown. No game archive or compiled third-party disk was downloaded. |
| AliceSoft game | Declaration reviewed; this does not identify or authorize the exact HDD's complete contents. No access gate was bypassed or age assertion made. |
| Kernel, HDD boot and shell used here | Existing checksum-pinned GPL FreeDOS(98)/FreeCOM inputs in [the preparation script](../tools/prepare_freedos.py) and [license guide](licenses.md). Matching complete sources and notices are retained beside generated fixtures. |
| IPL, PROBE and builder helpers used here | Existing original MIT sources, pinned by [the new manifest](../fixtures/freedos-hdd-manifest.json). No commercial resources, proprietary ROM/BIOS/font or third-party driver is needed. |
| Fonts/drivers/utilities in the private HDD | Inventory and rights unknown; the game declaration cannot settle them. This control uses the project's existing licensed replacement font and no external memory manager or utility bundle. |

Store any independently cleared private media, original and separate working copy,
outside the checkout and publication paths. HDI files (including uppercase names)
are ignored as an additional guard. The publication audit still permits exactly
the established ten web assets; neither the GPL control HDD nor any private media
is published. No private binary is forwarded to tools or external services.

## Independent HDD control and observed handoff

Prepare with:

```sh
python3 tools/prepare_freedos.py
python3 tools/prepare_freedos_hdd_fixture.py
cargo test --locked --test freedos_hdd --test headless_trace
```

`fixtures/freedos/shell.hdi` has SHA-256
`fb44e34131bfa2ca56ffb11ad004446c096718233391f2828b4d488114b40bcb`.
The original partition IPL selects the DOS partition; the **unmodified GPL HDD
boot source** from the pinned kernel archive is assembled with
`nasm -f bin -DNEC98HDD -DISFAT12`. The shared builder preserves the older ADV98
HDD hash unchanged. Only explicitly selected kernel/shell/license files and
original helper/configuration files are copied; no distribution slack/utilities
are copied. Keep the complete `fixtures/freedos` folder and corresponding sources
with this generated GPL disk if redistributing it.

| Invariant | Independently checked result |
| --- | --- |
| Header and geometry | 4,096-byte header, 4,194,304-byte capacity, 64 cylinders × 8 heads × 16 sectors × 512 bytes |
| Partition/IPL | PC-98 AA55 signatures; active A1/81 DOS partition at cylinder 1 (LBA 128), length 8,064 sectors |
| FAT12 BPB | 512-byte sectors, 4 sectors/cluster, 2 reserved sectors, 2 identical 6-sector FATs, 192 root entries; data starts at relative sector 26 |
| Read path to handoff | 122 IDE reads: 1 disk IPL + 1 partition table + 2 volume IPL + 12 root + 6 FAT + 100 kernel sectors; zero floppy reads/writes |
| Loaded kernel | Every one of the 49,938 kernel file bytes matches at physical 00000600; comparison occurs before executing its entry point |
| CPU handoff | CS:IP 0060:0000, SS:SP 1E80:0100, boot drive 80h, partition scratch word 0; real mode, PE/VM/PG clear, A20 masked |
| Loader progress | Raw text contains `FreeDOS(98) FAT Kernel GO!`; entry is reached after 4,062 executed scheduler slots (native observer sees it before executing slot 4,063) |
| Raw VRAM at handoff | 1,973 visible cells contain word 0000; these take the existing unsupported-ANK fallback path. This can produce a transient question-like background during an otherwise successful boot. |
| Useful guest progress | FreeCOM 0.85a_DBCS, `HDD SHELL READY`, `A:\>`; PC-98 scancodes type `PROBE`, execute actual PROBE.COM via guest DOS, then run `ECHO OK`; verify a separate `ok` output line and returned prompt |
| Cold/reset | Both boots repeat handoff, command execution and keyboard checks; zero instruction faults, actual IRQ acknowledgements, no disk writes and byte-identical disk export |

The same HDD shell test was run against an isolated copy of the **unchanged
baseline Rust sources**, omitting only assertions for the newly added telemetry
fields. It already passes both boots at the same entry point and slot count.
This is a **pass-before/pass-after control**, not evidence of a newly fixed boot
failure. No first bad CPU/disk transition is established on this control.

A separate eight-million-slot summary run reaches the shell with 316 IDE reads,
zero writes, zero CPU instruction faults and 800 acknowledged IRQs. Its rolling
window has 565 unique mode/CS:EIP entries among 4,096 attempts: repetition alone
also occurs at a working input prompt. The command/input tests distinguish that
case from a hang. The transient NUL fallback observation does **not** explain the
user's persistent display and lack of Enter response; that requires observations
from the exact independently cleared image.

## Bounded local observations

For a lawfully held private copy, use a local path outside this repository:

```sh
cargo run --locked --release --bin headless -- /private/local/disk.hdi 20000000 --progress --ap
```

`--progress` emits at most 16 progress samples and one final VRAM summary. It
suppresses the legacy decoded screen and unknown-port listing. It records:

- Attempts/retired count and exact unique/repeated `(CS,EIP,PE/VM/PG)` entries in
  the last **4,096 attempted instructions**. Halt/shutdown slots do not pretend to
  execute instructions. No growing global address set is allocated.
- Current flags/mode, HLT/shutdown, instruction fault count and historical last
  vector/error, PIC masks/IRR/ISR, PIT channel-0 reload/remaining/output and PIC
  IRQ acknowledgement count. Fault counts include identical repeated faults and
  attempted fault delivery; IRQ acknowledgements include failed delivery.
- Counts for the fixed 256 synthetic firmware entry vectors (IRQ thunks included),
  plus the last **16 INT 1Bh requests**: AX, byte count, CX/DX addressing fields,
  ES:BP destination and returned AH when observed. These do not decode code or
  output sector payloads. A missing return field means no return was observed.
- Read-only IDE task-file status/error/count/CHS-or-LBA fields, internal remaining
  sectors/LBA/cursor, read/write counters and IRQ/control/bank. Observing them
  never reads a data port or acknowledges INTRQ.
- The 2,000-cell viewport selected by text GDC start, wrapping at 2,048 cells:
  literal ANK `3F`, unsupported ANK fallback, JIS and invalid JIS range counts;
  at most **16 distinct** raw low/high/attribute/attribute-high tuples with
  occurrence counts, ordered by frequency. These are histogram entries, not a
  screen transcript. Counts describe stored glyph selection, even with text
  disabled or attributes hiding ink; they do not assert that pixels are visible.
  Text enable, both GDC enable/start states, graphics page and PEGC mode are shown.

A zero host-error result at budget end says **budget**, not boot success. No hang
heuristic automatically patches or skips guest execution. Use shorter repeated
budgets to locate a transition, then the existing opt-in `trace` flag for its
bounded instruction metadata if needed. Keep that richer trace private. No full
memory/disk dump or proprietary disassembly is added.

Native original-byte guests prove a single-PC loop, repeated same-vector #UD
retries and HLT; an IDE guest leaves DRQ/INTRQ pending while diagnostics preserve
its cursor and IRQ. It writes literal `003F`, unsupported `0001`, and JIS left/right
text words and checks separate counts. A ring/attribute-plane test verifies GDC
start wrapping, high attribute bytes and unchanged literal/fallback bitmap rows.
The summary classifies the same ANK pattern lookup the renderer uses; it does not
replace unsupported characters with spaces. Existing protected/paging/VM86 and
IRQ guest tests remain required. Wasm metric IDs 17/18 expose CS/EIP and 19/20
expose instruction faults/IRQ acknowledgements for read-only browser acceptance.

## Validation and remaining limits

Final validation in the assigned macOS arm64 worktree:

| Check | Result |
| --- | --- |
| All 45 README fixture preparation commands | Passed, including the new control and the unchanged ADV98 HDD hash; pinned downloads, complete GPL sources and notices retained |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | 7 passed |
| Unchanged-baseline `freedos_hdd` test | Passed, cold/reset kernel handoff and interactive commands; new telemetry assertions omitted because those fields do not exist on the baseline |
| `cargo test --locked` | 520 passed, 0 failed, 0 ignored, including all native CPU/device/DOS regressions |
| Final focused HDD/headless tests | Passed after tightening the total successful boot/input budget and bounded failure output |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` and `git diff --check` | Passed |
| `npm ci` / `npx playwright install chromium` | Passed |
| `npm run build` | Passed: original media/font reproduction, locked release Rust/Wasm build and exact ten-asset publication audit; Wasm 520,085 bytes |
| Full Chromium suite | 154 passed (5.1 minutes), `/` and `/pc98-rust/`, including new HDD cold/reset kernel/shell checks |
| Final focused Chromium HDD test | Passed at both roots after the final budget assertion |
| Local Markdown links, manifest/source hashes and HDI ignore checks | Passed |
| Exact private HDD / game / reference emulator | Not obtained / not retested / not performed |

The standard browser invocation initially could not bind the occupied port 8000.
The full run used `npm run build` followed by
`npx playwright test --config target/playwright.local.config.mjs --workers=2`.
That temporary ignored configuration imports the repository configuration and
changes only local server commands/ports (60711/60712), absolute test/output
paths and worker count; both serving roots and all tests are preserved. Ordinary
`npm test` remains the documented CI/reproduction command with free default
ports. No unrelated listener was terminated or reused.

An initial native run caught two full-CPU snapshot comparisons expecting no
telemetry change after #UD/#GP. Their architectural expectations remain intact;
they now additionally require exactly one instruction fault, including across
IRET and retry. The final full native suite passes. No boot-path behavior was
changed to make the new control pass.

The production-Wasm HDD test at both serving roots checks the manifest hash,
exact kernel bytes before entry, banner/read count, shell keyboard/PROBE/ECHO,
returned prompt, fault/IRQ counters, unchanged disk and cold/reset. Hosts only
mount, step, supply scancodes and inspect results; DOS runs in the Rust guest.
Existing application/save/audio and hardware tests also remain mandatory.

The reported HDI's version/geometry/loading addresses, persistent raw character
words, first divergent instruction, and any driver/extender/device requirement
remain unknown. No substantial new subsystem has been evidenced to justify a
separate speculative task Issue. Retain #111 as the unresolved investigation;
apply these observations to legitimately available media or a new independent
fixture before selecting a compatibility correction. A black-box reference
emulator comparison and private game boot/input/playability tests were **not
performed**. No private-media, human-gameplay or deployed-Pages check is required
for reviewing this limited checkpoint. No post-merge check is introduced.
