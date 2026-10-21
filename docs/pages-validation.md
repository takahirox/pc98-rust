# Issue #5 configuration and pre-merge validation

The Pages implementation builds the existing Rust core and cleared static demo.
It uploads only `web/` and does not change emulator behavior or licensing.

## Repository configuration

On 2026-10-21, authenticated repository administration was available. The initial
`GET /repos/takahirox/pc98-rust/pages` returned HTTP 404, and repository metadata
reported `has_pages: false`. The implementer enabled the Actions publishing source
with:

```sh
gh api --method POST repos/takahirox/pc98-rust/pages -f build_type=workflow
```

A subsequent GET confirmed `build_type: workflow` and
`html_url: https://takahirox.github.io/pc98-rust/`; `status` was null. The generated
`github-pages` environment was inspected: it has a custom deployment branch policy
allowing the `main` branch and no required-reviewer or wait-timer rule. No admin
credential was stored in the repository or printed. No permission limitation
prevented this setup. This configuration evidence does **not** verify a successful
deployment or a working public URL.

The official action release metadata and `action.yml` files were checked before
selecting compatible current majors: checkout v7, setup-node v7,
configure-pages v6, upload-pages-artifact v5 and deploy-pages v5. The hosted Ubuntu
runner supports their runtime. See the official
[Pages workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)
and [enablement input](https://github.com/actions/configure-pages/blob/v6/action.yml).
No enablement/admin token is needed in the committed workflow now that the source
is configured.

## Checks actually run

These commands ran in the assigned macOS arm64 worktree on 2026-10-21; no remote
workflow or production deployment was triggered.

| Command/check | Final result |
| --- | --- |
| `npm ci` | Passed; locked development dependencies installed, no vulnerabilities reported |
| `npm run build` | Passed; reproducible firmware/media/notices, release Rust/Wasm compilation, publication check |
| `npm run check:site` (also run by every build) | Passed; exactly nine approved files, including 97,230-byte valid Wasm with the actual emulator exports, both disks, HTML/JS/CSS and MIT notices |
| `npx playwright install chromium` | Passed |
| `npm test` | Passed: all 8 Chromium tests, four at the local root and the same four at `/pc98-rust/` |
| `python3 tools/prepare_freedos.py` | Passed; checksum-pinned legal guest/source fixtures prepared outside `web/` |
| `cargo test --locked` | Passed: 22 core integration tests and one required DOS integration test; none ignored |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `go run github.com/rhysd/actionlint/cmd/actionlint@latest .github/workflows/pages.yml .github/workflows/emulator.yml` | Passed; both workflow files accepted |
| Temporary packaging failure probes | Missing/empty/corrupt Wasm, a header-only module without emulator exports, a test-only `fixtures/freedos/` directory and a symlink to a test disk were all rejected; temporary changes were removed and the valid tree rechecked |
| Temporary tar artifact inspection | Passed: only the nine approved regular files, no links, and byte-identical `core.wasm`; no source, `target/` or FreeDOS files |
| `git diff --check` / staged whitespace check | Passed |

The temporary artifact was made with `tar --dereference --directory web -cf
<temporary-file> .`. The first macOS tar attempt included AppleDouble metadata;
repeating with `COPYFILE_DISABLE=1` produced the expected nine-file archive.
Production uses the official artifact action on Linux and does not use macOS tar.

The project-path test server exposes only `web/` at `/pc98-rust/`; root asset URLs
cannot fall back to a second copy of the app. Server responses confirmed successful
requests for `/pc98-rust/core.wasm`, `app.js`, `style.css` and both demo media.
Browser assertions check real WebAssembly memory, guest-owned score/move/win
state, DOM keyboard input, changed canvas pixels, FDC reads/writes, guest PCM/Web
Audio, pause/reset, local disk export/reinsertion, the on-screen keyboard, and
visible failure for missing/corrupt Wasm. This reuses the existing acceptance
suite rather than introducing a second emulator or mock demo.

PRs receive these build/packaging/browser checks through `emulator.yml`, with only
`contents: read`. `pages.yml` triggers on pushes to `main` and manual dispatch;
both of its jobs additionally require `refs/heads/main`. Only its deploy job has
Pages/OIDC write permissions. Its environment has no manual approval gate, and
concurrency serializes live deployments. Unmerged PR code is never deployed.

## Required post-merge operational follow-up: pending

Publication is **unverified**. After merge, confirm that the `main` push triggers
the Pages workflow, inspect the resulting run and verify the public project URL
retrieves the real Wasm/media and supports gameplay, keyboard and audio. If the
workflow, Pages enablement, public HTTP access or runtime behavior fails, create a
new `task`-labeled Issue with the run URL, error and reproduction and address it
independently. That operational follow-up is not a prerequisite for review
approval, Issue closure or merge. This node did not push, open a PR, merge, comment
or deploy production code.
