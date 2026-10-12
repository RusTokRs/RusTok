---
id: doc://docs/architecture/workplace-collaboration-superapp-plan.md
doc_type: architecture_proposal
status: proposed
owner: platform-architecture
canonical_for:
  - workplace-collaboration-superapp-plan
language: markdown
---

# Unified Workplace & Collaboration SuperApp: Messenger, Notion-Grade Docs, LiveKit Calls, and Calendar in RusToK

## 1. Executive Summary and Strategic Objective

Modern business software is severely fragmented across disconnected SaaS silos:
- **Team Communication:** Slack / Telegram / Microsoft Teams.
- **Knowledge & Documentation:** Notion / Confluence.
- **Video & Voice Conferencing:** Zoom / Google Meet.
- **Scheduling & Appointments:** Calendly / Cal.com.
- **Issue Tracking:** Linear / Jira.
- **ERP, Commerce, & Accounting:** Shopify, 1C:Enterprise, Odoo, SAP.

This fragmentation forces teams to constantly context-switch, copy-paste data manually across browser tabs, and rely on fragile third-party automation webhooks (Zapier/Make). Solutions like Macro (`macro-inc/macro`) attempt to unify team communication and note-taking, but they remain isolated from real-world enterprise transactional mechanics (inventories, warehouses, double-entry accounting, order checkouts, and multi-vendor marketplaces).

**The RusToK Strategic Advantage:**
RusToK expands beyond high-integrity Commerce, ERP, and Accounting into an **All-in-One Enterprise SuperApp**. By pairing our transactional PostgreSQL/ACID foundation and [ERP/CRM/Accounting Architecture](./crm-erp-accounting-matrix-and-plan.md) with a **highload communication and collaborative knowledge layer**, RusToK delivers:
1. **Highload Distributed Messenger (`rustok-messenger`):** Horizontally scalable messaging fabric backed by [Iggy streaming](../modules/registry.md) (`rustok-iggy`), monotonic sequence cursors, in-memory presence ring-buffers, sharded append-only message storage, and optional E2EE.
2. **Notion-Grade Collaborative Documents (`rustok-docs`):** Block-based structured pages with real-time multiplayer editing powered by **Loro CRDT** (Rust), integrated with [`crates/modules/flex`](../../crates/modules/flex) and [`crates/ui/rustok-grid`](../../crates/ui/rustok-grid) to provide dynamic database views (Table, Kanban, Calendar, Gallery).
3. **Live Business Blocks (Живые блоки):** Documents are not static text; they natively embed live product cards ([`rustok-product`](../../crates/modules/rustok-product)), live warehouse inventory stock levels ([`rustok-inventory`](../../crates/modules/rustok-inventory)), commercial invoices ([`rustok-billing`](./crm-erp-accounting-matrix-and-plan.md)), and CRM pipeline cards ([`rustok-deal`](./crm-erp-accounting-matrix-and-plan.md)).
4. **WebRTC Video & Voice Conferencing (`rustok-calls`):** Scalable audio/video rooms, screen sharing, persistent in-call chat, and recording powered by **LiveKit SFU**, with automated AI meeting transcripts and action item extraction into CRM deals.
5. **Calendar & Appointment Booking (`rustok-calendar`):** Team and personal schedules, RFC 5545 iCalendar / CalDAV synchronization, OAuth connectors (Google Calendar, Outlook/Exchange, Yandex), and public booking links that trigger deal creation and call room provisioning.
6. **Unified Mobile SuperApp ([`rustok_mobile`](../../rustok_mobile)):** Fully native Flutter application providing offline-first document caching, instant messaging, push notifications, and video calling.

---

## 2. Domain Decomposition & Sovereign Boundaries

