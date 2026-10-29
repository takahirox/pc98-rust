# FreeDOS(98) HDD investigation — Issues #111 and #113

## Issue #113 continuation: verified listing, private image not tested

**Partial investigation; the reported hang remains unresolved.**
[Issue #113](https://github.com/takahirox/pc98-rust/issues/113) continues #111,
which was administratively closed after PR #112. This continuation does not
establish a compatibility fix or claim game boot, input or playability. Leave
#113 open. The sections below this continuation preserve the #111 checkpoint's
evidence and validation; their access decisions describe that earlier run.

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

Validation performed in this continuation (macOS arm64):

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
No emulator, guest, test, build or dependency source changed in this continuation.

A broader documentation-anchor scan found an existing README #105 audit link
missing the `historical-` prefix now present in its target heading. Both the link
and heading predate this checkpoint; this unrelated mismatch was left unchanged.
All links added here were checked separately.

### What the controls can establish, and the next observation needed

The existing [native acceptance](../tests/freedos_hdd.rs) and
[production-Wasm acceptance](../tests/browser/freedos-hdd.spec.js) compare all
49,938 loaded kernel bytes before entry at 0060:0000, require the loader banner
and 122 IDE reads, then require a FreeCOM prompt, guest execution of PROBE and
ECHO OK, returned prompts, IRQ acknowledgement, zero instruction faults, and an
unchanged HDD on cold boot and reset. These are baseline compatibility checks;
passing them again is not a fail-before/pass-after correction.

The existing [bounded reporter](#bounded-local-observations) and original
[headless regression guests](../tests/headless_trace.rs) already distinguish
instruction loops/faults/HLT, IDE/PIC/PIT activity and literal `3F` versus ANK
fallback cells. No extra telemetry or speculative execution changes are justified
by the newly verified URL. A transient NUL background on the working control
still does not explain the reported persistent display and absent Enter response.

An independently eligible environment can next validate the exact archive and
component licenses privately, preserve an intact original and separate working
copy outside the checkout, then use the bounded Ap/IDE command below. Compare
kernel handoff and short later checkpoints with the control before selecting a
CPU/BIOS/IDE/input/rendering correction. Share only a short technical summary;
keep inventories, hash records, detailed traces and media private. Any diagnosed
defect needs an original or rights-cleared minimal fail-before/pass-after guest
in native and production Wasm. Until such evidence exists, the first divergence,
cause of the reported hang, shell responsiveness and game start remain **not
tested**. No independent large missing feature has been identified to justify a
new speculative task. #113 remains the continuation; no closing reference or
post-merge verification is introduced.

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
