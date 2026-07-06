# `rustok-channel` as an experimental core platform module

- Status: Accepted
- Date: 2026-03-25

## Context

The platform needs a unified platform-level context that answers the question of where external experience is published and read from: website, application, API client, or another external target.

Keeping this logic inside `apps/server` is inconvenient:

- the server should remain a thin composition root;
- channel context is needed not only for commerce, but also for blog/forum/pages and other modules;
- without a separate platform-level layer, it is unclear where to store channel metadata, target binding, and connections to external applications.

At the same time, the precise target model is not yet settled. We understand the direction, but do not want to prematurely cement a complex final architecture.

## Decision

A new module `rustok-channel` is introduced with the following properties:

- the module has `Core` status in the platform taxonomy;
- the module is simultaneously marked as `experimental` in terms of solution maturity;
- domain logic, storage, services, and documentation live in `crates/rustok-channel`;
- `apps/server` knows the module, registers it, and uses it for wiring/runtime resolution, but does not own its domain logic;
- UI, if and when it appears, should live alongside the module according to the general module-owned UI rule.

The first v0 scope is limited to a minimal model:

- `channels`
- `channel_targets`
- `channel_module_bindings`
- `channel_oauth_apps`

The connection to external applications is built through the existing OAuth/app subsystem. `rustok-channel` does not introduce its own parallel token system.

## Why this is `Core`

`Channel` is not an application-level feature like blog/forum/commerce. It is a platform context layer that defines:

- the external access target;
- enabled module surfaces;
- the channel's connection to external applications;
- the basic runtime context for server wiring.

For this reason, the module must always be present in the platform and must not be tenant-toggle'd like an optional domain module.

## Why this is `experimental`

We are deliberately launching this layer as a working prototype:

- the final model is not considered stabilized;
- backward compatibility of the v0 structure is not guaranteed;
- subsequent restructuring of storage/service contracts after real-world usage is allowed.

`Experimental` in this context describes the maturity of the solution, not optional status.

## Consequences

Positive:

- a single point for platform-level channel context emerges;
- the server remains thin;
- blog/forum/pages/commerce can gradually become channel-aware without local ad-hoc solutions;
- UI policy remains consistent: UI lives with the module, if it is needed at all.

Negative and limitations:

- the first version may require data and API migrations after real-world usage;
- some terminology and relationships will still be refined;
- v0 deliberately does not solve the entire scope of omnichannel/presence/integration orchestration.