| Sovereign Module | Crate Path | Core Responsibility | Persistence & Technology Stack | Non-Duplication Boundary |
| :--- | :--- | :--- | :--- | :--- |
| **`messenger`** | `crates/modules/rustok-messenger` | High-availability team chats, DMs, threads, channel hierarchies, read receipts, typing indicators, reactions. | **Streaming:** [`rustok-iggy`](../../crates/modules/rustok-iggy) partitions.<br>**Hot Cache:** Redis/Dragonfly.<br>**Cold Storage:** Partitioned PostgreSQL (`messenger_messages`, `messenger_conversations`). | Focuses purely on message transmission, cursors, and delivery. Does not own business entities; conversations link to external entities via `entity_type` + `entity_id`. |
| **`docs`** | `crates/modules/rustok-docs` | Block-based collaborative documents, workspaces, page trees, templates, Notion-like databases. | **CRDT:** Loro (Rust) delta logs + periodic page snapshots in PostgreSQL (`docs_pages`, `docs_blocks`, `docs_snapshots`). | Replaces static wiki tools. Cooperates with [`flex`](../../crates/modules/flex) for typed database properties and [`rustok-grid`](../../crates/ui/rustok-grid) for view rendering. |
| **`calls`** | `crates/modules/rustok-calls` | Audio/video rooms, participant tokens, screen sharing orchestration, recording egress hooks, meeting transcript ingestion. | **SFU:** LiveKit Server.<br>**State:** PostgreSQL (`calls_rooms`, `calls_sessions`, `calls_recordings`, `calls_transcripts`). | Media streaming is delegated to LiveKit SFU; `rustok-calls` manages room lifecycle, access tokens, RBAC authorization, and transcript persistence. |
| **`calendar`** | `crates/modules/rustok-calendar` | Personal & resource calendars, scheduling slots, meeting invitations, CalDAV / external calendar bi-directional synchronization. | **State:** PostgreSQL (`calendar_events`, `calendar_attendees`, `calendar_booking_slots`).<br>**Sync:** RFC 5545 iCalendar parsers. | Owns time allocations and booking links. Automatically creates calls via `rustok-calls` and notifies participants via `rustok-messenger`. |
| **`tasks`** | `crates/modules/rustok-tasks` | Linear-like team issues, sprints, backlogs, priorities, estimates, dependencies, board views. | **State:** PostgreSQL (`tasks_issues`, `tasks_sprints`, `tasks_labels`). | Issue tracking and developer/team tasks. Operational commercial CRM deals remain sovereign in [`rustok-deal`](./crm-erp-accounting-matrix-and-plan.md). |

---

## 3. Highload Distributed Messenger Architecture (`rustok-messenger`)

To guarantee zero database lock contention and scale horizontally across clusters:

```
[Flutter SuperApp / Web Admin]
            │
      (WebSocket / QUIC / gRPC stream)
            ▼
   [Edge Gateways (rustok-gateway)]
            │
            ├──────► [Iggy Streaming Fabric (rustok-iggy)]
            │             │  (Sub-ms stream partitioned by conversation_id)
            │             ├──────► [Ephemeral Hot State (Redis / Dragonfly)]
            │             │             • Presence ("online", "last seen")
            │             │             • Typing indicators
            │             │             • Ring-buffer (last 100 messages)
            │             ▼
            └──────► [Worker Ingestion Pipeline]
                          │  (Batched transactional writes)
                          ▼
                     [Partitioned PostgreSQL Store]
                          • append-only messages (Snowflake monotonic seq_id)
                          • zero update locks
```

### 3.1 Invariants of the Messenger Core
1. **Append-Only Immutability:** Messages are never updated in-place. Edits and deletions append delta events (`MessageEdited`, `MessageDeleted`) referencing the original 64-bit `message_id`.
2. **Monotonic Sequence Cursor (Snowflake ID):** Every message assigned a monotonically increasing 64-bit sequence identifier. Offline clients synchronize by requesting `GET /conversations/{id}/messages?since_seq={last_seen_seq}`, ensuring gap-free, deduplicated reconciliation.
3. **Decoupled Delivery Pipeline:** Incoming messages are immediately published to [`rustok-iggy`](../../crates/modules/rustok-iggy) and acknowledged to the client within < 5 ms. Persistence workers batch-insert records into PostgreSQL asynchronously, preventing database bottlenecks during traffic surges.
4. **End-to-End Encryption (E2EE):** Direct 1-on-1 and sensitive executive channels support double-ratchet E2EE; the server persists and relays encrypted binary blobs without inspecting payloads.

