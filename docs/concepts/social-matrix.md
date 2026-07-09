---
id: doc://docs/concepts/social-matrix.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Social Matrix Concept

0) System Doctrine
0.1. Main Principle

Everything is an Entity with:

- configurable fields (Custom Fields / Schema)
- universal relations (Relations)
- unified privacy (ACL/Visibility Matrix)
- single lifecycle (Draft→Publish→Edit→Archive→Delete)

This allows launching the platform incrementally: profiles+feed → groups → forum → marketplace → media, etc.

0.2. 7 Fundamental Principles

- Entity-based data model
- Groups = micro-social networks with modular constructor
- Forum = knowledge skeleton, not a "plugin on the side"
- Feed = central event aggregator
- Granular privacy "on every field/section/object"
- Module enablement at platform and group level
- Trust Levels + gamification as the systemic "glue" of engagement

1) Entity and Relation Layer
1.1. Basic Building Blocks

- Entity: id, type, owner_id, created_at, updated_at, status
- FieldSchema (for custom fields): type (text/number/date/select/file/relation), validations, localization
- Relation: from_entity → to_entity + relation_type + metadata (role, weight, date, source)
- Visibility/ACL: access rule for field, section, entity, action

1.2. Unified Privacy Standard (Visibility Matrix)

Audiences:

- Public / Everyone
- Friends
- Friends of Friends
- Followers
- Lists (custom)
- Only Me
- Hidden
- Group Members
- Role-based (admins/moderators/Trust Level ≥ N)

Important: BlackList (Block) = strict isolation (content/search/messages/mentions).

2) Identity, Profile, Account
2.1. User (User Entity)

Identification:
- username, email, phone

Account statuses:
- active / blocked / deleted / frozen

Types:
- regular / premium / moderator / admin

Verification:
- email, SMS, documents
- "badge" as attribute (blue/grey or trust levels)

Online status:
- online/offline, "last seen", custom status (emoji+text)

2.2. Profile (part of User or separate Entity)

- Base: avatar, cover, name, bio, city
- Extended: work, education, marital status
- Custom fields (CMS): admin adds any fields ("car brand", "favorite IDE")
- Showcase: achievements, badges, counters (friends, followers, reputation)

2.3. Profile Privacy

Access matrix for each field/section:
- who can see the profile
- who can see friends/subscriptions
- who can see online/last seen
- who can write
- who can comment/mention

3) Reputation, Trust Levels, Roles
3.1. Trust Levels (Discourse-style 0–4)

- TL0 Newcomer: restrictions on links/media/mentions
- TL1 Basic: standard rights
- TL2 Participant: can edit wiki posts
- TL3 Regular: extended rights (topic management, moving/renaming — by policy)
- TL4 Leader: almost a moderator (partially or fully)

Level growth is automatic based on activity (reading/replies/likes/time/reputation).

3.2. Global Platform Roles (CMS Roles)

- SuperAdmin / Admin / Editor / Global Moderator (+ granular permission scopes)

3.3. Gamification (Systemic)

- Karma/reputation: points for activity + thanks from people
- Badges: auto (year on site, 100 posts) + manual (awards from moderators)
- Leaderboards: top users/groups per week/month/all time

4) Social Graph (Hybrid Model)
4.1. Relationship Types (SocialEdge)

- Friend (mutual, confirmation)
- Follow (one-way)
- Soft downgrade: if removed from friends → remains a follower (optional)
- Lists: "close friends", "colleagues", "family", custom
- Mute: hide from feed without unfollowing
- Block: full isolation

4.2. Graph Functions

- incoming/outgoing requests
- recommendations (mutual friends/interests/geo — if available)
- contact import (optional)

5) Community Model: Groups, Pages, Events
5.1. Groups — Key Structural Element

Types:
- open / closed (by application) / secret (invite only, not searchable)

Roles within a group:
- owner (transferable), admin, moderator, member, banned

Settings:
- who can post (all / admins / premoderation)
- who can invite
- which modules are enabled

5.2. Group Modularity (phpFox-style constructor)

Group admin enables/disables blocks:

