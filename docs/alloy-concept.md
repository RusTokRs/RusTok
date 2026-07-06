---
id: doc://docs/alloy-concept.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# ⚗️ ALLOY — Self-Evolving Integration Runtime

> **Ideate → Build → Refine → Ship to production.**
> Without pain, without manual code, without system downtime.

**License:** Business Source License 1.1 with RusTok Additional Use Grant
**Repository:** github.com/RustokCMS/RusTok
**Concept version:** 1.0 / February 2026

---

## Table of Contents

1. [What is Alloy](#1-what-is-alloy)
2. [The Problem Alloy Solves](#2-the-problem-alloy-solves)
3. [Three Levels of Freedom](#3-three-levels-of-freedom)
4. [How It Works](#4-how-it-works)
5. [Three Architecture Layers](#5-three-architecture-layers)
6. [Code Lifecycle](#6-code-lifecycle)
7. [Alloy and the Platform](#7-alloy-and-the-platform)
8. [Application Areas](#8-application-areas)
9. [Why Rust Only](#9-why-rust-only)
10. [Ecosystem Flywheel](#10-ecosystem-flywheel)
11. [Roadmap](#11-roadmap)
12. [Architectural Decisions](#12-architectural-decisions)

---

## 1. What is Alloy

Alloy is an independent AI-native capability/runtime layer.
It receives a task in natural language, writes executable code, runs it in an isolated secure environment, iterates on errors, and then translates stable scenarios into native Rust modules. RusToK can host Alloy as a platform surface, but Alloy should not be thought of as an internal tenant module of RusToK.

**Alloy is simultaneously:**

- **A self-learning integrator** — connects to any service that has API documentation
- **A universal migrator** — processes any dirty, broken, legacy data
- **A rapid prototyping tool** — from idea to working module without programmers
- **A business process automator** — replaces N8N, Zapier, Make, but smarter and faster
- **A custom service builder** — don't want to integrate with someone else's? Create your own

> **Alloy's universal law:**
> Where there is data — there Alloy works. And data is everywhere.

---

## 2. The Problem Alloy Solves

### The World of Integrations is Broken

Thousands of services exist. Each with its own API. Each changes that API without warning. Integrations are written by programmers — expensive and slow. Integrations break — programmers are needed again. Security is different for everyone, and for most it's non-existent.

**Result:** businesses spend enormous resources just to make their services talk to each other.

### Data is a Toxic Asset

Companies have terabytes of data accumulated in legacy systems:
- Logic is scattered across stored procedures
- Formats have changed over decades
- Documentation is lost
- The people who wrote it have left

This data holds businesses hostage. Leaving SAP or Oracle is impossible not because it's good there — but because the data is there.

### Vendor Lock-in Everywhere

Data in Salesforce, logic in HubSpot, payments in Stripe. Every service creates a dependency. Businesses pay for features they use 10% of. And they cannot leave.

---

## 3. Three Levels of Freedom

Alloy gives businesses three levels of working with any service:

### Level 1 — Integrate

Have API documentation? Alloy integrates. Stripe, Salesforce, 1C, Google Analytics, any ERP, any CRM. Describe what you want — get the integration. API changed? Alloy notices and fixes it automatically. Without programmers.

### Level 2 — Take What You Need

Don't need the entire Salesforce — just the contacts module? Alloy extracts exactly that functionality and wraps it in its own module. You pay only for what you use. You depend only on what you choose.

### Level 3 — Create Your Own

Don't want to depend on anyone? Alloy builds a module from scratch based on your description. Your own CRM, your own analytics, your own payment logic. On a platform like RusToK, which can handle any data and any scale — from a personal blog to NASA servers.

> **Alloy frees you from dependence on third-party services.**
> **Integrate, take what you need, or build your own. The choice is always yours.**

---

## 4. How It Works

```
User: "Connect the store to Stripe and notify in Telegram on payment"
       ↓
Alloy analyzes the platform: what modules exist, what data is available
       ↓
AI writes a Rhai integration script
       ↓
Script runs in an isolated sandbox environment
       ↓
Logic error → shows diff to human
Minor error (type, format) → AI patches automatically
       ↓
Script is stable, used frequently → AI decides to compile
       ↓
AI generates Rust code → cargo build → native module
       ↓
Module available in the marketplace for host platform users
```

### Three Interaction Modes

**Natural language (chat)**
```
"Every night collect stats from GA4,
 compare with last week,
 if drop > 20% — post to Slack"
```

**YAML config** (for repeatable tasks)
```yaml
name: ga4-weekly-alert
trigger:
  type: cron
  schedule: "0 2 * * *"
transform:
  script: scripts/ga4_compare.rhai
  on_error: auto_patch
output:
  - notify: slack.send(channel='alerts', template='weekly_drop')
```

**Programmatic API** (for embedding)
```rust
let integration = engine
    .create_from_prompt("Connect orders to Stripe")
    .await?;
engine.run(integration, source).await?;
```

---

## 5. Three Architecture Layers

| Layer | Technology | Role | Call cost |
|-------|-----------|------|-----------|
| 🧠 **Brain** | MCP + AI (any provider) | Understands the task, writes code, debugs logic, decides when to compile | Expensive — called rarely |
| ✋ **Hands** | Rhai (embedded scripting) | Executes the AI-written code. Fast. Secure. Without recompilation. | Cheap |
| ⚙️ **Iron** | Compiled Rust | Hot scenarios become native code. Maximum speed. | Free at runtime |

### Key Economy Principle

AI is called **only** when intelligence is needed:
- Write a new script
- Figure out an unclear error
- Decide whether to compile the script

Executing 10 million records — Rust, AI not involved.

### Rhai Sandbox Security

The script has no access to anything beyond explicitly passed API:

| Capability | Status |
|------------|--------|
| Filesystem access | 🚫 Denied |
| Network requests (HTTP) | 🚫 Only through whitelist |
| Process execution | 🚫 Denied |
| Unlimited memory | 🚫 Limit (configurable) |
| DB writes | ✅ Only through explicit API |
| Infinite loop | 🚫 Timeout |

---

## 6. Code Lifecycle

| # | Stage | Who | Mode |
|---|-------|-----|------|
| 1 | User describes the task (text / YAML / API) | Human | — |
| 2 | AI analyzes data, structure, anomalies, custom functionality | AI | Auto |
| 3 | AI writes Rhai script with edge-case handling | AI | Auto |
| 4 | Script runs in isolated sandbox environment | Rust/Rhai | Auto |
| 5 | Minor error (type, format) → AI patches automatically | AI | **Auto** |
| 6 | Logic change → shows diff to human | AI + Human | **Manual** |
| 7 | Runtime counts calls, measures latency, builds patterns | Rust | Auto |
| 8 | AI analyzes patterns and decides: compile? | AI | Semi-auto |
| 9 | AI generates Rust code from Rhai → cargo build → native module | AI + rustc | Auto |
| 10 | Module published to marketplace | Platform | Auto |

---

## 7. Alloy and the Platform

### Alloy is not a tenant module of RusToK, but a separate capability layer

RusToK can host Alloy and give it managed access to auth, permissions, events, module APIs and UI shell. But this does not make Alloy another module inside the tenant module lifecycle. For Alloy, RusToK is a host platform and one of several possible execution surfaces.

```
┌─────────────────────────────────────────────────────────┐
│                     HOST PLATFORM (e.g., RusToK)        │
│  Auth · Permissions · Events · Module APIs              │
└──────────────────────────┬──────────────────────────────┘
                           │ governed platform surface
                    ┌──────┴──────┐
                    │   Alloy     │
                    │   runtime   │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
         integrations  scripts    generated modules
```

### Alloy — a horizontal capability layer

A regular module does one thing. One domain knows about orders, another about customers, a third about content. They don't have to know about each other.

Alloy works across these boundaries. It is the conductor. It can write scripts that simultaneously touch commerce, content, workflow and external APIs. This is its nature — an integrator and generator of capability flows.

### Autonomous Alloy

Alloy can run not only on top of a full RusToK instance. The minimal assembly is the Alloy runtime plus a host-provided surface for auth, storage, events and execution policy. Inside RusToK, this host layer is represented by `rustok-core`, `apps/server`, `alloy` and `rustok-mcp`, but this does not make Alloy a RusToK crate in spirit.

---

## 8. Application Areas

### Legacy System Migration

Any data source — forums (vBulletin, phpBB), CMS (WordPress, Joomla), ERP (SAP, Oracle), COBOL banking systems, custom monsters with lost documentation. AI understands not ideal data, but real data — crooked, broken, with custom functionality.

A preset is not field mapping. It's reverse engineering of a live system: AI analyzes real data of a specific installation including all custom hacks, cleans artifacts and builds an adapter specifically for it.

### Real-time Integrations

Any service that has API documentation. Stripe, PayPal, Telegram, Slack, 1C, any ERP, any CRM. Without programmers. The integration fixes itself when the API changes.

### Business Process Automation

- "When an order arrives → invoice in 1C → SMS to customer → update warehouse"
- "Every night: analytics, comparison with last week, alert on drop"
- "Sync products between platforms every 15 minutes"

### Rapid Prototyping

Describe an idea → AI builds a script → test in an isolated environment → ship to production. From idea to working module — hours, not weeks.

### Building Custom Services

Don't need the entire Salesforce — just contacts? Create your own module. Don't want to pay for extras — build your own. Rustok can handle any data and any scale.

### Native Module Generation

Every AI-generated script that becomes a native module is a new opportunity for the next user. The marketplace grows organically by the users themselves.

### Healthcare

DICOM, HL7 FHIR — 50 hospitals in 50 formats → unified structure. Anonymization pipeline. HIPAA, 152-FZ with audit trail. Cost of error — human life. Rust memory safety is not an option, it's a requirement.

### Finance and Banking

Legacy COBOL from the 1970s. Transactional flows. Regulatory reporting. Anti-fraud pipeline in real time. Reconciliation between dozens of internal systems.

### Scientific Data

NASA, ESA, CERN. Terabytes of telemetry in non-standard formats. HDF5, FITS, NetCDF. Real-time data — cannot wait.

### Gaming Worlds

Integration with Unity, Unreal, Godot. AI generates rules for new territories. Rust executes for millions of objects in real time. Rules evolve based on player behavior.

### IoT and Edge Computing

Thousands of sensors in thousands of formats. Alloy runs on 512 MB RAM. Normalizes data at the source. Native modules are deployed OTA.

### Enterprise ERP Rewrite

A company wants to leave SAP but 20 years of data holds them hostage. Alloy processes the entire history. New ERP in months instead of years.

### CDC — Live Sync

Tracks the source's binlog/WAL in real time. A continuously running service, not a one-time migration. Foundation for a subscription business model.

---

## 9. Why Rust Only

| Requirement | Python/JS | Java/Go | C++ | Rust |
|------------|-----------|---------|-----|------|
| Hardware-level speed | ❌ | ⚠️ GC pauses | ✅ | ✅ |
| Memory safety (compiler guarantee) | ❌ | ⚠️ GC | ❌ | ✅ |
| Embedded scripting (Rhai — native) | N/A | N/A | N/A | ✅ |
| From IoT (512 MB) to cluster | ❌ | ❌ VM | ✅ | ✅ |
| No runtime / garbage collector | ❌ | ❌ | ✅ | ✅ |
| Safe for medical/bank/NASA data | ❌ | ⚠️ | ❌ | ✅ |

No other language satisfies all six requirements simultaneously.

---

## 10. Ecosystem Flywheel

Alloy can be a growth mechanism for the RusToK ecosystem:

```
User searches for a solution to their pain
          ↓
Finds Alloy (free, powerful)
          ↓
Data and integrations move to Rustok
          ↓
Leaving = starting from scratch (doesn't leave)
          ↓
Asks for a new integration
          ↓
Alloy generates → module goes to marketplace
          ↓
Marketplace attracts new users
          ↓
      (repeat)
```

More users → richer ecosystem → more users.

### Comparison with Competitors

| | N8N/Zapier | Airbyte | LangChain | Alloy |
|--|-----------|---------|-----------|-------|
| AI writes integration code | ❌ | ❌ | ⚠️ | ✅ |
| Self-heals on API changes | ❌ | ❌ | ❌ | ✅ |
| Script → native module | ❌ | ❌ | ❌ | ✅ |
| Understands dirty/broken data | ❌ | ⚠️ | ⚠️ | ✅ |
| Rust (speed + safety) | ❌ | ❌ | ❌ | ✅ |
| Creates own services | ❌ | ❌ | ❌ | ✅ |

> **Alloy creates a new category: Self-Evolving Integration Runtime (SEIR)**

---

## 11. Roadmap

### Phase 1 — Foundation (Q1 2026)

- ✅ Rhai engine with helpers (dates, encodings, PHP deserialization)
- ✅ CLI interface
- ✅ SQL dump parser
- ⬜ Presets: vBulletin, phpBB, WordPress
- ⬜ Basic MCP client
- ⬜ Schema Probe (auto-analysis of DB structure)
- ⬜ Dry Run mode
- ⬜ Rollback mechanism

### Phase 2 — AI Core (Q2 2026)

- Auto-generation of Rhai scripts from data samples
- Self-debugging: minor errors auto, logic — confirmation
- Audit trail for every operation
- AI test generation
- AI provider plugin system (Claude, GPT, Ollama)

### Phase 3 — Integration Runtime (Q3 2026)

- Event-driven architecture
- CDC (Change Data Capture): binlog/WAL
- Webhook handlers with self-healing
- HotTracker: monitoring usage patterns
- YAML/JSON configs as first-class interface

### Phase 4 — Native Compilation (Q4 2026)

- AI generates Rust code from Rhai script
- Pipeline: gen → review → cargo build → test → hot swap
- Module marketplace v1
- Web UI in RusToK admin and/or external host applications

### Phase 5 — Ecosystem (2027)

- Cloud SaaS (managed service, usage-based billing)
- SDK for embedding Alloy in third-party applications
- Enterprise contracts (banks, healthcare, government)
- Specialized presets: DICOM, SWIFT, COBOL, HDF5
- Edge deployment: native modules on IoT devices
- Federated marketplace

---

## 12. Architectural Decisions

All key decisions are made. This is the single source of truth for development.

| Decision | Choice | Rationale |
|----------|--------|-----------|
| License | Business Source License 1.1 with RusTok Additional Use Grant | Non-production allowed under BSL 1.1; production, including SaaS/hosted/white-label/competing use of the entire platform or substantial parts, is allowed when Total Finances is under USD $3 million, above threshold requires commercial license |
| AI transport | Plugin system (trait AiProvider) | No vendor lock-in on AI provider |
| Task interface | Text / YAML / API by context | AI generates YAML from text |
| Self-debugging | Auto for minor, confirmation for logic | Balance of autonomy and control |
| "Hotness" determination | AI based on usage patterns | Smart decision, not a hard threshold |
| Compilation Rhai → Rust | AI generates Rust code | Rhai is dynamic, idiomatic Rust needed |
| Preset | Reverse engineering of a live system | Not field mapping — adapter for a specific installation |
| Sandbox | FS❌ HTTP-whitelist❌ Proc❌ Mem-limit✅ | Full isolation, external only through explicit API |
| API module registration | Core traits (compile-time) | Type safety, no overhead, new module = new tools |
| Script storage | DB (source of truth) + files (version control) | Best of both worlds |
| Admin UI | Section inside RusToK admin or separate host UI | Alloy not tied to a single shell |
| MCP transport | `rustok-mcp` as RusToK adapter over `rmcp` | Alloy gets governed AI ↔ platform bridge without embedding MCP in core |

---

## Summary

```
⚗️ ALLOY
```

AI writes executable code for any data-related task.
Rust executes it with maximum speed and guaranteed safety.
The system debugs errors itself and compiles stable scenarios into native modules.
Infinite integrations. Without programmers. Self-healing.
**From a personal blog to NASA servers.**

---

*⚗️ Reshape. Integrate. Evolve.*

---

## About the Name

Any descriptive name for Alloy is a crutch.

- "Data migrator" — only migration
- "Integrator" — only integrations
- "ETL on steroids" — only data
- "No-code automation" — only automation
- "Self-Evolving Integration Runtime" — clever, but dead

Alloy does all of this simultaneously — plus creates services from scratch, plus generates modules, plus self-heals, plus evolves. Any name describes only one facet and diminishes the whole.

It's like trying to name electricity. "Light source"? One-sided. "Engine"? Same. Electricity is just electricity. Everyone knows you can do everything with it.

The best tools are named simply: **Rust. Go. Rails. Stripe.** One word. No explanations in the name. Brand is built by deeds, not descriptions.

**Alloy is simply Alloy.**

> *Where there is data — there Alloy works.*
