# CRT and KCG BIOS contracts

Issue [#134](https://github.com/takahirox/pc98-rust/issues/134) is covered by an
original Rust GDC/scanout implementation and an original MIT boot probe. No
emulator implementation, NEC ROM dump or private game was consulted or included.

The primary source is NEC's *PC-9800 Series Technical Data Book, BIOS volume*
(1992), [scan](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf),
printed pp. 37–55 (PDF pages 54–72). The mode diagrams on pp. 41–42, area
list on pp. 44–45 and cursor diagrams on pp. 53–55 were inspected visually,
including their N/H profile icons. The icons are defined on introductory p. (5).
GDC parameter RAM, PITCH and CSRFORM follow NEC's
[μPD7220 documentation](https://bitsavers.trailing-edge.com/components/nec/uPD7220/uPD7220_A_Single-Chip_Graphics_Display_Controller_Feb1981.pdf),
attached data sheet printed pp. 5–7: four text partitions, raster-line lengths,
word-address stride, cursor row height and a blink half-period of `2 × BR` frames.

## Profile boundary

**The NEC source explicitly marks AH=1Ch, 1Dh and 1Eh as high-resolution-only.**
Its overview on p. 37 marks all three unavailable in normal mode; pp. 53–55
carry only the H icon. Neither the VM nor Ap profile implements the 1120×750
high-resolution CRT. The original issue's assumption that those are ordinary
normal-resolution Ap services is therefore not supported by that source.

To provide the combined callable Ap family explicitly requested by the issue,
these three entries are an **Ap compatibility extension**, using the documented
bit fields on the existing 640×400 text hardware. They do not claim NEC Ap ROM
parity or high-resolution support. In this extension, DH/DL are normal-resolution
raster-line numbers, not the halved, interlaced high-resolution coordinates in the
source's p. 54 diagram. Initialization chooses 80×25, pitch 80 and one area at 0;
it applies AL's KCG and cursor fields. VM rejects these three entries. VM supports
the common normal-mode area and cursor services and code-access selection; its
persistent dot mode remains unsupported. No Ap-only hardware is advertised on VM.

## Contract matrix

All listed services leave flags and segment registers intact. Unless specified
otherwise, this implementation also preserves every general register, including
registers whose contents NEC leaves unspecified. There is no invented CF/AH error
status: unsupported requests stop with a bounded descriptive emulator diagnostic,
before changing the prior display or BIOS table. Invalid guest buffers retain
normal translated-memory fault behavior.

`053Ch` is the synthetic firmware's CRT status byte. Mode set and sense AL bits 0/1 are
`1=20 rows` / `1=40 columns` (`0=25 rows` / `0=80 columns`).
Bits 2/3 select simple-graphics attributes / KCG dot access. Sense bit 7 is zero
for the implemented standard CRT. Direct hardware I/O changes hardware; it does
not rewrite the BIOS-maintained mode table. A subsequent BIOS mode/KCG selection
updates that table as well as the shared hardware. No extra invented work-area
bytes store pitch, cursor or partitions.

| AH | Profiles / NEC page | Inputs | Outputs / preserved registers | BIOS work area | Device and renderer effect | Reset / unsupported variants |
| --- | --- | --- | --- | --- | --- | --- |
| 0Ah / 0Bh | VM, Ap; 41–42 | Set: AL bits 0–3 above; sense: none | Set preserves all; sense returns AL, others preserved | Set writes mode encoding to 053Ch; sense reads it | Physical pitch 80; 16/20-line cells; 80/40-column pixel clock and even-word traversal; attribute selection; Ap KCG selection; one full area | 80×25, vertical-line attributes, code access, sense=00h; high/reserved bits rejected; VM ignores mode-set bit 3 as documented |
| 0Ch / 0Dh | VM, Ap; 43 | None | All preserved | None | GDC display start/stop, independent of port 68h text gate | ROM IPL starts display; firmware initialization alone leaves GDC stopped |
| 0Eh | VM, Ap; 43 | DX=CPU byte offset in text VRAM | NEC excludes DX; implementation preserves it too | None | One 400-line SCROLL area at DX/2; clears the other three areas | Start=0; physical text address wraps at 4096 words |
| 0Fh | VM, Ap; 44–45 | BX:CX list; **DH** first area 0–3, **DL** count 1–4; each entry: word start (0000h–0FFFh), word rows (1–20/25) | All preserved | None | Converts rows to raster lines and writes selected GDC parameter-RAM entries; unselected entries survive; scanout selects each area in sequence | Area 0 covers 400 lines; others empty; invalid start/count/entry rejected atomically; no more than four areas |
| 10h | VM, Ap; 45–46 | AL=0 blink, 1 steady | All preserved | None | CSRFORM full-cell cursor, BR=12; retains visibility; ignores CH/CL and DX | Full 16-line cell, blink, hidden; other AL rejected |
| 11h / 12h / 13h | VM, Ap; 46 | Show/hide; position DX=CPU byte offset | All preserved | None | Shared CSRFORM visibility, retained shape/blink; CURS word address; same address used for cursor readback and scanout | Hidden, address 0; GDC CURD exposes position |
| 14h / 1Ah | Ap mutable KCG; common replacement-font reads; 47–52 | BX:CX points to two scratch bytes followed by pattern; DX=JIS/ANK code or user definition | NEC excludes AH/BX/CX/DX; implementation preserves all | None | Shared CG font/user RAM; ANK DH=00h: 8 rows; DH=80h: 16 rows; full-width JIS: 32 bytes; 1Ah writes the existing 188 Ap slots | Reset clears user RAM; original Latin and licensed Shinonome fonts remain; no 24-dot font, NEC extensions or newly enabled VM user RAM |
| 1Bh | Ap code/dot; VM code only; 53 | AL=00h code, 01h dot | NEC excludes AX; implementation preserves all | Updates bit 3 of 053Ch | Same port 68h 0Ah/0Bh flip-flop as direct I/O; dot enables CG window and prevents text Kanji; glyph RAM/graphics untouched | Code; AL>1 and VM dot rejected |
| 1Ch | **Ap extension**; NEC H-only, 53–54 | AL: bit7 blink, bit6 present, bit5 KCG dot, bits4:0 BR (0→12); DH/DL top/bottom | All preserved | 053Ch=00h or 08h | Initializes text mode, pitch, area and CSRFORM; preserves cursor address, character/attribute VRAM, glyph RAM, bitmap GDC, palette and other devices | Normal 80×25; invalid top/bottom rejected; VM/high-res unavailable |
| 1Dh | **Ap extension**; NEC H-only, 54–55 | AL=physical columns, even 80–162 | All preserved | None | GDC PITCH; next text row uses this stride in every partition; VRAM wraps at physical end | Pitch 80; odd/out-of-range columns rejected; no high-resolution wraparound modes |
| 1Eh | **Ap extension**; NEC H-only, 55 | AL bit7 blink, bits4:0 BR (0→12); bits6:5 ignored; DH/DL top/bottom | All preserved | None | CSRFORM shape and blink; preserves prior visibility and character row height | Full-cell BR=12; top>bottom or bottom outside current cell rejected |

## Scanout and remaining limits

The text GDC owns the 16-byte SCROLL parameter RAM, character-row height, cursor
shape, visibility, blink and pitch. BIOS commands and guest writes to ports
60h/62h use this same state. Partial PRAM writes honor their command offset.
Text character/attribute RAM spans 4096 words; addressing wraps there. The first
partition's start also remains available through the existing GDC `start` field.
Rows whose four partition lengths have been exhausted are blank. The existing
fixed video scheduler supplies the frame clock; render calls do not advance it.
Cursor half-period is `2×BR` frames; attribute blink uses the slower 3/4-on duty.
40-column mode doubles pixels and skips odd VRAM words while preserving physical
pitch. Twenty-row mode includes four blank face lines after each 16-line glyph.
Simple graphics uses eight character-code blocks; vertical-line/underline/reverse
and visibility attributes are applied during scanout. This is a bounded normal
text implementation, not arbitrary GDC mixed graphics, zoom or high-resolution
interlace. `Bus::text()` remains the raw 80×25 diagnostic dump; framebuffer pixels
are the visible scanout oracle.

KCG dot mode does not select graphics resolution. It makes the existing Ap CG
window readable with A1h/A3h character selection and A5h half/line selection.
A9h and the window read the same ROM/user data as BIOS font reads. Returning to
code mode restores text Kanji from the unchanged storage. NEC specifies garbage
text Kanji in dot mode, without defining their pixels; this implementation
chooses blank glyph pixels deterministically. ANK, attributes, graphics and cursor
continue to work. This does not promise cycle-exact CG read throughput or ROM
content, full Japanese ANK, 24-dot glyphs or writable CG ROM.

## Reproduction and validation

Run `python3 tools/prepare_crt_fixture.py`, then `cargo test --locked --test crt`.
The script assembles `guest/crt.asm`, verifies the committed source/image hash
manifest and creates test-only `fixtures/crt/` images plus source/license copies.
The common source's `FIRST` assembly parameter produces independent first-call
entries for 0Fh, 1Bh, 1Ch, 1Dh and 1Eh. On upstream checkpoint
`28646db582b2c073c880b549751c280e09f3494d`, each was actually booted and stopped
at its matching `unsupported BIOS INT 18 AH=xx` diagnostic. The main image's
first call reproduces the reported AH=1Bh failure. The new implementation boots
all five entries and the combined sequence without that failure. To reproduce
an old trap, retain the generated fixture directory and run the upstream core
from a separate checkout (substitute the absolute fixture path):

```sh
cargo run --locked --manifest-path /path/to/upstream/Cargo.toml --bin headless -- /path/to/fixtures/crt/first-1b.hdm 100000 --ap
```

Repeat with `first-0f.hdm`, `first-1c.hdm`, `first-1d.hdm` and `first-1e.hdm`.
The same source-built images reach stage 1 in the new native guest test.

The probe uses a real FDC boot, actual guest INT, executable firmware thunk and
IRET, compares register/segment/flag snapshots, defines an asymmetric original
glyph, reads it through BIOS/ports/CG window, checks a ROM glyph independently
against window/port reads, and pauses for keyboard input at four display stages.
Hosts only step, render, read results and supply keys. Native and release-Wasm
Chromium tests check real area/pitch/glyph/cursor pixels, observe both blink
phases using bounded guest execution, restore the normal display, check guards
and exact original disk bytes, then repeat after reset. Browser tests run at
both `/` and `/pc98-rust/`. Native tests cover VM common services, all 16 mode
combinations, every even width 80–162, malformed requests without partial state,
partial area updates, physical wrapping, direct GDC programming and reset.
Generated fixtures are ignored and remain excluded by the unchanged strict
published-asset allowlist.

## Recorded checks (2026-11-05)

| Check | Result |
| --- | --- |
| README fixture/source/license preparation and `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | Prepared all documented fixtures; 10 Python tests passed |
| `python3 tools/prepare_crt_fixture.py` | Source, boot and image hashes verified; identical probe used natively and in Chromium |
| Upstream red controls | All five first-service variants produced their exact unsupported-BIOS diagnostic; the new core reached stage 1 for all five |
| `cargo test --locked` | 563 passed, zero failed/ignored, across 40 test/doc-test targets; includes seven CRT tests, VM/Ap font/video/CG, FreeDOS HIGH/HimemX and 619,664-byte allocation, HDD export/writes, FDD hot swap, protected/VM86 and sound regressions |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check` | Passed |
| Real release-Wasm build (`npm test` build pipeline / `python3 tools/build_web.py`) | Passed; actual `wasm32-unknown-unknown` release compiler, no substituted core |
| Strict `node tools/check_site.mjs` | Passed; exactly 24 approved publication assets; no CRT fixture published |
| Full Chromium run, two workers, `/` and `/pc98-rust/` | 231 passed initially, two desktop demo-loading race failures, three existing intentional skips; corrected test synchronization and reran the two failed cases plus both CRT cases: four passed in 28.3 s. All 233 distinct applicable cases passed across these runs |

The three existing skips are duplicate `/pc98-rust/` large-HDI resource scenarios
explicitly sampled only in the local project (1 GiB guest I/O, large FAT16 DOS,
occupied-media resource measurement); their local cases passed. They are not new
CRT exclusions. Both CRT browser cases check cold/reset, the complete 128-byte
result oracle, actual glyph/area/pitch/cursor pixels, bounded guest-frame blink,
absence of errors and exact unchanged disk exports.

An initial incorrect mode-sense inversion was caught by the desktop memory-probe
regression and corrected against the visually inspected p. 42 diagram before the
final native/full-browser runs. The LTR non-RAM boundary test was moved from
A3000h to A4000h to retain its original descriptor-crossing check after decoding
the complete text attribute RAM. The browser race occurred because a synchronous
Wasm stepping loop ran before the asynchronous bundled demo fetch/reset finished;
the test now waits for the media name and reset control readiness. Application
behavior and the assertion requirements were not weakened. An interrupted earlier
browser run left its two local HTTP servers behind; only those worktree-owned
servers were stopped before restarting validation.

The full native target test times totaled 387.58 s with concurrent browser work;
the full Chromium run took 8.3 min. These are local observations, not a new CI
performance gate. The CRT native guest takes forty framebuffer samples while executing the
same bounded guest slots, rather than rendering two hundred redundant samples.
The release build and successful unaffected browser cases were reused for the
focused reruns. No deployed-site or private-game verification is required.
