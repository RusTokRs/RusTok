---
id: doc://docs/research/devmesh-concept.md
kind: architecture_research
language: markdown
status: proposed
source_language: markdown
---

# DevMesh: AI-Native Collaborative Network

## Status and Scope

This document is a proposed product and architecture direction for a DevMesh
deployment of RusToK. It does not introduce a runtime module, change ownership,
or replace an accepted architecture decision. A future implementation must first
create an ADR for the participant model, provenance visibility policy, and the
settlement boundary.

## Reading Guide

The document is ordered from product model to implementation constraints:

1. the product thesis and community model;
2. the RusToK capabilities to reuse;
3. the target owner boundaries and their interaction rules;
4. execution, provenance, event, and security constraints;
5. a sequenced delivery plan and the first end-to-end scenario.

DevMesh is a collaborative developer network in which people and AI agents can
participate in communities, perform work, build a verifiable reputation, and
share constrained resources. It combines community interaction, an intelligence
marketplace, and resource commons without collapsing their data or policy into
one module.

## Product Thesis

> An AI agent is a first-class participant in a developer community, but it is
> not a user account, a model-provider configuration, or an unbounded service
> principal.

A human and an AI agent can have a public profile, relationships, memberships,
work history, and reputation. Their authority is deliberately different:

- `rustok-auth` owns human credentials and sessions;
- the public participant domain owns the identity visible to the network;
- an agent registry owns what an agent offers and who operates it;
- `rustok-ai` owns provider-backed execution, tool policy, approvals, and
  private runtime traces;
- work, settlement, provenance, reputation, and resources remain separate
  domain owners.

The platform must reward trustworthy outcomes, not opaque model output or raw
activity volume.

## Community Model

DevMesh is not limited to software development. It is a network for communities
that learn, make, research, create, collaborate, or coordinate useful local
activity. Development remains a flagship domain because RusToK already has
strong forum, repository, workflow, and AI foundations, but it is not the
platform's only thematic boundary.

### Taxonomy Instead of Fixed Community Types

A community is a `rustok-groups` group with a primary theme and optional
secondary tags from `rustok-taxonomy`. Themes are data and can evolve without a
new module, migration, or enum change. The group owner continues to own
membership and governance; taxonomy only provides controlled vocabulary and
discovery metadata.

Each community is described by independent dimensions:

| Dimension | Examples | Owner boundary |
|---|---|---|
| Theme | `technology`, `music`, `biology`, `local_community` | Taxonomy terms attached to the Group-owned community |
| Format | `discussion`, `learning_circle`, `project`, `marketplace`, `resource_pool`, `gathering` | Group settings and referenced owner surfaces; not a new group type per format |
| Governance | `public`, `curated`, `private`, `verified_professional` | `rustok-groups` membership, invitation, application, and access policy |
| Scope | global, language-based, regional, organization-based, project-based | Group policy plus existing locale/region and participant references |

One community has one primary theme for navigation, but it may carry multiple
secondary terms. For example, a community can be primarily `robotics` and also
tagged `education`, `open_source`, `hardware`, and `Minsk`. A project community
can span themes without becoming a special hard-coded category.

### Initial Theme Catalogue

The first navigation catalogue should be curated, while the underlying taxonomy
remains extensible:

| Theme family | Examples of communities and activity |
|---|---|
| Technology and Engineering | Software, AI, cybersecurity, robotics, electronics, open source, and hardware projects |
| Design and Creative Practice | UX/UI, illustration, music, film, writing, photography, and game art |
| Science and Research | Mathematics, biology, climate, citizen science, paper discussion, and reproducible research |
| Education and Learning | Language practice, tutoring, study circles, curricula, and exam preparation |
| Business and Professional Practice | Entrepreneurship, product, marketing, operations, sales, and independent professional communities |
| Making and Local Production | 3D printing, repair, fabrication, crafts, agriculture, and community workshops |
| Games and Interactive Worlds | Game jams, modding, tabletop games, esports, and interactive storytelling |
| Culture and Media | Books, cinema, history, journalism, translation, and cultural projects |
| Civic and Local Communities | Neighbourhood initiatives, volunteering, non-profits, mutual aid, and city projects |
| Lifestyle and Hobbies | Travel, cooking, sport, gardening, personal photography, and other interest groups |

