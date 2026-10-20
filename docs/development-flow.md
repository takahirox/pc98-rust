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

There are currently no application files, build/test commands, CI workflows, or
configured Markdown lint commands. Do not invent commands or add application code,
dependencies, CI, or tooling merely to document or validate a process.

Choose checks appropriate to the changed files and actual repository tooling.
For templates and documentation, inspect Markdown structure and Issue template
front matter, verify relative links and heading anchors, and check whitespace with
`git diff --check` (or `git diff --cached --check` for staged changes). Inspect newly
created files too, since unstaged Git diffs omit untracked files. Use available
Markdown tooling if present and check that the templates and guidance agree.

A templates/documentation-only change requires no new runtime tests, hardware/device
checks, or deployed-site verification. For future changes, inspect the tooling then
available and run applicable checks. Report actual results and limitations.

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