---

## 4. Notion-Grade Collaborative Documents (`rustok-docs`)

### 4.1 Block-Tree Data Model & Loro CRDT
Every page is modeled as a hierarchical tree of atomic blocks:
```rust
pub struct DocBlock {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub block_type: BlockType, // Paragraph, Heading, BulletList, Todo, Callout, Toggle, Table, Code, LiveErp
    pub properties: serde_json::Value,
    pub content: LoroText,     // High-performance CRDT rich-text
    pub children: Vec<Uuid>,
}
```

* **Zero Merge Conflicts:** Concurrent edits by multiple team members or AI agents are merged deterministically using the **Loro CRDT** engine compiled natively in Rust.
* **Granular Block Reordering:** Drag-and-drop block reordering is represented as CRDT tree movements, completely avoiding split-brain documents.

### 4.2 Notion-Like Databases: Unifying `flex` and `rustok-grid`
A database in `rustok-docs` is an aggregate of structured pages conforming to a typed schema:
1. **Data Model ([`crates/modules/flex`](../../crates/modules/flex)):** Reuses `FieldDefinition` to define typed column attributes (Text, Number, Select, MultiSelect, Date, Person, Formula, Relation, Rollup, Status).
2. **View Engine ([`crates/ui/rustok-grid`](../../crates/ui/rustok-grid)):** Provides 5 polymorphic views over the same underlying dataset:
   - **Table View:** Dense data grid with faceted filtering and column reordering.
   - **Kanban Board:** Columns grouped by `Status` or `Select` properties with drag-and-drop cards.
   - **Calendar View:** Events arranged by date attributes.
   - **Gallery View:** Visual cards highlighting image covers and key attributes.
   - **List View:** Minimalist linear summary.

### 4.3 Universal Entity Reference Protocol (UER / `EntityRef`)
To enable Notion-like live embedding of any platform entity across all 50+ RusToK modules, `rustok-docs` establishes a unified, strongly-typed reference protocol:

```rust
/// Canonical Uniform Entity Reference
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub tenant_id: Uuid,
    pub module: EntityModule, // Media, Blog, Forum, Pages, Taxonomy, Commerce, CRM, Billing, Flex
    pub entity_type: String,  // "asset", "post", "topic", "page", "term", "product", "deal", "invoice", "custom"
    pub entity_id: Uuid,
}

impl EntityRef {
    /// Formats as standard URN: urn:rustok:<tenant_id>:<module>:<entity_type>:<entity_id>
    pub fn to_urn(&self) -> String {
        format!("urn:rustok:{}:{}:{}:{}", self.tenant_id, self.module.as_str(), self.entity_type, self.entity_id)
    }
}
```

#### Pluggable Entity Preview Port (`EntityCardResolverPort`)
Host applications and consumer modules provide resolvers conforming to a unified trait:
```rust
#[async_trait]
pub trait EntityCardResolverPort: Send + Sync {
    async fn resolve_entity_preview(
        &self,
        security: SecurityContext,
        entity_ref: &EntityRef,
        view_mode: EntityViewMode,
    ) -> Result<EntityPreviewCard, EntityResolveError>;
}
```

### 4.4 Four Polymorphic Display Modes
Every embedded entity can be seamlessly toggled across 4 projection views without changing the underlying reference:

