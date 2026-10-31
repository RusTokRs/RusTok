# apps/next-admin/packages/forum / CRATE_API

## Purpose

`@rustok/forum-admin` (`apps/next-admin/packages/forum`) is the Next.js administration UI package for the `rustok-forum` module. It provides full community discussion moderation workspaces, category tree operations, topic authoring, reply drafting, and restructuring mechanisms (merge, fork, split, range move, slug rename).

## Architecture & Responsibilities

- **Module Registration:** Registers the forum module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`forumNav`) under `modulePlugins` with granular RBAC permission guards (`forum_categories:manage`, `forum_topics:manage`, `forum_topics:update`).
- **Category Tree Administration:** Exposes `CategoryTreeAdmin` and `CategoryEditorDialog` supporting nested categories, drag-and-drop reordering, topic posting policies, and subtree archive/restore actions.
- **Editorial & Reply Authoring:** Implements `ForumTopicEditor` and `ForumReplyEditor` integrated with the RusToK rich text engine (`@rustok/richtext`), draft preservation, and double-submit prevention.
- **Advanced Topic Restructuring:** Implements specialized workflows for thread refactoring:
  - `ForumTopicMerge`: Merges threads while preserving reply chronology.
  - `ForumTopicFork`: Promotes a reply branch into a standalone topic.
  - `ForumTopicReplyRange`: Transplants a contiguous range of replies across topics.
  - `ForumTopicSplit`: Splits topic discussions based on selected reply boundaries.
  - `ForumTopicSlugRename`: Safely updates topic URL slugs.
- **Fluid Frontend Architecture (FFA):** Framework-agnostic command validation, identity generation, and business rules live in `src/core/` (`identity.ts`, `topic-fork.ts`, `topic-merge.ts`, `topic-reply-range.ts`, `topic-slug-rename.ts`, `topic-split.ts`), fully verifiable via pure Node.js tests.
- **GraphQL Integration:** Directly dispatches typed queries (`forumTopics`, `forumTopic`, `forumTopicReplies`, `forumCategories`) and mutations (`createForumTopic`, `createForumReply`, `mergeForumTopics`, `forkForumTopicReplyBranch`, `moveForumTopicReplyRange`, etc.).

## Public Surface

- `forumNav`: Admin sidebar navigation configuration with 8 dedicated sub-routes.
- `ForumReplyEditor`: Rich-text reply composition component.
- `ForumTopicEditor`: Rich-text topic authoring and metadata form.
- `ForumTopicFork`: Branch extraction workflow component.
- `ForumTopicMerge`: Topic consolidation workflow component.
- `ForumTopicReplyRange`: Contiguous reply range transfer component.
- `ForumTopicSlugRename`: URL slug modification component.
- `ForumTopicSplit`: Discussion partition component.
- `CategoryTreeAdmin`: Category tree visualization and management workspace.
- `CategoryEditorDialog`: Create/edit dialog for forum categories.
- `core/*`: Pure domain models, identity generators, and command builders (`isForumUuid`, `newForumUuid`, `buildForumTopicForkCommand`, `buildForumTopicMergeCommand`, `buildForumReplyRangeMoveCommand`, `buildForumTopicSlugRenameCommand`, `buildForumTopicSplitCommand`).
- `api/*`: Transport functions for category CRUD, topic moderation (pin, lock, close, reopen, delete, restore), reply moderation (approve, reject, delete, restore), and discussion operations.

## Invariants & Guarantees

1. **RBAC Permission Enforcement:** All destructive or administrative routes require explicit permissions (`forum_categories:manage`, `forum_topics:manage`, `forum_topics:update`).
2. **Idempotent Topic Restructuring:** Restructuring commands generate unique `operationId` UUIDs ensuring at-most-once execution on the backend outbox and stream engine.
3. **No Self-Targeting:** Topic merge, fork, and range moves disallow source and target topic identity equality (`sourceTopicId !== targetTopicId`).
4. **Draft Safety:** In-flight reply and topic drafts are guarded against accidental erasure on transient network or validation failures.
5. **Parity with Leptos Admin:** Category management, topic lifecycle actions, and discussion restructuring maintain functional parity with `crates/modules/rustok-forum/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `next/navigation`, `react-hook-form`, `zod`, `sonner`, `@rustok/richtext`, and `@/shared/ui/shadcn` design system components from `apps/next-admin`.
- **Backend API:** Communicates exclusively with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Table Access:** Never queries forum database tables or repositories directly.

