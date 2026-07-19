# Main protection rollout

This checklist is the final administrative handoff for moving RusTok from direct-to-`main` implementation to protected pull-request delivery.

## Preconditions

- The `Migration Infrastructure Approval` workflow has run on at least one pull request and published the `Migration harness approval` Check Run on that pull request's head SHA.
- `Repository Ruleset Contract` and `Hardening Gates` are green for the same repository state.
- Emergency access and ownership are documented before removing direct pushes.

## Ruleset configuration

Create an active branch ruleset targeting `refs/heads/main` with:

1. Require a pull request before merging.
2. Require status checks to pass before merging.
3. Require branches to be up to date before merging.
4. Require `Migration harness approval` from GitHub Actions (`integration_id: 15368`).
5. Set `do_not_enforce_on_create` to `false`.
6. Block force pushes and branch deletion.
7. Require conversation resolution before merging.
8. Do not configure permanent bypass actors. Emergency bypass must be temporary, attributable and reviewed.

## Cutover

1. Finish the currently authorized direct-to-`main` implementation series.
2. Open a test pull request that does not change migration infrastructure and confirm a successful head-SHA `Migration harness approval` Check Run.
3. Open a second test pull request that changes a protected migration file without the approval label and confirm the Check Run fails.
4. Apply `migration-infra-approved` and confirm both the base-owned evaluator and the PR sandbox preflight rerun successfully.
5. Enable the active ruleset.
6. Rerun `Repository Ruleset Contract` manually and confirm the live API audit passes.
7. Make pull requests the only normal delivery path from that point onward.

## Rollback

If the ruleset blocks all recovery paths:

1. Use a time-bounded organization or repository owner bypass.
2. Correct the ruleset or required-check source binding.
3. Rerun the live audit.
4. Remove the temporary bypass immediately.
5. Record the incident and remediation in the repository issue that tracks this rollout.
