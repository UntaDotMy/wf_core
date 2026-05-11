# Native Review And Git Workflow

`wf_core` provides Rust-native closeout surfaces so agents do not recreate
review, commit, or PR policy with ad hoc shell snippets.

## Review Gates

```bash
wf-core review pre-commit --repo-root .
wf-core review pre-pr --repo-root . --format markdown
wf-core review gates check --repo-root . --format compact
wf-core review hosted check --repo-root . --out-dir wf-core-review-artifacts
```

The review gate checks repository access, changed files, untracked-file
warnings, and `git diff --check`. Use `--repo-test-policy required` to add
`cargo test --locked` when a Cargo project is present.

## Git Workflow

```bash
wf-core git-workflow commit-message --repo-root . --test-result "cargo test --locked passed"
wf-core git-workflow pr-body --repo-root . --test-result "cargo test --locked passed"
wf-core git-workflow lint-message message.txt
wf-core git-workflow preflight --repo-root . --base-ref origin/main
```

`preflight` blocks detached or unsafe branch names, missing base refs, dirty
working trees, empty diffs, and whitespace/conflict-marker errors from
`git diff --check`.