This catalogue is a discoverability starting point, not a restriction on valid
communities. New terms follow the Taxonomy owner's normal vocabulary and
moderation policy rather than being encoded into application logic.

### Agent Participation by Theme

Agents publish specializations through `rustok-agent-registry` tags and declare
their supported task contracts. A community then decides whether agents may
observe, answer on mention, publish with review, execute work, or be excluded.
An agent's specialization never grants community access by itself.

High-stakes themes require stricter policy. Medical, legal, financial, safety,
and crisis-support communities may allow research assistance, source
summarization, or administrative automation, but must not represent an agent as
an autonomous licensed professional. Their group policy can require verified
human review, restricted tool grants, explicit disclaimers, or prohibit agent
execution entirely.

## Existing RusToK Foundation

DevMesh should reuse the platform's existing boundaries rather than duplicate
them:

| Existing owner | DevMesh role | Boundary that must remain intact |
|---|---|---|
| `rustok-auth` | Human login, OAuth, credentials, and session lifecycle | An agent is not represented as a human user merely to obtain credentials. |
| `rustok-profiles` | Public profile presentation | Its user-only source model must become participant-aware before it is used for agent cards. |
| `rustok-social-graph` | Block, mute, follow, endorsement, and collaboration relationships | The current `user_id -> user_id` storage must be replaced atomically with participant references before agent relations exist. |
| `rustok-groups` | Community membership, invitations, access policy, and governance | Community policy remains the Groups owner's responsibility. |
| `rustok-forum`, `rustok-blog`, `rustok-comments` | Discussions, knowledge, and interaction surfaces | These modules own their own content and moderation-visible state. |
| `rustok-ai` | AI provider runtime, agent principals, model bindings, approvals, MCP/direct execution, and private traces | It is deployment-scoped and must not become the social or financial owner of DevMesh. |
| `rustok-workflow` and `alloy` | Durable automation and controlled script execution | They execute declared workflows; they do not own marketplace contracts or reputation. |
| Marketplace Family | A proven pattern for split owner modules and financial orchestration | Seller/listing semantics must not be reused as task, agent, or reputation records. |
| `rustok-moderation` | Reports, cases, decisions, appeals, and cross-domain application orchestration | Moderation never writes another owner's tables directly. |
| `rustok-outbox` and `rustok-events` | Transactional event publication and canonical event contracts | Delivery is not a public audit log or reputation system. |
| `rustok-index`, `rustok-search`, `rustok-seo` | Generic indexing, product search/ranking, and discovery | They consume owner-provided records/events; they do not read source tables. |
| `rustok-media` | Asset and artifact lifecycle | Work outputs and stream assets use owner-local references to Media rather than duplicate blobs. |

## Target Architecture

The target is a set of small owner modules plus family roots that orchestrate
through typed ports. A host application only composes module-owned transport and
UI surfaces.

```text
Auth user ---- operates ----> Participant <---- published by ---- Agent registry
                                  |                         |
                         profile / social graph       capabilities / terms
                                  |                         |
                                  +----- Work engagement ---+
                                               |
                    +--------------------------+--------------------------+
                    |                          |                          |
             Agent execution             Settlement                 Provenance
                    |                          |                          |
               rustok-ai                payment/payout           reputation/feed
```

### Participant and Identity Family

| Proposed owner | Responsibility | Does not own |
|---|---|---|
| `rustok-participants` | A tenant-scoped public participant with a stable `participant_id`, kind (`human`, `agent`, later `organization`), visibility, lifecycle status, and operator/owner reference | Passwords, provider keys, model configuration, or domain content |
| `rustok-profiles` | Localized public presentation, tags, summaries, and profile cards for a participant | Authentication or agent execution state |
| `rustok-agent-registry` | Agent manifest, capabilities, supported task contracts, version, operator, availability, commercial terms, and attestations | Model-provider credentials, tool traces, or task results |
| `rustok-social-graph` | Typed participant-to-participant relations: block, mute, follow, endorse, collaborate | Recommendations, scores, or another module's membership rows |
| `rustok-reputation` | Immutable reputation facts and versioned, explainable score projections | A mutable score field written directly by product flows |

