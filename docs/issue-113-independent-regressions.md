# Issue #113 independent CPU and firmware regressions

This record describes the original regressions and validation on the sanitized
Issue #113 tree. The comparison base is public `main` at
`a33bca6` (PR #119), fetched from GitHub during this validation. The implementation
builds on that base without importing old private Run history or investigation
documents. All evidence below comes from repository-authored MIT guests and
tests, or the existing pinned legal FreeDOS controls.

## Corrected behavior and independent evidence

Real-mode IRET now validates successive stack operands, allowing 16-bit SP to
wrap between operands while still rejecting an operand that crosses its segment
limit. Protected-mode and VM86 frames keep contiguous validation. The original
[IRET guest](../guest/iret_wrap.asm) exercises a BIOS interrupt entered at SP=2,
two word frames and two dword frames. The guest checks carry, restored SP and
guards and emits `IRET WRAP PASS`. Native tests additionally require zero faults,
unchanged media and zero writes. The production-Wasm tests require the same
marker, result bytes and unchanged media, cold and after reset at both roots.

INT 1F/AH=90 extended copies accept supported 386 data-descriptor limit bits,
granularity, accessed and AVL bits while rejecting invalid descriptors and
out-of-range RAM requests. In VM86, descriptor tables, source, destination and
saved carry use the guest's translated address space. Destination permissions
are checked before mutation. The original
[VM86 firmware guest](../guest/vm86_firmware.asm) combines this copy with mapped
BIOS timer state, recoverable timer/RTC page faults, a split nonidentity RTC
buffer, saved carry, interval callback and keyboard input. Native and browser
tests require `VM86 BIOS PASS`, exact results and unchanged media cold/reset.

Blocking BIOS keyboard input now executes synthetic ROM STI/HLT/retry
instructions through the CPU. Thus IOPL checks and VM86 HLT exceptions reach the
guest monitor instead of host code forcing IF or halting the virtual task. The
existing original [VM86 BIOS monitor](../guest/vm86_bios.asm) handles the expected
HLT fault and resumes ordinary IRQ/key delivery. Its native checks account for
those actual monitor entries. Denied firmware OUT requests cannot leak into a
later CPU slot.

Firmware reads and writes use the shared address-translation helpers, including
Graph LIO GINIT's bounded work area and return-frame alias checks. Service output
preflight protects device/FIFO state from buffer faults. Independent native
coverage includes IDE buffers and scatter-mapped floppy DMA; DMA still writes
physical RAM without setting CPU PTE dirty bits.

## Reproduce

Follow [all fixture preparation and required checks](../README.md#reproduce-the-checks).
The new offline preparation commands are:

```sh
python3 tools/prepare_iret_wrap_fixture.py
python3 tools/prepare_vm86_firmware_fixture.py
```

Each script verifies a checked manifest and retains the original assembly,
license and manifest beside its generated disk under `fixtures/`, outside
`web/`. The manifests pin only independent fixture bytes:
[IRET](../fixtures/iret-wrap-manifest.json) and
[VM86 firmware](../fixtures/vm86-firmware-manifest.json).

Focused native checks after preparation:

```sh
cargo test --locked --test cpu386 --test devices_ap --test vm86 \
  --test iret_wrap --test vm86_firmware --test vm86_bios
```

`npm test` builds the release Rust/Wasm core, audits the approved publication
assets and executes Chromium against `/` and `/pc98-rust/`. Both new browser
tests verify the independent disk against its checked manifest before mounting.
The VM86 firmware browser probe supplies Enter through ordinary browser input.

## Fresh validation

The two new boot probes were copied, with their independent fixtures, into a
temporary export of public `main` without changing its emulator sources. Both
failed there: IRET did not reach its pass marker; the VM86 firmware guest stopped
at the unsupported INT 1F VM86 copy. This establishes fail-before behavior
independently of any private image.

The following additional native regressions each failed against unchanged public
`main` and passed in the corrected tree's full native suite:

- `real_iret_wraps_between_stack_operands_but_not_inside_them`
- `int1f_copy_accepts_386_limit_bits_and_granularity_but_rejects_invalid_descriptors`
- `vm86_firmware_extended_copy_uses_mapped_descriptor_data_and_preflights_destination`
- `vm86_blocking_bios_keyboard_wait_executes_sti_hlt_with_monitor_protection`
- `vm86_firmware_floppy_dma_preserves_scatter_mapping_and_fault_atomicity`

Fresh checks on 2026-10-31 (local date):

| Check | Result |
| --- | --- |
| Fixture-fetch Python unit tests | 7 passed |
| All Python preparation/check commands in `emulator.yml` | Passed, including pinned binaries, matching sources, licenses, manifests, media and font checks |
| `cargo test --locked` | 546 passed; zero failures or ignored tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` | Passed |
| `npx playwright install --only-shell chromium` | Passed; the redundant full-Chromium download stalled and was stopped |
| `npm test -- --workers=4` | 168 passed at both roots, including cold/reset probes; release Rust/Wasm build and audit of exactly 10 approved assets passed |
| `git diff --check` and documentation relative links | Passed |

The initial `npm test` attempt used eight workers: 162 passed and six early local
tests failed during page startup (two connection resets and four profile-enable
timeouts while Wasm was loading). No emulator assertion failed in those six
tests. The full four-worker rerun passed without changing emulator code or
weakening assertions; the initial startup failures remain an execution caveat.
The existing legal HDD controls passed kernel handoff and actual `PROBE`/`ECHO`
shell input cold/reset in native and production-Wasm execution.

## Historical report and private observation

[The existing HDD investigation](hdd-boot-investigation.md) remains byte-for-byte
at its already-public `main` contents. Its statements that the private image was
not tested and its hang remained unresolved describe those earlier independent
control milestones; they are not a report of this correction's implementation
stage or fresh private testing in this validation node.

The supplied recovery context reports an inherited local implementation-stage
observation: the affected boot/input path reached a responsive shell on cold boot
and reset. This node did not access or retest private media. Fresh public tests
above establish the independent architectural corrections; they do not establish
complete commercial-game playability. No private media, detailed diagnostics,
hashes, memory/code traces or screenshots are included here. This validation
record does not close Issue #113 or create a new private-testing approval gate.
