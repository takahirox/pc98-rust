# Ap DOS devices and reproducible guest evidence (Issue #11)

The selected DOS workflows now exercise extended RAM, an IDE boot disk, EGC
aligned raster operations, PEGC packed pixels, a bus mouse, game music and 86 PCM
in the original Rust core. This is a bounded compatibility implementation, not
complete PC-9821 hardware fidelity. The existing CPU interpreter and VM profile
are preserved. No CPU opcode extension was needed for these fixtures.

## Reproduction and use

Run the README checks, including the three additional preparation commands:

```sh
python3 tools/prepare_freedos.py
python3 tools/prepare_dos_apps.py
python3 tools/prepare_cpu_fixture.py
python3 tools/prepare_hdd_fixture.py
python3 tools/prepare_music_fixture.py
python3 tools/prepare_device_fixture.py
```

Preparation needs Python 3.12+, NASM, a C compiler and make. The music script
builds the checksum-pinned JWasm **assembler**, then the independently authored
PMD driver and MC compiler as **guest executables**. The device script reuses
that assembler for DEBUG. No emulator or chip implementation is downloaded.
Preparation fails if input, file, adapted source or image hashes differ from the
checked-in manifests. Do not use `--update-manifest` for routine verification.

Select **PC-9821Ap DOS** in the browser. For `fixtures/dos-apps/adv98.hdi`, select
**IDE · HDI boot**, then insert the image. It boots FreeDOS and the unchanged
ADV98 game with an original guest mouse driver. Enable **Send mouse movement and
buttons**: relative movement selects menus, left click accepts, and right click
opens the game menu. Movement is relative rather than a host-positioned cursor;
the engine deliberately hides the DOS mouse cursor. Uncheck the box to use only
the keyboard. Blur, pause and reset release buttons and keys.

Save with Esc, Return, Return; export the selected IDE disk and reinsert the HDI
to restore saves. Reset preserves disk writes and resets device/CPU/RAM state.
Inserting floppy 1 or IDE chooses that medium for the next reset; floppy 2 never
changes the boot choice. VM cannot mount a new IDE image; switching profiles
preserves existing media and only exposes IDE to Ap.

Insert `music.hdm` in floppy 1 and enable sound. FreeDOS runs MC on the original
MIT `guest/play.mml`, installs the published PMD guest driver and starts ADV98.
The title is silent. Starting the scenario executes its existing `#bgm PLAY.M`
command, loads the compiled score through guest DOS, and calls the guest PMD API.
PMD's timer interrupt sequences FM and SSG notes. No host driver/API or original
ADV98 release music is substituted. The older `fm98.hdm` remains supplemental.

`pegc.hdm` runs the original RAM probe, then independent DEBUG command parsing,
port output and memory fill/copy commands. It writes EGC planes, all PEGC banks
and all 256 palette entries through guest CPU instructions. DEBUG's optional
x87 detection is disabled in its build; its command implementation is unchanged.
The utility is a device fixture, not an additional game compatibility claim.

Native equivalents use the same core:

```sh
cargo run --release --bin headless -- fixtures/dos-apps/adv98.hdi 16000000 --ap
cargo run --release --bin headless -- fixtures/dos-apps/music.hdm 18000000 --ap
cargo run --release --bin headless -- fixtures/dos-apps/pegc.hdm 22000000 --ap
```

## Interfaces and limits

