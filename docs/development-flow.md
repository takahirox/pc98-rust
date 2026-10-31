# Development flow

The default flow for pc98-rust is Issue → implementation → PR → review → revision
as needed → merge. Perform post-merge verification only when required by the Issue.
The default branch is `main`.

## Collaboration language

Use **English** for newly authored templates, development/review/merge documentation,
AI instructions, Issue and PR titles and bodies, review comments, and commit
messages. This collaboration convention does not require translating historical
content or changing product localization.

## 1. Start with an Issue

Use the [Issue template](../.github/ISSUE_TEMPLATE/issue.md) to capture the problem,
expected outcome, relevant context, reproduction steps where applicable, and
concrete completion criteria. No assignee or owner field is required.

Use the source Issue to define scope. Clarify genuinely blocking ambiguity before
dependent implementation; do not invent requirements.

Default to completion criteria an AI agent can execute and verify. Require human
confirmation, physical-device testing, subjective evaluation, or external approval
only when necessary for the actual Issue. Explain why and what result is expected.

Separate these three categories:

- **Pre-merge acceptance:** mandatory criteria achievable and verifiable before merge.
- **Optional validation:** useful additional checks that do not block completion.
- **Required post-merge verification:** genuinely necessary checks possible only after
  merge. State None when there are none; do not add them routinely.

Checks possible only after merge must not be pre-merge approval conditions. For
example, a merge-triggered deployment or published-site check belongs after merge.
This distinction does not waive implementation requirements or applicable checks
that can be performed before merge.

## 2. Implement the smallest complete change

Inspect the current repository and preserve useful existing content and justified
conventions. Prefer the smallest coherent design that fully satisfies the Issue.
Avoid unnecessary abstractions, speculative extensibility, unrelated refactoring,
frameworks or subsystems, and extra policies or configuration without a demonstrated
need tied to the Issue. Added complexity is not automatically beneficial; simplicity
does not excuse omitted requirements or necessary validation.

### Validation for the current repository