- Wall (micro-posts)
- Group Forum (see section 6)
- Media: photo/video/audio/files
- Wiki / knowledge base
- Polls
- Group Chat
- Group Market
- Group Events

5.3. Pages — Brands/Individuals/Organizations/Products

Differences:
- no "join", only "subscribe"
- possible hierarchy "brand → product"
- CTA buttons, contacts, business hours, reviews/ratings
- page market showcase

5.4. Events

- name, description, cover
- date/time, timezone
- location: online/offline
- organizer: user/group/page
- participation statuses: "going/maybe/not going/invited"
- content: discussion wall, photo reports, related forum topics

6) Forum Core (Discourse Engine) as "Knowledge Skeleton"
6.1. Global Forum + Local Group Forum

- Global forum: categories/subcategories/topics
- Group forum: tab within a group (topics live "longer than feed")

6.2. Topic / Thread and Features

- statuses: open/closed/pinned/archived
- wiki mode (first post editable by trusted users)
- mark as solution (Q&A)
- auto-close after N days
- merge/move
- tags
- editor: Markdown/BBCode, quote, @mention, attachments

Important: forum is the source of "eternal" content; feed is the accelerator of distribution.

6.3. "Group ↔ Forum" Relationship

Rule:
- "Discussion in group" = forum topic + group privacy inheritance

Synchronization:
- Open group: may appear in global forum/search and reach general feeds (by setting)
- Closed/Secret: topics visible only to members, not indexed globally

7) Content (Posts) and Lifecycle
7.1. Content Types (as Entity)

- Short Post (text + background)
- Rich Post (text + attachments: photo/video/poll/audio/files)
- Article (long-read, "VK Articles/Telegraph-style")
- Link preview (OG parsing)
- Repost (with/without comment)
- Forum Thread Snippet (with "Discuss" button)
- Media posts (gallery, video, audio)

7.2. Publication Attributes

- author
- publication location: user wall / group / page / event
- privacy: public/friends/lists/only me/group members
- geo, tags/hashtags, @mentions
- scheduled posting
- comments: on/off
- pinned post

7.3. Lifecycle

Draft → Scheduled → Published → Edited → Archived → Deleted
Edit history — visible (by policy).

8) Reactions, Comments, Interaction
8.1. Reactions

- basic like + extended emotions (❤️ 😂 😮 😢 😡 🔥)
- custom reactions (admin configurable)

8.2. Comments

- structure: flat + threaded replies
- reactions on comments
- @mentions, markdown
- edit/delete (by rules/time windows)
- comment pinning by author

8.3. Sharing

- to wall / to group / to DM / external link / to stories (if module enabled)

9) Feed — Central Event Aggregator
9.1. Aggregation Sources

- friends + subscriptions
- groups/pages/events
- forum topics (from subscribed categories/groups)
- recommendations (algorithmic)

9.2. Feed Modes

- Chronological: strictly by time
- Smart Feed: affinity score + engagement + freshness
- Interesting: popular for a period

Filters:
- photo/video only
- friends/groups only
- hide authors (mute)

Technique:
- infinite scroll, pagination, feed caching

10) Media Matrix (Global Media)
10.1. Photos

- albums (user/group/event)
- face tagging
- reactions/comments
- EXIF (hideable)
- download (toggleable)

10.2. Video

- upload + HLS streaming
- embed (YouTube, Vimeo, RuTube)
- shorts/reels — vertical feed
- live (optional)
- playlists, views, reactions

10.3. Audio (VK-style)

- track/podcast upload
- playlists
- global player (SPA behavior)

10.4. Files/Documents

- preview, versioning
- types: pdf/doc/zip/gif…
- sharing by audiences

11) Messenger (Messaging)
11.1. Structure

- 1-on-1 dialogs
- group chats (conferences) with administration

11.2. Message Types

- text, emoji/stickers/GIF
- photo/video/audio/files
- voice messages with recognition (Speech-to-Text) — "killer feature"
- geo
- reposts of posts/topics/products
- contact forwarding (profile)

11.3. Statuses and Features

