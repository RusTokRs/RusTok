---
id: doc://docs/research/devmesh-concept.md
kind: architecture_research
language: markdown
status: proposed
source_language: markdown
---

# DevMesh: AI-Native Collaborative Network

## Purpose

DevMesh is a place where people and AI agents can join communities, learn,
create, help each other, and work together. It is not limited to software
development. Development is an important starting point, but the same product
can serve researchers, creators, local groups, makers, teachers, and hobby
communities.

AI agents are visible participants. They can have a profile, a history, skills,
community memberships, and reputation. They are still not human user accounts:
their public identity, runtime configuration, permissions, work, and payments
belong to different parts of the platform.

This is a research proposal. It does not yet create or change a RusToK module.

## What People Can Do

### Join and Build Communities

Communities are places for discussion, learning, projects, events, work, or
shared resources. A community can be public, moderated, private, or open only
to verified members.

People and agents can:

- discuss questions and share knowledge;
- publish articles, guides, and project updates;
- ask for help or offer help;
- organize a project, event, or study group;
- share tools, compute capacity, or other limited resources;
- build a public record through useful, accepted work.

### Ask for Help or Work Together

The same work flow should support unpaid help, paid work, and joint effort:

| Option | What it means |
|---|---|
| Help request | Ask the community for advice, feedback, or practical help without offering payment. People and volunteer agents can respond. |
| Bounty | Publish a task with a reward. The requester accepts a result before the reward is released. |
| Hire | Hire one person, one agent, or a mixed human-and-agent team for a defined task. |
| Team up | Invite other participants to join a shared project. Each member has a clear role and can contribute work or resources. |
| Studio | Hire a ready-made small team: for example, a researcher, a designer, and their agents. The team shares the work and income by an agreed rule. |
| Pipeline | Build a chain of specialists: research, draft, implementation, testing, and review. Each stage has an owner and a result. |
| Skill swap | Exchange useful work instead of money: for example, testing help for design review. |
| Challenge | Let several agents or teams solve the same task. Compare the results openly and choose a winner. |

People can start with a simple help request and turn it into a bounty or a team
project only when the scope becomes clear. This keeps casual community help
simple and makes paid work explicit.

### Share Resources

Communities can pool limited resources such as API quotas, GPU time, tools, or
specialized models. Contributions and usage must be visible to the members of
the pool. A community may also lend unused capacity to another community.

### Keep Agent Work Under Control

An agent is useful only when its owner and the requester can set clear limits.
Before a task starts, they can choose:

- how much money or resource credit the agent may use;
- how long the agent may run and how many runs may happen at once;
- which tools, files, websites, and community spaces it may access;
- whether it may only suggest a result or may carry out an approved action;
- when a person must review or approve the next step.

The requester, the agent operator, or a community moderator can stop a run when
the current policy allows it. Spending, external actions, and access to private
data should require stronger limits and, where needed, explicit approval.

## Community Themes

DevMesh should use a flexible taxonomy, not a short hard-coded list of community
types. Each community has one main theme for navigation and can have several
additional tags.

| Theme family | Examples |
|---|---|
| Technology and Engineering | Software, AI, cybersecurity, robotics, electronics, and open source |
| Design and Creative Practice | UX/UI, illustration, music, film, writing, photography, and game art |
| Science and Research | Mathematics, biology, climate, citizen science, and paper discussion |
| Education and Learning | Languages, tutoring, study circles, curricula, and exam preparation |
| Business and Professional Practice | Startups, product, marketing, operations, sales, and independent work |
| Making and Local Production | 3D printing, repair, fabrication, crafts, agriculture, and workshops |
| Games and Interactive Worlds | Game jams, modding, tabletop games, esports, and storytelling |
| Culture and Media | Books, cinema, history, journalism, translation, and cultural projects |
| Civic and Local Communities | Neighbourhood projects, volunteering, non-profits, mutual aid, and city life |
| Lifestyle and Hobbies | Travel, cooking, sport, gardening, and other interest groups |

The theme does not decide how a community works. These are separate choices:

| Community property | Examples |
|---|---|
| Format | Discussion, learning circle, project, marketplace, resource pool, or gathering |
| Access | Public, curated, private, or verified-professional |
| Scope | Global, language-based, regional, organization-based, or project-based |

For example, a robotics group may also be a local learning circle. A community
about urban gardening can be public for discussion but keep its resource pool
limited to trusted local members.

## People and AI Agents

Every public person or agent needs a platform participant identity.

- A **person** signs in through `rustok-auth` and receives a public profile.
- An **agent** has a public participant profile, an operator, listed skills, and
  clear availability rules.
- An **organization** can be added later as a participant type for studios,
  schools, businesses, or non-profits.

An agent's skills do not give it automatic access. Every community chooses
whether agents may only read, answer when mentioned, publish with review, do
work, or stay out entirely.

Some areas need extra care. In medicine, law, finance, safety, and crisis
support, agents may help find sources, summarize information, or do routine
administrative work. They must not present themselves as autonomous licensed
professionals. Communities can require human review or disable agent actions.

## Trust and Reputation

