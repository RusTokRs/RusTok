# FORUM-20AP initially non-public topic descriptor materialization

Status: source-ready / unvalidated

## Scope

`FORUM-20AP` closes the topic-created descriptor residual left by the richer
Forum notification visibility composition. A `forum.topic.created` event may now
materialize a semantic descriptor for an active topic that is already non-public
when the host publishes the exact Forum notification recipient-context capability.

The descriptor remains deliberately minimal. It contains the topic target plus
`topic_id` and `category_id` template fields. It does not materialize the topic
title, body, route, audience policy, recipient identity, or owner-only state.

## Authorization boundary

Descriptor creation is not recipient authorization. The topic-created audience
resolver still pages the exact bounded category subscription rows and re-resolves
each candidate recipient through the host recipient-context capability. Current
category and topic audience policy is evaluated for that exact recipient before a
candidate is returned. The author exclusion and raw-subscription cursor semantics
remain unchanged.

When the host does not publish recipient context, descriptor creation retains the
historical public-only check. Missing, foreign, closed, deleted, or otherwise
inactive topics remain absent without an existence oracle.

## Compatibility

This slice changes no notification owner storage, candidate policy, inbox state,
transport, migration, dependency, delivery channel, or open-authorization
contract. Existing public topics retain the same descriptor shape, and stale
descriptors remain subject to current visibility reauthorization during audience
resolution and notification open.

Suggested maintainer validation commands are recorded in the machine contract.
Tests, Cargo commands, formatting, verifiers, and repository CI validation were
not run by the implementation agent.