- sent/delivered/read
- reply/forward
- edit/delete (by rules)
- message search
- pin/archive dialogs
- settings "who can write"

12) Marketplace (Socially Embedded)
12.1. Types

- classifieds (private listings)
- stores (shops within groups/pages)

12.2. Product/Listing Entity

- name, description, price/currency, condition, category
- photo gallery, geo
- statuses: active/sold/removed
- shipping (options)

Integrations:
- "message seller" → messenger
- seller reviews and rating
- listings in groups (flea markets)
- showcase on pages

13) Notifications

Categories:
- social (friends/subscriptions/groups)
- content (like/comment/repost)
- mentions
- forum (reply/new topic)
- messages
- events
- market
- system (verification/security/policies)

Channels:
- in-app, push, email (instant/digest), SMS (critical)

Settings:
- granular toggles by category+channel
- DND mode
- grouping of identical events
- digest frequency

14) Search and Navigation

Global search for:
- people, groups, pages, posts, topics, comments
- events, products, media, files

Filters:
- type, date, location, author, group/page

Hashtags:
- clickable, tag pages, trends by period

15) Moderation, Security, Privacy
15.1. Moderation (Two-Level)

Global:
- users (bans/strikes/verification)
- content (posts/comments/topics)
- groups/pages (closure/transfer)
- market

Local (in group/page):
- post premoderation
- member management
- group forum moderation
- group market moderation

Tools:
- reports with reasons
- moderation queue
- automod (anti-spam/stop-words/scoring, AI optional)
- moderator action audit logs
- shadow/temporary/permanent bans

15.2. Security

- 2FA
- session and device history
- activity log (logins/changes)
- account recovery
- suspicious activity alerts

16) CMS Layer: Themes, Blocks, Localization, Extensibility
16.1. Block/Widget System (phpFox-style)

- drag&drop layout editor
- widgets: "popular topics", "new users", "birthdays", "advertising"
- sidebar/header/footer configuration

16.2. Localization

- phrases as variables
- multilingual + RTL

16.3. Themes

- dark/light (auto)
- branding via CSS variables

16.4. Custom Fields

- field creation for any entity
- types: text/number/date/select/file/relation
- view/edit permissions

17) API and Integrations

- OAuth login via Google / Apple, etc. (by policy)
- contact import
- embed widgets for external sites
- internal API: REST + GraphQL, webhooks
- Bot API (Telegram model) — optional

18) Final Relationship Map (Logical)
graph TD
  U[User] --> P[Profile]
  U --> SG[Social Graph]
  SG --> F1[Friends/Followers/Lists/Mute/Block]

  U --> C[Content Entities]
  C --> FEED[Feed Aggregator]

  U --> G[Groups]
  G --> GW[Group Wall]
  G --> GM[Group Media]
  G --> GWIKI[Group Wiki]
  G --> GCHAT[Group Chat]
  G --> GMARKET[Group Market]
  G --> GF[Group Forum]

  GF -. privacy sync .-> FF[Global Forum]
  FF --> FEED

  U --> M[Messenger]
  M --> FEED

  U --> N[Notifications]
  U --> R[Reputation/Badges/Trust Level]

19) Canonical User Flow (Scenario Proving System "Glue")

- A user enters the "Rust Developers" group
- The group has the "Forum" + "Wiki" modules enabled
- Sees a pinned wiki post "FAQ/Rules"
- Writes a reply with Markdown, receives reactions
- Activity increases reputation → grows Trust Level
- The forum topic appears in friends'/subscribers' feeds (if audience allows)
- A friend sees the topic snippet in the feed, reacts 🔥 and reposts
- The author receives a notification (in-app + push), and the group gets engagement growth
- Knowledge remains in the forum "for years," while the feed brings traffic "now"

Summary (Concise)

From Gemini: concept of feed, entities, hybrid social graph, idea of "groups as micro-social networks", VK-like media/audio

From Opus: full modular layout, role/privacy/lifecycle tables, admin panel, security, search, notifications, forum engine as core

Result: a single document that can be split into modules and immediately used for domain/contract design.