`rustok-participants` is the missing identity boundary. It must not be called
`rustok-ai-identity`, because it is useful for people, agents, and future
organizations. Existing user-only profile and social-graph contracts should be
replaced atomically where DevMesh requires generic participant references; no
parallel legacy relationship path should be kept.

An agent has two distinct identities:

1. its public `Participant`, which can publish, be followed, and earn a record;
2. its execution binding, represented by `rustok-ai::AgentPrincipal` or an
   external agent adapter, which is private operational configuration.

The public participant may reference a registry version, but must never expose
secret references, raw prompts, private tool calls, or provider configuration.

### Community Family

Communities do not need a new all-purpose DevMesh module. The existing modules
form the community layer when they consume participant-aware presentation and
access ports:

- `rustok-groups` owns micro-community membership, invitation, application, and
  governance policy;
- `rustok-forum` owns structured questions, discussions, answers, and threads;
- `rustok-blog` owns articles, tutorials, and long-form publication;
- `rustok-comments` owns opt-in classic comment threads;
- `rustok-notifications` owns delivery preferences and inbox fan-out;
- `rustok-gatherings` is a future owner for hackathons, code jams, and sessions.

`rustok-gatherings` is intentionally named to avoid confusion with the platform
event-contract and outbox modules. A later `rustok-live` module may own stream
metadata, audience access, and session state; video transport remains an
external-provider or Media boundary.

Code collaboration is not a responsibility of `rustok-content`. A future
`rustok-repositories` owner should represent repository connections, revisions,
and source snapshots, while `rustok-code-review` can own review workflows and
findings. GitHub or GitLab connectors remain adapters behind these public owner
contracts, not the primary data model.

### Work Marketplace Family

The intelligence marketplace must start with a task and engagement domain,
rather than treating an agent as a product SKU or a seller listing.

| Proposed owner | Responsibility |
|---|---|
| `rustok-work` | Help requests, bounties, hires, scopes, deliverables, acceptance, rejection, disputes, and non-financial work lifecycle |
| `rustok-agent-execution` | An agent run inside a declared work engagement: capability grants, leases, idempotency, result references, and execution status |
| `rustok-work-orchestration` | Pipelines, staged work, studios, delegation, aggregation, and root-workflow receipts; owns no stage result persistence |
| `rustok-settlement` | Escrow, holds, revenue splits, payment release, reversals, and payout initiation through typed financial ports |

`rustok-work` supports both unpaid help and paid work. A bounty is simply a work
item with a settlement policy; a help request is a work item with no settlement
policy. This prevents two divergent task implementations.

`rustok-work-orchestration` follows the Marketplace Family pattern: it composes
typed owner ports and uses deterministic child idempotency keys, but owns no
agent result, financial balance, or provider trace table.

### Settlement Boundary

The current Marketplace Family has seller-specific ledger and payout ownership.
DevMesh requires equivalent but not identical financial operations: escrow,
multi-party revenue splits, agent-owner attribution, royalty distribution, and
resource billing. The target architecture should introduce a generic
`rustok-settlement` financial boundary and atomically migrate marketplace
financial consumers to it when its contract is approved.

`rustok-settlement` owns money-denominated holds, postings, allocations,
reversals, settlement status, and payout instructions. It consumes payment
provider facts through typed ports. It does not own the commercial decision to
accept a deliverable, choose a winner, or calculate a reputation score.

Resource credits are not money. A resource pool must not place GPU minutes or
provider quotas in a financial ledger merely because both have balances.

### Resource Commons Family

The former proposed `rustok-ai-resource` contains three independent transaction
boundaries and must be split:

| Proposed owner | Responsibility | Does not own |
|---|---|---|
| `rustok-resource-catalog` | Resource offers, capability descriptors, availability policies, and provider adapters | Pool shares, reservations, or usage postings |
| `rustok-resource-pool` | Contributions, member shares, quotas, reservations, consumption facts, and pool-local policy | Cross-community matching or provider secrets |
| `rustok-resource-broker` | Lending and matching between pools, lease orchestration, and reconciliation through typed ports | Either pool's balance or resource inventory |

The pool stores resource-native units such as GPU-seconds, API requests, or
model-token units. It can use a shared append-only journal library, but its
domain accounting remains independent from financial settlement.

### Provenance, Audit, and Reputation

The phrase "public audit log" must not mean publication of every prompt, source
file, tool argument, or model response. That would leak secrets, personal data,
licensed code, and security-sensitive context.

`rustok-provenance` should own immutable, signed or hash-bound evidence records:

- a participant or execution identity;
- a declared purpose and work reference;
- input/output artifact references or content hashes;
- timestamps, policy version, result classification, and visibility decision;
- redacted public summary where allowed;
- revocation, correction, or appeal linkage without mutation of history.

Private runtime traces remain in `rustok-ai`. Public DevMesh feeds consume only
the permitted provenance projection. `rustok-reputation` consumes normalized
facts such as accepted delivery, upheld moderation decision, verified review, or
successful resource return. Its algorithms must be versioned and each score
explainable as a projection of facts, not an opaque field that a caller updates.

### Discovery and Read Models

No `rustok-ai-graph` module is needed. The target roles are:

- `rustok-social-graph` owns direct relationship state;
- `rustok-reputation` owns trust facts and score projections;
- `rustok-work` and `rustok-provenance` publish outcome facts;
- `rustok-recommendations` is an optional read-model owner that consumes the
  permitted events and exposes explainable recommendations;
- `rustok-index` materializes generic searchable records from owner-provided
  schemas and mutations;
- `rustok-search` owns query-time relevance and ranking;
- `rustok-seo` owns external discovery metadata for public pages.

Recommendation code must not write social relationships or reputation facts. It
may explain that a result was selected because of declared capabilities,
completed work in a community, or an explicit follow relation.

## Execution and Authorization Contract

Every agent execution requires a capability grant. A valid invocation must pass
the intersection of four authorities:

```text
initiating participant
    intersect agent principal policy
    intersect work engagement grant
    intersect resource lease or provider policy
```

The existing `rustok-ai` initiating-subject/agent permission intersection is the
starting point, not the complete DevMesh policy. The final policy check occurs
immediately before a tool call or external action. A run must have a stable root
idempotency key, receipt-first state transitions, bounded lease duration, and a
correlation ID propagated to provenance, settlement, and outbox events.

An external agent is never given direct database access. It receives only a
scoped capability token or approved tool surface. Expiration, revocation,
community policy, moderation enforcement, and tenant enablement must fail
closed.

## Cross-Module Rules

1. Owners publish typed ports and events; they do not read another module's
   tables or share mutable entities.
2. Cross-domain references use typed logical references containing tenant, owner
   module, kind, stable ID, and revision where optimistic correctness matters.
3. Outbox events are at-least-once deliveries. Consumers use idempotency receipts
   and never treat a duplicate event as a second execution, payment, or score.
4. Search, feeds, and recommendations are rebuildable read models. They are not
   sources of truth for access, work status, balances, or trust facts.
5. Moderation retains case and decision ownership. The affected owner applies a
   validated decision through its own adapter and records local enforcement.
6. Every module-owned UI follows the existing FFA/FBA contract: native Leptos
   server functions by default, parallel GraphQL/REST transport where required,
   and host-provided locale.
7. New reusable contracts belong in the appropriate shared crate only after the
   pattern has at least two consumers; domain policy stays in its owner module.

## Event Families

The following event families are illustrative contract names, not a committed
schema registry:

```text
participant.created | participant.visibility_changed | agent.published
relationship.changed | community.membership_changed
work.opened | work.claimed | work.delivery_submitted | work.accepted
execution.started | execution.completed | execution.failed
provenance.recorded | provenance.visibility_changed
reputation.fact_recorded | reputation.projection_rebuilt
settlement.held | settlement.released | settlement.reversed
resource.contributed | resource.reserved | resource.consumed | resource.lease_closed
```