| Display Mode | Visual Representation | Target Entities & Use Cases |
| :--- | :--- | :--- |
| **1. Inline Mention / Pill** | `@Topic: RFC-042 Highload Messenger` | Mentioning team members, forum threads, blog drafts, products, or wiki pages in running text. Hovering displays an instant popover card. |
| **2. Compact Card / Bookmark** | Title, author avatar, thumbnail, excerpt, status badge, and reply/view count | Bookmarking relevant articles, customer tickets, contracts, or closed deals. |
| **3. Interactive Live Widget** | Functional, interactive embedded UI component with real-time state | - **Media ([`rustok-media`](../../crates/modules/rustok-media)):** In-line video/audio player with waveform, multi-page PDF viewer with table-of-contents.<br>- **Forum ([`rustok-forum`](../../crates/modules/rustok-forum)):** Embedded discussion thread with in-line reply composer and reaction buttons.<br>- **Blog ([`rustok-blog`](../../crates/modules/rustok-blog)):** Readable article with reading progress and author bio.<br>- **Wiki ([`rustok-pages`](../../crates/modules/rustok-pages)):** Transcluded documentation section with two-way sync.<br>- **Commerce ([`rustok-product`](../../crates/modules/rustok-product)):** Live price, variant selector, and buy CTA.<br>- **ERP & CRM ([`rustok-crm`](./crm-erp-accounting-matrix-and-plan.md)):** Deal stage switcher, invoice payment tracking, act of reconciliation download. |
| **4. Database Relation & Rollup** | Structured database column linked to external entities | Creating custom databases (e.g. "Release Tracker") where records link directly to Forum Topics, Blog Posts, or Media Assets, with auto-populated rollup fields (e.g. view count, reply count, publishing status). |

### 4.5 Media Lifecycle & Reference Retainment (`DocsMediaReferencePolicy`)
Embedding media in collaborative documents adheres strictly to the [`rustok-media`](../../crates/modules/rustok-media) reference governance discovered during code review:
1. **Reference Registration:** When an image, video, audio file, or PDF is inserted into a doc block, `rustok-docs` registers a reference via `MediaReference` with owner `docs` and retention policy `RetainedByActiveDocBlock`.
2. **Preventing Premature Garbage Collection:** Background cleanup jobs in `rustok-media` fail closed and never purge media assets that retain active references in published or revision-history document blocks.
3. **Signed Streaming URLs:** For sensitive internal documents, media assets are streamed via signed, time-limited tokens generated on demand according to the viewer's document access level.
4. **Blurhash & Responsive Transcoding:** Automatic generation of blurhash placeholders and responsive WebP/AVIF variants for zero-layout-shift document rendering.

### 4.6 Bidirectional Backlinks & Knowledge Graph Index (`docs_entity_links`)
To transform documentation into an interconnected company brain, `rustok-docs` maintains an append-only relational link ledger:

```sql
CREATE TABLE docs_entity_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    source_doc_id UUID NOT NULL,
    source_block_id UUID NOT NULL,
    target_module VARCHAR(64) NOT NULL,
    target_entity_type VARCHAR(64) NOT NULL,
    target_entity_id UUID NOT NULL,
    link_type VARCHAR(32) NOT NULL, -- 'mention', 'embed_card', 'interactive_widget', 'relation_column'
    anchor_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_docs_entity_links_doc FOREIGN KEY (tenant_id, source_doc_id) REFERENCES docs_documents(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_docs_entity_links_target ON docs_entity_links (tenant_id, target_module, target_entity_type, target_entity_id);
CREATE INDEX idx_docs_entity_links_source ON docs_entity_links (tenant_id, source_doc_id);
```

* **Reverse Backlinks Accordion:** Any Forum Topic, Blog Post, Product page, or CRM Deal automatically displays a "Referenced in Documents" panel listing all pages mentioning it.
* **AI Knowledge Graph (MCP Seam):** Feeds directly into `rustok-mcp` and AI agents, allowing LLMs to reason across cross-domain relationships (e.g., *"What technical specs or forum discussions are linked to Invoice INV-2026-004?"*).

### 4.7 RBAC Security Delegation & Graceful Redaction
Collaborative documents frequently combine public documentation with confidential internal records:
* **Context Propagation:** When rendering an embedded entity, the resolver is passed the viewing user's canonical `SecurityContext`.
* **Zero Leakage / Masked Card Fallback:** If a user viewing a shared document does not have permission to read the embedded resource (e.g., an unauthorized team member viewing a document with an embedded B2B Invoice or private HR Forum Topic), the live block renders a graceful masked card:
  > 🔒 **Restricted Resource** — *You do not have permission to view this Forum Topic / Invoice in this tenant.*
