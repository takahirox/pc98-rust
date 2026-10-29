# Issue #107 validation

The original Rust interpreter implements the bounded
[i486 paging contract](protected-mode.md#i486-4-kib-paging-issue-107).
All evidence uses redistributable fixtures. The private `aliceY3.hdi` was not
provided and was **not retested**. No subsequent HDD blocker, game boot,
DOS-extender compatibility or playability is claimed.

## Baseline and independent guest

The original MIT [guest](../guest/paging.asm) and
[manifest](../fixtures/paging-manifest.json) are reproducible with NASM.
The 1,261,568-byte disk SHA-256 is
`6541b4c1072a1f92401f76f0efb0a71f6ad08a51f16da46084345c14c5fae4c2`.
Preparation copies the original source, repository MIT license and manifest next
to the generated disk, outside `web/`. No proprietary disk/ROM/font or emulator
core/dependency is added.

The identical disk was run on an isolated source archive of baseline
`d10f28a4dcc39e62639c2029c1bab5416aeda9de` and reproduced:

```text
unsupported paging transition: CS:IP=0008:0001fcd2 opcode=0f 22 modrm=c0
CR0 GPR=eax old=60010011 written=e0010011 PE=1 PG=1
next-ip=0001fcd5; write not committed
```

The implemented core prints guest-generated `PAGING PASS` in 6,681 retired
instructions. It checks nonidentity code/data/stack, a guest #PF frame and PTE
repair/IRETD retry, CR0/CR2/CR3/SMSW, stale mappings and INVLPG/CR3 reload,
16/32-bit protected execution, PG disable and exact result/stack guards. The
native test also checks final control registers, fault metadata and unchanged
disk bytes. Chromium executes the production release Wasm and checks the same
guest result and guards, cold and after reset at both serving roots.

To repeat the baseline without changing this worktree (requires the named
revision in local Git history):

```sh
python3 tools/prepare_paging_fixture.py
python3 - <<'PY'
import io, pathlib, subprocess, tarfile, tempfile
image = str(pathlib.Path('fixtures/paging/paging.hdm').resolve())
revision = 'd10f28a4dcc39e62639c2029c1bab5416aeda9de'
with tempfile.TemporaryDirectory() as directory:
    archive = subprocess.check_output(['git', 'archive', revision])
    tarfile.open(fileobj=io.BytesIO(archive)).extractall(directory, filter='data')
    result = subprocess.run(['cargo', 'run', '--locked', '--manifest-path',
        str(pathlib.Path(directory) / 'Cargo.toml'), '--bin', 'headless', '--',
        image, '50000', '--ap'], capture_output=True, text=True)
    assert 'unsupported paging transition' in result.stderr
    print('Baseline reproduced')
PY
cargo run --locked --bin headless -- fixtures/paging/paging.hdm 50000 --ap
```

## Automatic checks

Run every fixture producer in [README](../README.md#reproduce-the-checks), then:

```sh
python3 -m unittest discover -s tools -p test_fixture_fetch.py
python3 tools/build_font.py --check
python3 tools/prepare_paging_fixture.py
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
git diff --check
npm ci
npx playwright install chromium
npm test
```

This run prepared all 43 fixture producers, including checksum-pinned FreeDOS
and matching source archives, applications, CPU/HDD/music/device/font fixtures
and original MIT probes. The fetch-helper tests passed (7 tests), as did the
focused paging, protected-mode, control-register, trace and DMA tests. Final results:

| Check | Result |
| --- | --- |
| `cargo test --locked` | 482 passed, zero failed/ignored; 30 test-suite summaries including empty binary/doc suites |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check`, `git diff --check` | Passed |
| Complete real-Wasm Chromium suite | 150 passed (both roots, including paging cold/reset); 1.5 minutes |
| `npm run build` and publication audit | Passed; ten approved assets, valid 512,045-byte release Wasm |
| Fixture-fetch helper tests and all 43 fixture preparations | Passed |

Final Wasm SHA-256:
`669051d330e729e569ddd39cd4c41a8ad4d4590e8cc2945ed8c9162d4dad5512`.
`Cargo.lock` is unchanged and `cargo tree --locked` lists only this crate.

Port 8000 was already occupied by an unrelated local server. Browser verification
therefore used this temporary config, with the same tests and production build;
the Pages server remained at port 8001. The temporary config is not committed.

```js
import config from './playwright.config.js';
config.projects[0].use.baseURL = 'http://127.0.0.1:18000/';
config.webServer[0].command = 'python3 -m http.server 18000 --bind 127.0.0.1 --directory web';
config.webServer[0].url = 'http://127.0.0.1:18000';
export default config;
```

The exact commands were `npm run build` and
`npx playwright test --config .paging-playwright.config.mjs`.
The build verifies firmware/media/font checksums, builds release Rust/Wasm and
audits exactly the ten permitted publication assets and Wasm exports. Generated
paging and third-party test fixtures stay outside the publication tree.

## Coverage and remaining boundaries

The native paging permission matrix tests both levels' Present/RW/US bits,
supervisor/user helpers and WP. Other probes cover A/D and cached accounting,
nonpresent and permission repair, physical page walks, A20 (including a cached
clean PTE becoming MMIO), PC-98 RAM/text/graphics/ROM/open bus, cross-page spans,
32-bit linear wrap, 16-bit stack offsets, nonidentity GDT/IDT/LDT and LTR busy
updates, first-prefix fetch/operand fault frames, EGC rollback, and nested
GP→PF/PF→GP/PF→PF/double/triple-fault delivery. Actual FDC DMA transfers remain
physical while CPU reads use a different frame.

Explicit limits remain ordinary writable-RAM paging tables, diagnostic stops
for reserved present flags, deterministic TLB replacement and no cache-cycle
fidelity. Privilege/TSS switching, VM86, DPMI, numeric x87, PSE/PAE and Windows
remain separate features. The existing independently reproduced privilege/task
and numeric diagnostics retain their grouped roadmap; this run supplies no new
private-software compatibility finding or speculative follow-up Issue.
