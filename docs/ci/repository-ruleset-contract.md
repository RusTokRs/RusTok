# Repository ruleset contract

The `main` branch must have an active GitHub ruleset whose `required_status_checks` rule contains both exact contexts:

- `Migration harness approval`;
- `Repository ruleset contract`.

Both checks must:

- originate from the GitHub Actions application (`integration_id: 15368`), not from `any source`;
- be published on the latest PR head SHA by base-owned evaluators;
- use strict required-status-check policy so a pull request is tested against the current base branch;
- set `do_not_enforce_on_create` to `false`;
- target `refs/heads/main` through an active repository or organization ruleset.

The `pull_request_target` evaluators run against trusted base policy. They do not rely on their workflow job statuses as merge gates because those workflow contexts are based on the base revision. Instead:

- the migration evaluator compares protected files as untrusted data and creates `Migration harness approval` directly on the PR head SHA;
- the ruleset evaluator reads active branch rules through the GitHub Rules API and creates `Repository ruleset contract` directly on the PR head SHA.

Each evaluator then fails its own job when its decision is negative. The required head-SHA checks prevent a PR from merging when migration infrastructure lacks approval or repository settings drift from the checked contract.

The machine-readable source of truth is [`repository-ruleset-contract.json`](repository-ruleset-contract.json). The exact activation payload is [`repository-ruleset-admin-payload.json`](repository-ruleset-admin-payload.json). The live audit is implemented by `.github/workflows/repository-ruleset-audit.yml` and `scripts/verify/verify-repository-ruleset-contract.mjs`.

## Administrative setup

1. Open repository or organization rulesets and create an active branch ruleset targeting `main`.
2. Enable **Require status checks to pass before merging**.
3. Add `Migration harness approval` and `Repository ruleset contract` after both base-owned evaluators have published them at least once.
4. Select GitHub Actions as the expected source for both checks rather than `any source`.
5. Enable **Require branches to be up to date before merging**.
6. Do not enable the option that skips required checks when the branch is created.
7. Avoid permanent bypass actors. Any temporary bypass must be time-bounded and reviewed separately because the public active-rules endpoint does not expose bypass actors.

The `Repository Ruleset Contract` workflow runs on pull requests targeting `main`, pushes to `main`, a daily schedule, and manual dispatch. It checks the active rules returned by GitHub's branch rules API and fails closed on missing, duplicated, loose, source-unbound, malformed, or symlinked policy data.

## Stronger organization-level option

GitHub Enterprise Cloud organizations should additionally use **Require workflows to pass before merging** and bind the base-owned migration and ruleset evaluator workflows from `main`. A required workflow is stronger than a status-context-only rule because required status check names do not identify a specific workflow file or trigger. The repository-level two-check contract remains the minimum portable baseline.

<!-- smoke-only protected change; this branch must never merge -->
