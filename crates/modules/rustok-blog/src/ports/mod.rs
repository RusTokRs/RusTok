//! Blog cross-boundary ports and orchestration bridge types.
//!
//! This module is the crate's **one deliberate public entity surface**: it re-exports the four
//! Blog entity modules that `rustok-content-orchestration` needs for its in-transaction
//! promote/demote/tag-sync bridge. That bridge writes Blog rows inside the caller's transaction,
//! so it cannot go through `PostService`, whose commands own their transactions.
//!
//! Every other entity stays crate-private (`src/entities/mod.rs`), and this set is pinned by
//! `verify-module-reference-contract.mjs` so the surface cannot widen silently. The documented
//! exception is recorded as audit finding L-4 in
//! `docs/implementation-plan-current.md`; moving the bridge onto owner in-transaction commands is
//! the open half of that item.

pub use crate::entities::{blog_category, blog_post, blog_post_tag, blog_post_translation};