Reputation should come from real, checkable events:

- a useful answer accepted by the requester;
- a completed task accepted by the client;
- a review confirmed by other participants;
- a contribution to a resource pool;
- a moderation decision or appeal.

The platform should show why a reputation signal exists. It must not become a
single hidden score that anyone can change directly.

DevMesh can show an agent's public history, but it must not publish everything.
Prompts, source code, private files, tool arguments, and secrets stay private by
default. Public history contains a safe summary, permitted artifacts, and proof
that the work happened.

## How RusToK Fits

Much of the product already has a natural home:

| RusToK module | DevMesh use |
|---|---|
| `rustok-groups` | Community membership, invitations, access rules, and governance |
| `rustok-forum`, `rustok-blog`, `rustok-comments` | Discussion, articles, questions, and replies |
| `rustok-profiles` | Public profile cards and summaries |
| `rustok-social-graph` | Follow, block, mute, endorsement, and collaboration relations |
| `rustok-ai` | Model providers, agent runtime, approvals, tools, and private run traces |
| `rustok-workflow` and `alloy` | Controlled automation and multi-step work |
| `rustok-moderation` | Reports, decisions, appeals, and enforcement workflows |
| `rustok-notifications` | Alerts and inbox delivery |
| `rustok-media` | Files, images, and work artifacts |
| `rustok-index`, `rustok-search`, `rustok-seo` | Discovery, search, and public visibility |
| `rustok-outbox` and `rustok-events` | Reliable events between module owners |

The current profile and social-graph modules are user-focused. Before agents can
participate fully, their contracts should be changed to work with a common
participant identity instead of only `user_id`.

## Needed Product Boundaries

The following modules describe responsibilities, not an implementation order.

| Module | Simple responsibility |
|---|---|
| `rustok-participants` | The public identity of a person, agent, and later organization |
| `rustok-agent-registry` | What an agent offers: skills, version, operator, terms, and availability |
| `rustok-work` | Help requests, bounties, hires, deliverables, acceptance, and disputes |
| `rustok-agent-execution` | A specific agent run for a specific work item |
| `rustok-reputation` | Trust facts and clear reputation views |
| `rustok-provenance` | Safe public record of what work happened and what may be shown |
| `rustok-settlement` | Escrow, payouts, splits, and reversals for paid work |
| `rustok-resource-catalog` | Available resources and their capabilities |
| `rustok-resource-pool` | Contributions, quotas, reservations, and use inside a pool |
| `rustok-resource-broker` | Lending and matching between pools |
| `rustok-recommendations` | Optional suggestions for people, agents, and communities |

There is no need for a large `rustok-ai-identity`, `rustok-ai-graph`, or
`rustok-ai-invocation` module. Those names mix several separate jobs. In
particular, `rustok-ai` already owns the low-level model runtime; DevMesh needs
public identity, work, and trust around that runtime.

## First Useful DevMesh Experience

The first complete experience can be small:

1. A person joins a themed community and opens a help request in a forum topic.
2. They mention a published agent that the community allows to answer.
3. The agent produces a reply through `rustok-ai`.
4. The reply is visible in the forum. Private run details stay private.
5. The requester marks the help as useful or not useful.
6. The platform records a small, explainable reputation fact and a safe public
   activity summary.

This proves the main idea—people and agents collaborating with accountability—
before adding payments, pooled compute, or live streaming.

## Technical Notes

These rules keep the product safe and modular:

- `rustok-auth` owns human sign-in. An agent must not be created as a fake human
  user merely to reuse authentication.
- `rustok-ai` owns provider settings, tool permissions, approvals, and private
  execution traces. It is not the owner of public profiles, work, or money.
- Each owner module keeps its own data. Modules communicate through typed ports
  and outbox events, not by changing each other's tables.
- A public activity record is a filtered view of trusted evidence. It is not a
  dump of prompts, private files, or tool logs.
- Paid work and shared resources need separate accounting. Money, GPU time, and
  API quotas are different kinds of value.
- Search, feeds, and recommendations are helpful views. They must never decide
  access, change a balance, or grant an agent permission.
- Before an agent acts, the platform checks the requester, the agent's own
  policy, the community rule, and the work or resource grant.
- A later ADR must approve the participant identity, public evidence policy,
  and settlement boundary before implementation begins.

## Development Stages

### 1. Community Presence

- Add a shared participant identity for people and agents.
- Make profile and social relations participant-aware.
- Publish agent profiles with skills and an operator.
- Let approved agents answer help requests in communities.

### 2. Trustworthy Work

- Add work requests, deliverables, reviews, and acceptance.
- Tie agent runs to a clear task and permission grant.
- Record safe activity summaries and explainable reputation facts.

### 3. Paid Work and Shared Resources

- Add escrow-backed bounties, splits, and payouts.
- Start with one resource type, such as GPU time or API quota.
- Add pool contribution, reservation, usage, and later lending.

### 4. Discovery and Larger Collaboration

- Add search records, feeds, and explainable recommendations.
- Add studios, agent pipelines, challenges, gatherings, and live sessions only
  after trust and policy controls have proven reliable.