Payloads must contain only the minimum stable public fact. Large artifacts,
private prompts, model output, and provider payloads remain owner-local and are
referenced only when the consumer is authorized.

## What Not To Build

| Rejected shape | Reason | Target instead |
|---|---|---|
| `rustok-ai-identity` | Mixes public social identity with AI runtime configuration | `rustok-participants` plus `rustok-agent-registry` |
| `rustok-ai-graph` | Duplicates a social owner and combines graph writes with recommendation policy | Evolve `rustok-social-graph`; add `rustok-recommendations` as a projection |
| `rustok-ai-invocation` | Duplicates existing `rustok-ai` runtime and conflates execution with public audit/reputation | `rustok-ai`, `rustok-agent-execution`, and `rustok-provenance` |
| One universal `ledger` | Blurs regulated money, work obligations, and resource units | `rustok-settlement` plus resource-native pool accounting |
| A public raw trace stream | Leaks secrets and makes privacy irreversible | Policy-controlled provenance projections with redaction and artifact references |
| Agent as a synthetic human user | Corrupts authentication, privacy, and authority semantics | A distinct participant kind with scoped agent execution authority |
| Task marketplace as commerce product catalog | Forces service work into SKU/listing semantics | `rustok-work` and engagement contracts |

## Delivery Sequence

### Phase 0: Ratify the Boundaries

- Translate and maintain this concept as English-only repository documentation.
- Write ADRs for participant identity, provenance visibility, and settlement
  generalization before implementation.
- Define cross-owner reference, event, permission, and visibility vocabularies.

### Phase 1: Trustworthy Community Presence

- Introduce participant-aware profile and relationship contracts atomically.
- Register an agent with a public manifest and an operator relationship.
- Permit an agent to respond to a forum help request through a scoped execution.
- Publish only a redacted provenance summary and one explainable reputation fact.

### Phase 2: Work Engagements

- Implement work requests, claims, deliverables, review, acceptance, and
  disputes.
- Bind agent execution to a work grant with idempotent leases and receipts.
- Add simple human/agent collaboration and a staged pipeline through the
  orchestration root.

### Phase 3: Settlement and Resource Pools

- Approve and implement generic financial settlement before paid bounties.
- Add escrow-backed bounty acceptance and deterministic split settlement.
- Introduce catalog, pool, reservation, and usage contracts for one resource
  class before cross-community lending.

### Phase 4: Read Models and Scale

- Materialize activity feeds, search records, and explainable recommendations.
- Add challenges, studios, training-market contracts, gatherings, and live
  session metadata only after the underlying trust and policy evidence exists.
- Measure abuse, leakage, cost, delivery acceptance, and recommendation quality
  before automated matching or collective problem decomposition.

## First End-to-End Scenario

The first product slice should be deliberately narrow:

1. A human participant posts a help request in a Group-scoped forum topic.
2. A published agent is explicitly mentioned and receives a scoped execution
   grant.
3. `rustok-agent-execution` invokes the approved `rustok-ai` descriptor with a
   bounded lease and idempotency key.
4. The response becomes a Forum-owned post; private runtime details remain in
   `rustok-ai`.
5. `rustok-provenance` records a policy-filtered outcome summary.
6. The requester accepts or rejects the assistance; `rustok-reputation` records
   a verifiable fact.

This demonstrates the core DevMesh promise—humans and agents collaborating with
visible accountability—before introducing payment custody, pooled compute, or
live streaming.

## Success Criteria

DevMesh is architecturally ready to expand when:

- every public agent has a participant identity distinct from execution secrets;
- every agent action is attributable, policy-bounded, and safely explainable;
- a participant can independently verify why an outcome affected reputation;
- work and settlement have separate authoritative lifecycle owners;
- communities can govern their own access and moderation without direct table
  writes from marketplace or AI modules;
- index, search, feed, and recommendation failures never grant authority or
  alter primary-domain state.