* **Strict Tenant Boundary:** Cross-tenant entity resolution is unconditionally rejected at the database and resolver levels.

### 4.8 Cross-Domain Content Orchestration & Transformation
Integrating with [`rustok-content-orchestration`](../../crates/modules/rustok-content-orchestration) (`ContentOrchestrationBridge`), `rustok-docs` supports seamless content lifecycle workflows:
* **Promote Topic to Doc (`promote_topic_to_doc`):** Transform a successful brainstorm or architectural discussion in [`rustok-forum`](../../crates/modules/rustok-forum) into an official internal specification or policy document.
* **Publish Doc to Blog (`publish_doc_to_blog`):** Export an approved product announcement or whitepaper directly into [`rustok-blog`](../../crates/modules/rustok-blog) with preserved rich-text AST, images, and category metadata.
* **Transclude Wiki Page (`transclude_wiki_to_doc`):** Embed live, auto-synchronizing sections from [`rustok-pages`](../../crates/modules/rustok-pages) into team workspaces without manual copy-pasting.

### 4.9 Universal Search & Fast Inserter (`UniversalEntityPicker`)
Typing `@` or `/` in the rich-text editor triggers a unified fuzzy search across the entire tenant knowledge base:
* Blog Posts ([`rustok-blog`](../../crates/modules/rustok-blog))
* Forum Topics & Replies ([`rustok-forum`](../../crates/modules/rustok-forum))
* Wiki & CMS Pages ([`rustok-pages`](../../crates/modules/rustok-pages))
* Media Assets & Documents ([`rustok-media`](../../crates/modules/rustok-media))
* Taxonomy Terms & Categories ([`rustok-taxonomy`](../../crates/modules/rustok-taxonomy))
* Products & SKUs ([`rustok-product`](../../crates/modules/rustok-product))
* CRM Deals & Leads ([`rustok-crm`](./crm-erp-accounting-matrix-and-plan.md))
* Counterparties & Contracts ([`rustok-party`](./crm-erp-accounting-matrix-and-plan.md))
* Internal Collaborative Docs ([`rustok-docs`](./workplace-collaboration-superapp-plan.md))

---

## 5. WebRTC Conferencing & AI Meeting Assistant (`rustok-calls`)

1. **Selective Forwarding Unit (LiveKit):**
   - Self-hosted or managed LiveKit SFU handles high-bandwidth WebRTC audio/video distribution.
   - `rustok-calls` acts as the control plane: validates [`rustok-rbac`](../../crates/modules/rustok-rbac) room permissions, signs short-lived participant JWTs, and processes room lifecycle webhooks.
2. **AI Meeting Transcripts & Action Items:**
   - Egress workers record meeting audio tracks.
   - Background audio transcription pipelines (Whisper / [`rustok-ai`](../../crates/modules/rustok-ai)) produce synchronized text transcripts.
   - Large Language Models extract structured action items and meeting summaries, automatically publishing them to the associated deal in [`rustok-deal`](./crm-erp-accounting-matrix-and-plan.md) or conversation thread in `rustok-messenger`.

---

## 6. Calendar & Appointment Booking Engine (`rustok-calendar`)

1. **Enterprise Scheduling:**
   - Multi-calendar support for users, meeting rooms, and shared team resources.
   - Bi-directional CalDAV and OAuth synchronization with Google Calendar, Microsoft Outlook, and Yandex 360.
2. **Public Booking Links (Calendly / Cal.com Replacement):**
   - Configurable availability windows (e.g., "Monday–Friday, 10:00–18:00 MSK, 30-minute slots").
   - Buffer times between appointments, automated timezone conversion.
   - Booking confirmation triggers:
     - Calendar event creation with auto-generated LiveKit meeting URL.
     - Confirmation email via [`rustok-email`](../../crates/modules/rustok-email) and messenger alert via `rustok-messenger`.
     - Automatic counterparty / lead creation in [`rustok-party`](./crm-erp-accounting-matrix-and-plan.md) and [`rustok-deal`](./crm-erp-accounting-matrix-and-plan.md).