| Area | Implemented contract and evidence | Unsupported or unvalidated subsets |
| --- | --- | --- |
| Extended RAM | NEC INT 1F/AH=90, ES:BX GDT, CX **bytes**, SI=DI=0. Source/destination data descriptors at +10h/+18h; present, expand-up RAM descriptors, writable destination, 64 KiB limits. Copy preserves A20 state, supports overlap, validates both ranges before writing. Guest `MEM98.COM` round-trips data at physical 200000h. Native tests check top-of-RAM, holes, ROM, overflow, error carry, VM rejection and reset | No XMS/EMS manager, protected mode or DPMI. The selected guests use conventional RAM and/or INT 1F rather than XMS/EMS. Descriptor granularity/expanded limits are rejected. A20/HMA behavior remains covered by the preserved CPU tests |
| IDE | One master on PC-98 bank zero, 430/432 bank selection, 640h 16-bit PIO data, 642–64E task file, 74C alternate status/control, reset, IDENTIFY, CHS/LBA read/write/verify, sector count 0=256, INT 11h on slave IR1. Firmware uses this same task file/data path | No ATAPI, DMA, second drive/bank, SCSI/SASI electrical controller, device timing fidelity or copied NEC IDE option ROM |
| HDD image/BIOS | HDI header geometry; exactly 512-byte sectors, 1–16 heads, 1–63 sectors, bounded cylinders, exact declared length/offset, 32 MiB maximum. INT 1B DA/UA 80h zero-based CHS or 00h LBA; geometry, reset, read/write/verify; failures return carry. BWA 55D advertises disk, 584 boot DA/UA. Original partition IPL selects active DOS partition and loads its unchanged GPL FreeDOS HDD boot sector | Only HDI is accepted for IDE. HDI headers/padding survive export. Filesystem/partition parsing and file writes run entirely in guest DOS. No HDD content or application names are recognized by Rust |
| EGC | 6A unlock/mode selection, 7C charger enable, 4A0–4AF register byte lanes for CPU word I/O, active/read planes, compare read, foreground/background/pattern selection, pattern latches, all eight ROP truth-table terms and per-lane masks. Independent DEBUG fills three planes and copies selected-plane readbacks into its own conventional RAM | Only aligned transfers, shift register 0 and length 15 are exercised. Arbitrary bit shifts, streaming bit lengths and GDC drawing through EGC remain unvalidated; no full EGC fidelity claim |
| PEGC | 6A=07/21 extended mode, 512 KiB storage, E0004/E0006 sixteen 32 KiB bank selectors, A8000/B0000 windows, full-byte A8/AA/AC/AE palette, two 256 KiB display pages, GDC PITCH/start; 640×400 packed pixels. B8000 is open bus in this mode. Standard 16-color CG/GRCG/page behavior remains tested | No PEGC plane/ROP mode, linear upper-memory aperture, 480-line mode or full reset palette. Packed/planar cross-mode pixel reinterpretation is not modeled. Unimplemented PEGC MMIO reads zero and ignores writes |
| Mouse | 7FD9 buttons/nibbles, 7FDD HC rising-edge latch/counter reset and X/Y/high/low selection, 7FDF 8255 mode/BSR, BFDB 120/60/30/15 Hz, IRQ13/INT 15h. Signed movement wraps in eight-bit hardware counters. Browser DOM pointer events only enter this device | Original MIT guest TSR provides ADV98's INT 33 reset/status/motion/range calls. It has no cursor renderer, acceleration, callbacks, wheel or full Microsoft driver API; no host INT 33 implementation |
| Sound | SSG port A reports inactive joystick and INT5/IRQ12 selection, fixing PMD's real interrupt discovery. Existing mathematical OPN synthesis extends to six FM channels through the masked 18C/18E bank, register 29 channel enable, timers/IRQ. A460 ID/mask; A466/A468/A46A/A46C bounded 32 KiB playback FIFO, eight hardware rates, signed 8/16-bit mono/stereo input, threshold IRQ and reset. PCM and FM mix into the existing 48 kHz mono Web Audio stream | Approximate FM/envelopes, no analog fidelity, prescaler switching, channel-3 special mode, LFO, exact pan/envelope scaling, built-in rhythm samples, ADPCM RAM/decoder or recording. The accepted game uses FM/SSG, and 98VIDEOP uses 86 PCM; neither needs rhythm or ADPCM. No chip sample ROM is included |

The original GDC display start/PITCH/PRAM and CPU-driven raster/scroll paths remain
in use. No vector drawing engine or extra ANK/kana CG was needed by
these guests; those remain unsupported. [Issue #16 user-character support](user-characters.md)
adds mutable Ap font RAM separately. Shinonome Japanese glyphs, license and
attribution are unchanged. The fixed 8 MHz scheduling model is retained; it is
not a physical i486DX2/66 MHz timing model.

## Documentation used

The Ap's [NEC specification](https://support.nec-lavie.jp/support/product/data/spec/cpu/b211-1.html)
defines the physical baseline. PC-98 contracts come from the independent
UNDOCUMENTED 9801/9821 chapters for
[IDE](https://www.webtech.co.jp/company/doc/undocumented_mem/io_ide.txt),
[mouse](https://www.webtech.co.jp/company/doc/undocumented_mem/io_mouse.txt),
[EGC](https://www.webtech.co.jp/company/doc/undocumented_mem/io_egc.txt),
[display/palette](https://www.webtech.co.jp/company/doc/undocumented_mem/io_disp.txt),
and [sound](https://www.webtech.co.jp/company/doc/undocumented_mem/io_sound.txt).
Packed-pixel PEGC memory details follow the hardware description in
[SL9821's PEGC technical notes](https://www.satotomi.com/sl9821/sl9821_tec5.html),
not its emulator implementation. INT 1F/90's byte count/GDT contract is from
[Ralf Brown's Interrupt List release 61](https://www.cs.cmu.edu/~ralf/files.html).
FreeDOS guest sources establish the actual boot/BIOS requirements, including the
PC-98 IPL signature at FEh and partition layout. PMD sources establish its guest
driver's port/IRQ calls. None of these guest sources implements host emulation.

## Acceptance and provenance

The machine-readable [HDD](../fixtures/hdd-manifest.json),
[music](../fixtures/music-manifest.json) and
[device](../fixtures/device-manifest.json) manifests pin archives, adapted source,
per-file rights, binaries, media, requirements and builds. Original ADV98 and
98VIDEOP provenance remains in the existing manifest. All generated DOS images,
sources, compiler and notices remain outside `web/`; Pages still publishes only
its original ten approved assets. See the [license audit](licenses.md).

Native checks verify guest title pixels, real FAT file creation/save/reload,
PIO/IRQ/error boundaries, mouse latch/IRQ/guest-driver state, all PEGC banks and
palette entries, EGC readbacks, RAM service errors and sequenced game music.
The production-Wasm Chromium suite repeats guest execution at `/` and
`/pc98-rust/`: DOM pointer events reach guest driver-owned RAM and change ADV98's
UI, HDD exports preserve its save file, DEBUG produces actual framebuffer colors,
and ADV98 starts non-silent PMD music in a running Web Audio graph. Existing VM,
floppy/D88, Japanese, video, FreeDOS and CPULEVEL regressions remain mandatory.

The selected workflows meet the device acceptance with the limits above. This
does not promise arbitrary period games or complete hardware support. No Windows,
VGA, external backend, human approval, proprietary media or post-merge validation
is introduced. Any future extension should identify an independent guest need and
add comparable automatic evidence before expanding these claims.
