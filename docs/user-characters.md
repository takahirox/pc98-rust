# Ap 16-dot user characters (Issue #16)

The Ap DOS profile implements INT 18h/AH=1Ah with guest-owned character RAM.
Definitions feed the text renderer, A9h port reads, the CG window, and full-width
INT 18h/AH=14h reads. A guest can copy those bytes into graphics VRAM; the host
renders that VRAM normally. No game-specific text or host font drawing is used.
The VM profile retains its previous BIOS/CG behavior.

## Contract and sources

The manufacturer’s [PC-9800 Technical Databook, BIOS, printed pp.47–49 and 52](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
specifies **BX:CX**, not DS:DX or ES:BX. DX is the registration code.
The buffer starts with **two scratch bytes**, followed by **32 pattern bytes**:
left byte, right byte, repeated for sixteen rows, MSB first. AH, BX, CX and DX
may change; other registers are preserved. This implementation preserves all
registers and leaves scratch unchanged. Each byte uses a wrapping 16-bit offset
and the ordinary guest bus address mapping. AH=14h full-width reads use the same
buffer layout. The existing simplified ANK path is unchanged.

The BIOS table gives the bounding range 7620h–777Fh and 188 characters; it explicitly
states that BIOS does not check registration codes. The original authors’
[PC-9801 Programmer’s Bible, KCG/user-character discussion](https://vtda.org/books/Computing/Programming/PC-9801Bible_%E6%9D%B1%E4%BA%AC%E7%90%86%E7%A7%91%E5%A4%A7%E5%AD%A6.pdf)
identifies the 188 usable JIS codes as **7621h–767Eh and 7721h–777Eh**.
These are the supported codes, including all four endpoints. Cells 20h/7Fh,
inter-row gaps, other rows, ANK codes and H98 extensions are outside this subset.
An out-of-contract AH=1Ah request stops with an explicit unsupported-registration
core error before accessing the buffer or changing fonts. This is an emulator
boundary, not a claimed hardware error status: the unchecked hardware operation
has no specified safe result for arbitrary codes. No fabricated carry/error
return or successful no-op is supplied.

The manufacturer’s [Hardware databook, printed pp.165 and 194–198](https://vtda.org/docs/computing/NEC/PC-9800TechnicalDataBookHARDWARE%2BOCR_1993.pdf)
defines text codes and CG selection. A1h holds the cell, A3h the row minus 20h;
A5h bit 5 selects left (1) or right (0), with the line in bits 0–3. In the user
character CG window, odd offsets 1,3,…,31 expose the selected half, mirrored
through A4FFFh. Even offsets are unspecified by hardware and return FFh here.
The existing dot-access gate and ordinary replacement-glyph window stay in use.
CG port/window writes and access/display timing contention are outside this
service’s supported subset; definitions are loaded through BIOS.

## Lifetime and assets

All 188 RAM patterns start blank. Redefining a code replaces exactly that pattern;
subsequent BIOS/CG reads and text frames see the new bytes immediately. Existing
graphics pixels remain VRAM contents until the guest redraws them.
`Machine::reset`, browser Reset and profile changes clear every definition along
with the other volatile devices. Merely inserting/ejecting media in the core does
not clear definitions; the browser’s normal mount/reset workflow does. This is a
deterministic emulator reset policy, not a claim about physical RAM retention
across different PC-98 reset mechanisms. Scratch and caller buffers stay guest RAM.
The immutable licensed Shinonome JIS asset and original Latin ANK glyphs are
unchanged; no proprietary ROM, game glyphs, FEP or general extended font is added.

## Reproduction and assertions

Run the [complete repository checks](../README.md#reproduce-the-checks), including
`python3 tools/prepare_font_fixture.py` after FreeDOS preparation. The latter builds
`fixtures/freedos/fontload.hdm` and `userfont.hdm` from the checksum-pinned legal
FreeDOS boot/kernel/shell. These ignored images remain outside the publication
tree; keep that entire folder’s source archives and notices if redistributing.
Both probe sources and their deliberately original pixels use this repository’s
MIT license. No PIPOO data or proprietary BIOS/OS/font is required.

The exact Issue control is assembled in a temporary directory outside the worktree.
It returns to DOS successfully. Its trailing scratch word is intentionally retained
for regression fidelity: with the documented leading scratch layout, its last
pattern row is zero. The extended [guest](../guest/userfont.asm) uses leading
scratch and an asymmetric pattern. It checks BIOS, both CG halves and the odd-byte
window against its own bytes, then draws text and CG-derived planar graphics.
A key replaces the same code with the inverted pattern; another returns to DOS.

[Native tests](../tests/user_font.rs) check every slot, independent DS/ES,
64 KiB offset wrap, buffer guards/scratch, registers, invalid-code rejection,
neighboring slots, replacement glyphs and reset. They boot both controls through
FreeDOS and assert every text and graphics pixel for both patterns.
[Browser tests](../tests/browser/user-font.spec.js) run the actual Wasm in Chromium
at both root and project paths, check all 32 readback bytes and all 512 rendered
pixels per stage, return to FreeCOM, reset, and reload the original pattern.
The fixture step and both test suites are mandatory in CI. Screenshots, manual
play, physical hardware and PIPOO are unnecessary for acceptance.

Optional local PIPOO validation was not performed for this implementation. This
service alone does not establish gameplay, save-system correctness or complete
PIPOO compatibility. No post-merge verification is required.

## Validation recorded 2026-10-21

- Prepared the pinned FreeDOS, DOS apps, CPU, HDD, music and device fixtures with
  their README commands, plus `python3 tools/prepare_font_fixture.py`: passed.
- `cargo test --locked`: 67 native tests passed, including both FreeDOS font
  controls. After strengthening the font storage/register assertions,
  `cargo test --locked --test user_font` passed all three tests again.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`: passed.
- `npm ci` and `npx playwright install chromium`: passed.
- `npm run build`: media/font reproducibility, release Wasm build and publication
  check passed (10 approved assets; replacement font hash unchanged).
- `npx playwright test tests/browser/user-font.spec.js --workers=2`: both root and
  project-path tests passed on the normal ports.
- The later `npm test` encountered an unrelated workspace listening on port 8000.
  `npm test -- --config=/tmp/pc98-issue16-docs/playwright.config.mjs` then passed
  **all 26 tests**. That temporary config imported the repository config and
  changed only the server commands, listening ports/base URLs to 18000/18001,
  and absolute test/server paths. The project server retained the same
  `/pc98-rust/` mapping and root-asset failures. No unrelated server was stopped.
- `git diff --check` and local documentation link checks: passed.
