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
- register source providers through `ModuleRuntimeExtensions` without making a
  producer depend on the notifications module.

## Safety boundaries

Template data is a bounded string map and cannot contain arbitrary JSON or HTML.
Audience pages contain at most 256 unique recipient UUIDs. Target routes are
returned only after owner authorization and must be safe root-relative paths.
Provider errors expose stable retryability without leaking private source data.

## Entry points

- `NotificationSourceProvider`
- `NotificationSourceRegistry`
- `register_notification_source_provider`
- `NotificationSemanticDescriptor`
- `NotificationAudiencePage`
- `NotificationOpenAuthorization`

The contract does not synchronously participate in producer transactions.
Producers commit their owner state and semantic outbox event whether or not a
notifications consumer is installed.
