# RusToK Domain Hygiene & Invariant Verification Report

- **Target:** `crates: translation`
- **Files Scanned:** 86
- **Total Issues:** 15 (🔴 Critical: 15, 🟡 High: 0, 🟢 Medium: 0)

## Module Scorecard

| Module | P0 Critical | P1 High | P2 Medium | Total | Status |
|---|---|---|---|---|---|
| `rustok-translation` | 15 | 0 | 0 | 15 | 🔴 Action Required |

## Discovered Findings by Pattern

### `[P0-CRITICAL]` P-TXN-ROLLBACK-SWALLOW: Transaction Rollback Swallowing (15 occurrence(s))

| File | Line | Snippet | Recommendation |
|---|---|---|---|
| `crates/modules/rustok-translation/src/collaboration.rs:269` | 269 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/inventory.rs:199` | 199 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/inventory.rs:289` | 289 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/inventory.rs:464` | 464 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:497` | 497 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:639` | 639 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:739` | 739 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:874` | 874 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:1049` | 1049 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:1180` | 1180 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:1312` | 1312 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:1457` | 1457 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:1851` | 1851 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:1873` | 1873 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |
| `crates/modules/rustok-translation/src/workflow.rs:2646` | 2646 | `let _ = transaction.rollback().await;` | if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, "transaction rollback failed"); } |

---
Canonical Standards Reference: [`docs/standards/logic-hygiene-patterns.md`](docs/standards/logic-hygiene-patterns.md)