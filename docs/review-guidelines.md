# Review and merge guidelines

Contributors and AI reviewers should follow the [development flow](development-flow.md),
including the [English collaboration convention](development-flow.md#collaboration-language).

## Review against the source Issue

Read the source Issue, its acceptance criteria, the PR description, the complete
diff, and the validation evidence. The central review question is:

> Does the PR completely solve the source Issue, without unnecessary scope or complexity?

Check both directions:

- **No missing work:** required behavior and acceptance criteria the PR claims to
  resolve are implemented and appropriately validated. Do not approve a PR as
  closing an Issue while requirements remain unresolved. An intentionally partial
  PR must say so and leave the Issue open.
- **No unnecessary work:** reject unnecessary abstractions, speculative extensibility,
  unrelated refactoring, frameworks/subsystems, or extra policies/configuration
  without a demonstrated need tied to the Issue.

Avoid overengineering. Prefer the smallest coherent design that fully satisfies
the Issue. Added complexity is not automatically beneficial. Preserve justified
existing conventions, and do not use simplicity to excuse omitted requirements
or necessary validation.

## Check the result and validation

Review applicable interface changes against the [UI/UX principles](../README.md#uiux).

Verify correctness, consistency with the repository, relevant documentation, and
evidence for mandatory pre-merge acceptance. Select checks appropriate to the
changed files and actual tooling. Emulator changes require the
[native, Wasm and browser checks](../README.md#reproduce-the-checks), including
preparing the legal guest fixtures. A templates/documentation-only change needs Markdown/template
structure,
links, whitespace, and consistency checks, not unrelated runtime suites or new
tooling. See [current validation guidance](development-flow.md#validation-for-the-current-repository).

Apply the [test and CI performance principles](development-flow.md#test-and-ci-performance-principles):
review the additional defect coverage, duplication and execution/maintenance cost
of new or expanded checks. Ask:

> Does this change provide sufficient confidence with the lightest appropriate validation, without redundant execution?

Check that the PR accurately reports checks actually performed and their results,
including failures, skipped checks, and unperformed checks with reasons. Optional
validation is separate from mandatory completion criteria.

Mandatory pre-merge acceptance must be achievable and verifiable before merge.
Checks possible only after merge, such as merge-triggered deployment and
published-site verification, must not block pre-merge approval. Record genuinely
necessary post-merge work separately, or state None. Report it as pending until
performed; pre-merge evidence does not prove deployment or publication succeeded.
This distinction does not waive required implementation or applicable pre-merge checks.

## Choose an actionable review outcome

- **Agent-fixable findings:** request changes. Identify each finding, its location,
  the unmet requirement or concrete problem, and the expected correction or
  validation. Review the updated PR again after revision.
- **Necessary human input:** record the specific question, why human confirmation
  is necessary, and what answer or evidence is expected. Mark the review as waiting
  for human input and use the workflow's wait path. If no automated wait path is
  available, pause and report the pending question to the operator. Resume review
  when the answer arrives; do not repeatedly request code fixes for missing human
  input. Distinguish this state from agent-fixable findings, and continue independent
  review or fixes where possible. Do not add human approval requirements without
  an actual need tied to the Issue.
- **Ready to merge:** approve and merge when the implementation fully solves the
  Issue it claims to close, mandatory pre-merge checks are sufficient and pass, and
  no blocking review findings remain. Follow repository permissions and protections.

Record nonblocking suggestions as optional. Required checks possible only after
merge do not prevent approval of an otherwise complete PR. After merge, perform
any required verification and report actual results, failures, and needed follow-up;
keep unperformed checks pending. No post-merge verification is required for the
templates/documentation-only work in Issue #1.