The emulator application has native Rust tests, reproducible guest fixtures, a
Wasm build, and automated browser tests. For emulator or guest changes, follow the
commands in [README](../README.md#reproduce-the-checks), including legal FreeDOS
fixture preparation and the actual browser artifact tests. Report the exact
checks performed and their outcomes. No deployed website or manual playtest is
needed for this application.

For documentation-only work, do not add application dependencies, CI or tooling
merely to validate a process. There is no configured Markdown lint command.

Choose checks appropriate to the changed files and actual repository tooling.
Apply the [test and CI performance principles](#test-and-ci-performance-principles)
when choosing or changing validation.
For templates and documentation, inspect Markdown structure and Issue template
front matter, verify relative links and heading anchors, and check whitespace with
`git diff --check` (or `git diff --cached --check` for staged changes). Inspect newly
created files too, since unstaged Git diffs omit untracked files. Use available
Markdown tooling if present and check that the templates and guidance agree.

A templates/documentation-only change requires no new runtime tests, hardware/device
checks, or deployed-site verification. For future changes, inspect the tooling then
available and run applicable checks. Report actual results and limitations.

### Test and CI performance principles

**Detect the required failures at the lightest appropriate layer, without redundant
execution.** Preserve required correctness, regressions and meaningful integration
coverage when redistributing checks. Changes to tests, workflows or build tooling
belong in separately scoped implementation work; these principles add no merge gate.

1. **Prefer the lightest meaningful test layer.** Use fast native Rust unit or
   component tests for calculations, branches and broad input combinations, and
   Python unit tests for fixture-fetch logic. Use guest boot tests for CPU/firmware
   interactions and actual release-Wasm Chromium tests for browser controls,
   rendering, media transfer and serving-path boundaries. Add assertions that
   catch a concrete defect, rather than mirror the implementation.
2. **Choose combinations deliberately.** Select representative disk formats,
   VM/Ap profiles, CPU/device states and cold/reset cases covering distinct paths;
   exhaust combinations where their interactions require it. The local-root and
   `/pc98-rust/` Playwright projects check different asset-resolution boundaries.
   Preserve these and required guest regressions; do not automatically multiply
   every new case across every profile, format and path.
3. **Match validation to the change.** Follow the
   [current validation guidance](#validation-for-the-current-repository) and
   [README checks](../README.md#reproduce-the-checks): documentation needs structure,
   links and whitespace checks; browser UI changes need applicable actual-Wasm
   browser checks; emulator/guest changes retain native, Wasm and browser validation
   with legal fixtures. CI/build changes need validation of affected build,
   packaging and workflow behavior. Use focused checks for fast feedback, then
   applicable full regression checks before merge. Keep optional validation
   separate; only genuinely post-merge-only checks belong after merge, and only
   when the Issue requires them. The acceptance workflow currently runs on PRs
   and `main` pushes without path filters; the separate Pages workflow builds and
   publishes on `main` pushes or manual dispatch on `main`. Local checks do not
   prove publication, and this guidance does not change workflow triggers.
4. **Reuse work and eliminate duplicate execution.** Reuse builds, prepared
   fixtures and checksum-verified downloads with keys tied to relevant sources,
   toolchains and dependencies, while isolating mutable disks and test state.
   Existing Actions setup caches npm downloads; `npm test` already runs the build
   and packaging check before Playwright, so avoid an equivalent extra build
   without a reason. When changing CI, avoid equivalent push/PR runs for the same
   revision, cancel superseded acceptance runs when safe, and preserve necessary
   deployment ordering (Pages is currently serialized without cancellation).
   Measure resource use when parallelizing independent work; more browser workers
   need not improve feedback time.
5. **Review test value and added cost together.** Identify the additional defect
   each new or expanded test detects, whether existing coverage already provides
   that assurance, and its execution and maintenance cost. Watch repeated fresh
   compiler/browser setup, large disk transfers, fixture preparation, long guest
   loops and Cartesian-product growth. The
   [HDD export investigation](browser-acceptance-validation.md) illustrates reducing
   automation transfer cost while retaining every full-image equality assertion.
6. **Maintain an execution-time budget.** No normal-CI baseline or target has been
   established. The linked HDD investigation records historical Ubuntu browser
   runs and local native/browser timings with environment details; they are
   evidence, not a current whole-CI target. During actual test/CI performance work,
   establish a representative baseline and target from comparable runs, monitor
   growth and investigate overruns. Record elapsed feedback time, total runner/
   computation time and queue time separately. Revisit test structure and duplicate
   work while retaining meaningful assertions and required coverage. Existing job
   timeouts are safety limits, not performance targets; no new benchmark, hardware
   purchase or timing gate is required by this documentation.

## 3. Open a PR for the Issue

Use the [PR template](../.github/pull_request_template.md). Explain the problem,
resulting behavior, focused changes and their purpose, related Issues, and validation
actually performed. Accurately report failures, skipped checks, and unperformed
checks with reasons. Do not require unrelated runtime suites for documentation.

Only claim to close an Issue when the PR fully addresses it. State intentionally
partial scope clearly and leave that Issue open. Record any required post-merge
checks separately as pending until performed; these checks do not make an otherwise
complete implementation partial.

## 4. Review and revise

Follow the [review guidelines](review-guidelines.md). Ask:

> Does the PR completely solve the source Issue, without unnecessary scope or complexity?

Check both missing requirements and unjustified scope or complexity. Request fixes
for agent-fixable findings, revise, and review the updated PR again. When necessary
human input is missing, record the specific question and use the workflow's wait
path for the answer, as described in the review guidelines.

## 5. Merge and report verification

Approve and merge when the implementation and mandatory pre-merge checks are
sufficient and no blocking review findings remain. Follow applicable repository
permissions and branch protections.

If the Issue requires post-merge verification, perform it after merge and report
the actual results, failures, and follow-up needed. Keep it pending until performed.
Pre-merge validation does not prove deployment or publication succeeded. No
post-merge verification is required for the collaboration templates and guidance
introduced in Issue #1.
