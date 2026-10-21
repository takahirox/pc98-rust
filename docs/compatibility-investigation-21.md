# Six-game compatibility re-test — Issue #21

Research performed locally on 2026-10-21 for
[Issue #21](https://github.com/takahirox/pc98-rust/issues/21).
**All six exact versions were reacquired and executed; none is confirmed playable.**
The three previously identified blockers are cleared in the affected real guests.
Three new, distinct defects have original, game-free reproductions. Their
[separate filing-ready drafts and probe sources](compatibility-followups-21.md)
are deliverables, not already-created GitHub Issues.

The node instruction explicitly prohibits pushing, PRs, merging and comments.
Consequently no results comment or follow-up Issue was posted, and #21's remote
acceptance steps remain pending. This document can supply the results comment
when a later workflow authorizes publication. No emulator fixes belong to this
research checkpoint, and it does not claim to close #21.

## Baseline, rights and isolation

- Read the live Issue, the [original research comment](https://github.com/takahirox/pc98-rust/issues/13#issuecomment-6309507515),
  [original report](compatibility-investigation-13.md), development/review guidance,
  [Ap limits](ap-devices.md), and implementations/tests for #14–#16.
- GitHub's `main` was `0adef0c2c6227506a2a6b447eb80a4df297ab9c7`, after merged
  PRs #18, #19 and #20. The assigned checkpoint was
  `997a281b5cd1c66303ed1590baaa078744d83c96`; its complete file tree was identical
  to that main commit (`git diff` empty). All runs used that unchanged source.
- macOS arm64, Rust 1.94.0, native release Rust core, `Profile::Pc9821ApDos`,
  original synthetic firmware and replacement font, 640 KiB conventional RAM,
  existing 4 MiB address space, 640×400 renderer and unchanged 8 MHz scheduler.
  There was no Windows, reference emulator, emulator/chip library or proprietary
  firmware/font acquisition. Supplied GR-DOS was used only with its publisher disks.
- On this date, all four original distribution pages and all six archive URLs
  returned HTTP 200 locally. Reviewed MUTATE's current PC-9801 download/common
  notes and publisher manuals, and freshly acquired `MARKADIA.DOC`, `TRIANS.DOC`
  chapters 9–10 and `PIPE.DOC`. MUTATE offers these original boot disks for free
  local use; the other authors permit local freeware use with separate
  redistribution restrictions. No redistribution permission was inferred from
  the freeware category. No unofficial archive was substituted.
- All downloads, supplied drivers/OS, author documents/manuals, reference PDFs,
  generated disks, guest-created files, local instrumentation, traces and native
  frame captures stayed under one mode-0700 `/tmp/pc98-issue21-local-*` directory
  outside the worktree. Only newly authored research text and original MIT probe
  source in Markdown are committed. No restricted bytes went to Git, hosted CI,
  Pages, GitHub Issues/PRs or their attachments. Local frame captures were inspected
  to distinguish actual scenes from changing memory; none is a deliverable.

The exact versions and freshly checked archive identifiers follow. The linked
pages are the original publisher/author-submitted paths, not redistribution grants.

| ID / title / version | Original source / archive | Archive SHA-256 |
| --- | --- | --- |
| SAM / Samurai Shoujoden / 1.10 | [MUTATE](https://www.mutate-soft.com/km-plus/) / [Sam110-GRDOS.zip](https://www.mutate-soft.com/km-plus/master/Sam110-GRDOS.zip) | `d71efe330217a88284241f8d2a93d2d3e07b6c130d05931e6d4d001aa25b3d11` |
| CA / Circuit Angel / 1.01a | [MUTATE](https://www.mutate-soft.com/km-plus/) / [CircuitAngel101a-GRDOS.zip](https://www.mutate-soft.com/km-plus/master/CircuitAngel101a-GRDOS.zip) | `e03bf11b2fb491557dae0321937a541a2d118835ff4e8f671dae4e8f017cda51` |
| HIME / Dengeki Butouki TURBO / 1.56a | [MUTATE](https://www.mutate-soft.com/km-plus/) / [HimeT156a-GRDOS.zip](https://www.mutate-soft.com/km-plus/master/HimeT156a-GRDOS.zip) | `9f0e4dfcc9b1f53ec7a0a518b2e28e017b68e4705040c838532d7f59ede49468` |
| MKD / MARKADIA / 1.05 | [Author release](https://www.vector.co.jp/soft/dos/game/se001354.html) / [mkd105.lzh](https://ftp.vector.co.jp/01/05/3521/mkd105.lzh) | `ef78e05e7140439a7b850fbe0f556602e888f508523abca91c32d57b3fc594bf` |
| TRI / TRIANS98 / 1.02 | [Toxsoft release](https://www.vector.co.jp/soft/dos/game/se011295.html) / [trian102.lzh](https://ftp.vector.co.jp/01/12/337/trian102.lzh) | `da0f682ae5a4233840485353ddeeed15fee47c81621377ec8c420a1fdedba43a` |
| PIPE / PIPOO / 1.00 | [Merveilleux release](https://www.vector.co.jp/soft/dos/game/se005985.html) / [pipoo100.lzh](https://ftp.vector.co.jp/00/59/3547/pipoo100.lzh) | `47cd6560c892766fbe51a8db236073e1e30c8124830ac9e2d9dcadb6156f53c1` |

All hashes match the #13 report. The three FDI headers and lengths were checked
against its geometry tuple; only the 4096-byte container header was removed.
Their HDM hashes match the original report exactly. Each untouched AUTOEXEC still
loads its own supplied NL (`-f6 -O11` for SAM/HIME; `-O11` for CA) before the game.

For MKD/TRI/PIPE, `tools/prepare_freedos.py` was imported with `DEST` redirected
outside the repository. Its four pinned downloads, matching GPL source archives
and notices were retained there. The minimal disk hash was again
`015faaa0dfb4c23f7a8aa4b74bbad2d460afc55c64517c5b4ea7330d4a8f8d1e`.
The original boot sector, DOS files, every original game archive member and the
same #13 AUTOEXEC text were rebuilt into writable FAT12 media. Archive paths and
8.3 names, root capacity and cluster totals were checked. MKD/TRI/PIPE used
795/297/249 clusters; all three prepared HDM hashes match #13 exactly. MKD's batch
and supplied MDRV98 remained intact. No game sector, executable or asset was
patched to bypass a failure.

## Execution and coverage

The release `headless` binary and an external local Rust executable depending on
this worktree by path exercised the same `Machine::step`, bus, firmware, renderer
and sound implementation. Observation did not change guest state. The observer
recorded BIOS parameters, callback entry, font readback equality, actual IN results,
reset-vector entries, short CPU/stack histories, FDC/FM/PCM counters and hashes.
Complete logs remained private; the evidence below is an original-written summary.

Each game first received 35 million single-step slots, or stopped on a core error.
Audio energy was accumulated as sum of squared samples, then the sample queue was
cleared every 100,000 slots. For surviving guests, Return, Space, Escape, Left,
Right and Up were each pressed for 50,000 slots and released for 2 million slots:
47.3 million total. These initial runs took about 8–11.4 seconds per title with
three concurrent local processes; wall time is not guest time.

SAM/CA/HIME/MKD also ran from pristine media to **113.2 million slots**, including
30M/60M observations, Return and Space at 60M/70.1M, then Left, Right and X and
another 30M. Native frames, text/VRAM hashes, display pages, palette color counts
and device counters were inspected. Separate no-input controls for the three
MUTATE titles matched all five sampled frame hashes through 113.2M. Their manuals
were then used for an additional 81M run: BS/Return and keypad 4/6 for the fighters,
N/B and keypad 4/6 for CA, with 2M-slot holds and 1M-slot releases after 60M.
The final 81M frames also matched separate no-input controls for all three titles.
These are attempted controls, not proof of interactive gameplay. No listening,
Wasm/browser run, end-to-end save/reload or full-game completion is claimed.

## Before/after matrix

Before means the bounded #13 baseline `c1aaafabb0a7`; after means current main
above. **Partial** means measured progress with unverified or broken interaction;
**failed** means a new verified launch blocker. None qualifies as playable.

| Exact title/version | Before | After / classification | Video, controls and gameplay | Audio and save/reload |
| --- | --- | --- | --- | --- |
| Samurai Shoujoden 1.10 | NL callback wait | **Partial:** request at slot 1,065,590; actual `108a:1857` callback at 1,085,591, exactly once in 35M; game code reached | Fighting scenes with sprites, backgrounds, portraits and changing timers/health are rendered by 60M and animate through 113.2M. Scripted-key and no-input frames match; these establish attract/demo execution, not menu selection or controlled fighting. Artwork accuracy is not certified | At 113.2M: 28,225 OPN writes, 5,920 key edges, energy 10,015.95500; PCM 0. 711 disk reads, 0 writes. Save/reload untested |
| Circuit Angel 1.01a | Same NL wait | **Partial:** request 1,068,404; `1089:1857` callback 1,088,405, once; game code reached independently | Road/vehicles, portraits, changing timer and racing scenes render/animate. Japanese text appears incomplete; no reference comparison establishes its cause. Input controls match no-input samples; no controlled race/menu transition established | 43,543 OPN writes, 10,624 edges, energy 586.61043; PCM 0. 647 reads, 0 writes. Save/reload untested |
| Dengeki Butouki TURBO 1.56a | Same NL wait | **Partial:** request 1,066,799; `108c:1857` callback 1,086,800, once; game code reached independently | Fighters, backgrounds and timer/health render/animate. No-input control produces the same sampled scenes. No controlled attack, character selection or gameplay transition established | 57,864 OPN writes, 11,068 edges, energy 16,623.50887; PCM 0. 599 reads, 0 writes. Save/reload untested |
| MARKADIA 1.05 | Endless bit-6 GDC wait | **Partial:** actual port-0060h reads include 2,446,044 low and 693,381 high bit-6 results in 35M; execution leaves the old wait repeatedly | VRAM continues changing, but the sampled visible frame remains one color (`87389b5a644f4325`, FNV-1a-64) from 15M through 113.2M. A title, correct opening artwork and controlled gameplay were not established. Direct BIOS key-map polling sees stale bytes; see follow-up C | 25,568 OPN writes but 0 key edges/energy/PCM at 113.2M; no working music claimed. 234 reads, 0 writes. Save/reload unreachable in tested sequence |
| PIPOO 1.00 | INT 18h/AH=1Ah error at 26,559,171 | **Failed at a new stage:** 73 real definitions load successfully, each matching all 32 caller pattern bytes; execution returns to old `3231:0f65` 73 times. New AH=4Ah stop at 26,574,515 | Generated animation data and changing graphics precede the stop. Call uses AX=4A00h, CX=0600h, return `2ff9:0055`; valid drawing-mode service is rejected. Menu, animation correctness and gameplay input not reached | 292 reads, 33 writes; generated `MERVLOGO.3DA` is 29,521 bytes. No `PIPE.SCR` was created. This is initialization, not a tested score save. No FM/PCM/audio energy; reload untested |
| TRIANS98 1.02 | Unexplained repeated reboot | **Failed; immediate reboot cause now evidenced:** missing Graph LIO ROM directory leads `INT A0h` into empty ROM, which runs into reset vector | Seven reset-vector entries including initial boot in 35M. First alias entry `f990:66f0` at slot 5,396,651, followed by firmware at 5,396,652. No user-file prompt, title or input transition reached. See provenance below | Graphics unchanged; no OPN/PCM/audio energy or FDC writes. `TRIANS.USR` creation/save/reload not reached |

The OPN counters and sample energy demonstrate synthesis activity, not correct
musical notes, instruments, balance or fidelity. The sampled visible scenes support
limited rendering/animation claims only. Remaining MUTATE input behavior and
MARKADIA's one-color output are inconclusive beyond the specific evidence below;
there are no speculative graphics, sound, CPU-speed or per-game tickets.

## Remaining defect evidence

### A. PIPOO: drawing timing mode

The [manufacturer's BIOS databook](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf),
printed p.91 (PDF page index 107), defines INT 18h/AH=4Ah: CH=06h selects flash
drawing, CH=16h selects flashless drawing, preserving all registers. PIPOO's
CH=06h is in contract. The unchanged firmware has no such case and explicitly
reports `unsupported BIOS INT 18 AH=4a`. Original `drawmode.asm` reproduces that
same error through FreeDOS at 4,393,206 slots, without any PIPOO material.
Removal of this blocker would not establish that PIPOO becomes playable.

### B. TRIANS98: Graph LIO directory and initialization

This was treated as unresolved until a positive control-path explanation was
available. The game installs the Graph LIO vectors from segment F990h's entry
table, starting with the word at offset 0006h. At slots 5,383,378/5,383,379 the
A0h vector changes from the DOS handler to **F990:0000**, because that table word
is zero in the synthetic ROM. At slot 5,383,474, `INT A0h` at `2d90:79e3` transfers
there. The saved interrupt frame has return `2d90:79e5`; SS:SP is `3ee8:8ade`.
Execution traverses zero-filled ROM until physical FFFF0h (`f990:66f0`) and
executes the synthetic reset-vector far jump. This is guest instruction flow,
not a host `Machine::reset`, FDC reset or demonstrated CPU triple fault.

The [same manufacturer databook](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf),
printed pp.138 and 141–143, specifies the F990h Graph LIO directory, caller vector
registration, work/stack areas and GINIT (normally INT A0h). Original
`lioinit.asm`, using a private 1200h-byte work area and saving the prior DOS vector,
independently reproduces the path: three vector entries including initial boot
in 10M slots; first alias at 4,417,109. Thus the immediate reset cause is positively
explained by the absent ROM service/table. Later LIO calls, later installation
requirements, gameplay and any subsequent faults remain untested. This does not
justify implementing every LIO function as part of this research task.

### C. BIOS keyboard bitmap: independently reproduced, observed in MARKADIA

After 60M slots, holding Return (scan 1Ch) for 2M leaves the device's group-3
pressed state at 10h, while guest RAM `0000:052Dh` remains 00h. A bounded operand
observer identifies MARKADIA reading that address at `2d6c:300f`, as well as other
bytes in `052Ah–0539h`. During this window it reads 052Dh 119 times; this is
actual guest access, not an inference from an unmapped port.

The original `keymap.asm` reads BIOS INT 18h/AH=04h group 3 and independently
compares it to the work-area byte. After booting for 5M slots, injecting Return
and running 1M more, it prints `KEYMAP BWA STALE` and returns to DOS. The
[original authors' PC-9801 Programmer's Bible](https://pc98.ne.jp/devdocs/pc-9801programmersbible.pdf),
printed p.69, identifies the key-state table at 052Ah and shift state at 053Ah;
the NEC BIOS databook pp.32–33 defines grouped held-key sensing. `sync_keys` updates
the ring and modifiers but never mirrors the 16 held-key bytes. This establishes
a distinct BIOS work-area defect and a concrete MARKADIA consumer. It does not
prove the cause of every MUTATE input failure, or MARKADIA's blank video/audio.

Existing repository Issues #1, #3, #5, #7, #9, #11 and #13–#16 plus #21 were checked
through GitHub (open and closed); none is a dedicated duplicate of A/B/C. The
three [draft tasks](compatibility-followups-21.md) group by these contracts, not
by title, and provide original redistributable controls and objective acceptance.
No tickets were actually filed under this node's no-write instruction.

## Validation and publication audit

| Check actually executed | Outcome |
| --- | --- |
| `cargo build --locked --release --bin headless` | Passed against unchanged emulator |
| `cargo test --locked --lib --test core` | 22 core tests passed, no failures/ignored; library has no unit tests |
| External native Rust observers and six exact guest setups | Built and ran; bounds and findings above; no source or guest modifications |
| Existing original timer, hblank and fontload controls on private FreeDOS media, Ap, 16M slots each | `TIMER CALLBACK PASS`; both GDC ports and `HBLANK TRANSITION PASS` plus DOS return; `FONTLOAD RETURNED`, no core error |
| New original drawmode, lioinit and keymap controls | Reproduced A/B/C as detailed above; these are pre-fix failures, not passing acceptance tests |
| `python3 tools/build_media.py --check` | Passed; firmware/demo/license assets reproducible and unchanged |
| `python3 tools/build_font.py --check` | Passed; replacement font hash unchanged |
| Markdown structure, relative links, staged content and `git diff --cached --check` | Checked before commit |
| Git/publication scope | Only the two original-written research Markdown files; no runtime, fixture, dependency, tool, media or workflow changes |

Full DOS fixture/native, Clippy, Wasm/browser and site-build suites were not run:
this is documentation-only work. Existing small original controls were run
privately to verify the relevant baseline; they do not substitute for real-game
re-tests. The additional userfont control was also booted for 16M slots without
a core error, but its interactive acceptance was not exercised or counted as a pass.

The Pages workflow still uploads only `web/`, and `tools/check_site.mjs` retains
its exact ten-asset allowlist. The current publication paths were inspected for
unexpected files; no private directory is within that tree. This is a source/path
audit: nine allowed files are present, and generated `core.wasm` is absent in this
worktree. This is not a new Wasm build or deployment claim. No game became a fixture, CI
download or acceptance dependency. There is no required post-merge verification.

## Downstream handoff

Local research and documentation are complete. When a later node is authorized
to write to GitHub, file the three separate English `task` drafts, then post this
six-title matrix on #21 with their actual URLs. Do not describe the drafts as
existing Issues or mark the remote comment criterion satisfied beforehand.
Do not upload private media/captures or require them in acceptance. The private
directory is ephemeral and is not a dependency for the report or original probe
sources. No usage allowance was purchased, reset ticket redeemed, or provider/model
switched. Current source/CI/Pages and all prior fixtures are preserved.
