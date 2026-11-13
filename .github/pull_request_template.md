<!-- Write the title and body in English. Follow the development flow:
https://github.com/takahirox/pc98-rust/blob/main/docs/development-flow.md -->

## Problem and related Issues

Describe the source problem and link the related Issues. Use and preserve
`Closes #NUMBER` when this PR fully implements that Issue and satisfies mandatory
pre-merge acceptance, even if required post-merge-only checks are pending. For
intentional partial work, state the remaining scope and use a non-closing reference.

## Outcome

Describe the resulting behavior, including before/after examples when useful.

## Focused changes

Summarize the changes and why they are needed to solve the Issue.

## Validation

Record checks actually performed and their results against the pre-merge acceptance
criteria. Report failures, skipped checks, and unperformed checks with reasons.
Keep optional validation distinct. Choose checks appropriate to the changed files
and existing tooling; see the
[current validation guidance](../docs/development-flow.md#validation-for-the-current-repository).

## Required post-merge verification

State None unless the Issue requires it. List necessary checks possible only after
merge as pending until performed; do not claim deployment or publication succeeded
based on pre-merge validation.
Follow the [merge and verification lifecycle](../docs/development-flow.md#5-merge-and-report-verification)
for evidence recording after merge and Issue closure, and linked Issues for actual
defects. Pending post-merge-only verification must not cause a review/publication loop.
