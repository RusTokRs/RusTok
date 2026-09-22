# Fly build-versus-adopt spike: `<capability>`

## Problem

State the editor capability and the compatibility or browser constraint it must satisfy.

## Non-negotiable boundaries

- The dependency must not become the canonical project model.
- `fly` and `fly-ui` must remain free of UI-framework and RusTok dependencies.
- Backend sanitization policy remains authoritative.
- Unknown GrapesJS/provider data must remain lossless.
- Rich-text behaviour stays outside Fly.

## Candidate matrix

| Candidate | API fit | Native | WASM | Licence | Maintenance | Security | Transitives | Bundle impact | Exit cost |
|---|---:|---:|---:|---|---|---|---:|---:|---:|
| Local implementation |  |  |  |  |  |  |  |  |  |
| Candidate A |  |  |  |  |  |  |  |  |  |
| Candidate B |  |  |  |  |  |  |  |  |  |

## Spike implementation

Document the smallest experiment, fixture set, target matrix, and measurable acceptance criteria.

## Findings

Include API limitations, unsupported syntax, memory/lifecycle behaviour, bundle measurements,
security observations, and replacement implications.

## Decision

Build | Adopt | Defer | Reject

Explain why and link the matching dependency record when adopting a crate.