---

## 7. Phased Implementation Roadmap

```mermaid
gantt
    title Workplace & Collaboration SuperApp Implementation Roadmap
    dateFormat  YYYY-MM-DD
    section Phase 1: Highload Messenger Core
    Iggy Streaming Fabric & Gateways      :p1_1, 2026-11-01, 30d
    Sharded Storage & Sync Cursor Protocol :p1_2, after p1_1, 25d
    Flutter Mobile UI & WebSocket Client   :p1_3, after p1_2, 20d

    section Phase 2: Collaborative Docs (Notion Alternative)
    Block Tree & Loro CRDT Rust Engine     :p2_1, 2026-12-15, 30d
    Flex Database Views (Table, Kanban)    :p2_2, after p2_1, 25d
    Live Business & Media/Blog/Forum Blocks:p2_3, after p2_2, 20d
    Backlinks & Knowledge Graph Index      :p2_4, after p2_3, 15d

    section Phase 3: LiveKit Calls & AI Transcripts
    LiveKit Control Plane & Token Signer   :p3_1, 2027-02-15, 20d
    Mobile & Web WebRTC Client Integration :p3_2, after p3_1, 20d
    Meeting Transcripts & AI Action Items  :p3_3, after p3_2, 15d

    section Phase 4: Calendar & Booking
    Calendar Event Engine & CalDAV Sync    :p4_1, 2027-03-25, 25d
    Public Booking Slots & CRM Automations :p4_2, after p4_1, 20d
```

### Milestone Checklist:
- [ ] **Milestone 1 (`rustok-messenger`):** Standalone distributed messaging passing 10,000 msgs/sec in-process benchmarking with Iggy; offline-first replay verified with Flutter test harness.
- [ ] **Milestone 2 (`rustok-docs`):** Concurrent editing of block trees with Loro CRDT; zero-conflict resolution under 10 concurrent synthetic writers; dynamic Table and Kanban views operational; Universal Entity Reference protocol (`EntityRef`), Media retainment policies (`DocsMediaReferencePolicy`), and bidirectional backlinks ledger (`docs_entity_links`) verified.
- [ ] **Milestone 3 (`rustok-calls`):** LiveKit SFU integration; end-to-end room creation, audio/video broadcast, screen sharing, and transcription pipeline.
- [ ] **Milestone 4 (`rustok-calendar`):** Bi-directional RFC 5545 sync; public booking links generating calendar events and CRM deals.

---

## 8. Cross-Reference and Documentation Governance

- **Core Business Architecture:** [CRM, ERP, and Accounting Matrix and Plan](./crm-erp-accounting-matrix-and-plan.md)
- **ADR Governance:** [ADR: Atomic CRM, ERP, and Accounting Domain Architecture](../../DECISIONS/2026-10-12-atomic-crm-erp-accounting-architecture.md)
- **Streaming Infrastructure:** [`crates/modules/rustok-iggy`](../../crates/modules/rustok-iggy)
- **Dynamic Field Engine:** [`crates/modules/flex`](../../crates/modules/flex)
- **UI Grid & Views:** [`crates/ui/rustok-grid`](../../crates/ui/rustok-grid)
- **Mobile SuperApp:** [`rustok_mobile`](../../rustok_mobile)
- **Media Asset Engine:** [`crates/modules/rustok-media`](../../crates/modules/rustok-media)
- **Content & Forum Domain:** [`crates/modules/rustok-blog`](../../crates/modules/rustok-blog), [`crates/modules/rustok-forum`](../../crates/modules/rustok-forum), [`crates/modules/rustok-pages`](../../crates/modules/rustok-pages)
- **Cross-Domain Content Orchestration:** [`crates/modules/rustok-content-orchestration`](../../crates/modules/rustok-content-orchestration)
- **Taxonomy & Category Trees:** [`crates/modules/rustok-taxonomy`](../../crates/modules/rustok-taxonomy)
