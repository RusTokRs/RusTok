# Ruleset activation request

## Objective

Activate the repository ruleset in `docs/ci/repository-ruleset-admin-payload.json` after the current direct-to-`main` implementation series is complete.

## Owner action

Use the repository Rulesets UI or the GitHub REST `POST /repos/RusTokRs/RusTok/rulesets` endpoint with the checked payload. Do not alter the required context, integration source, strict policy, PR requirement, force-push/deletion rules, review count or bypass actor list during activation.

## Acceptance

- A normal pull request receives a successful `Migration harness approval` Check Run on its head SHA.
- A protected migration infrastructure change fails until `migration-infra-approved` is applied.
- `Repository Ruleset Contract` passes against the active rules for `main`.
- Direct pushes to `main`, force pushes and branch deletion are rejected after cutover.
- No permanent bypass actor is configured.

## Rollout source

Follow `docs/ci/main-protection-rollout.md` exactly, including the positive and negative test pull requests before activation.
