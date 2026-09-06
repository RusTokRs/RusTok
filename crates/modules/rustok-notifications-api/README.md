# `rustok-notifications-api`

## Purpose

`rustok-notifications-api` is the neutral cross-module contract between semantic
event owners and the notifications owner. It contains no notification
persistence, delivery provider SDK, contact data, source database model, or UI.

## Responsibilities

- identify notification source modules and semantic notification types with
  bounded validated keys;
- describe recipient-neutral notification meaning and target identity;
- resolve candidate recipients in bounded cursor pages;
- authorize target opening for one tenant and recipient;
- let source modules register deferred provider factories through
  `ModuleRuntimeExtensions` without depending on the notifications owner;
- materialize those factories only after the executable host has a neutral
  `HostRuntimeContext`.

The `server` feature explicitly enables `rustok-api/runtime`; notification
contracts do not rely on unrelated host crates to activate that dependency.

## Safety boundaries

Template data is a bounded string map and cannot contain arbitrary JSON or HTML.
Audience pages contain at most 256 unique recipient UUIDs and hide their
invariant-bearing collection behind accessors. Source revisions must be positive.
Target routes are validated internal paths with an optional strictly bounded
query; external URLs, fragments, traversal, percent encoding, whitespace, and
malformed parameters fail closed.

The neutral API never exposes SeaORM connections or source persistence models.
Factory/provider duplicate slugs, factory/provider identity mismatches, and
factory construction failures are explicit errors. Provider errors expose stable
retryability without leaking private source data.

## Entry points

- `NotificationSourceProvider`
- `NotificationSourceProviderFactory`
- `NotificationSourceRegistry`
- `NotificationSourceFactoryRegistry`
- `register_notification_source_provider_factory`
- `materialize_notification_source_registry`
- `NotificationSemanticDescriptor`
- `NotificationAudiencePage`
- `NotificationOpenAuthorization`

The contract does not synchronously participate in producer transactions.
Producers commit their owner state and semantic outbox event whether or not a
notifications consumer is compiled or tenant-enabled.
