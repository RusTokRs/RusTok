# FORUM-15E storefront member-card UI actualization — 2026-08-10

Status: `source-ready / storefront-presentation / privacy-filtered-lookup / no-new-owner-reads / media-avatar-resolution-open / runtime-evidence-open`

## Fresh cursor

This slice started from `main@3b1ba79619a9c37f7bc90fb773843c7287d2d4ff`. The one commit after FORUM-15D changed Commerce-only sources and did not overlap Forum storefront composition.

During the final pre-merge gate, `main` advanced to `d7d6044c9e6bddf42f0ea4fa03ac6ba376873ba9` through Index contract CI work only. That movement did not touch Forum storefront files, and the exact FORUM-15E file set was replayed onto the new base before the final `behind 0` gate.

FORUM-15 remains `in_progress`. FORUM-15D carried bounded, privacy-aware member-card data through both real Forum storefront transports, but the Leptos storefront still explicitly ignored `StorefrontForumData.member_cards`.

FORUM-15E consumes that already-loaded payload in the real storefront presentation. It adds no owner read, GraphQL request, server function or package dependency.

## Presentation boundary

The Forum storefront continues to own only its local presentation DTO. Profiles remains authoritative for handle, display name, tags, preferred locale, avatar Media identity and profile privacy/block admission. Forum remains authoritative only for its topic/reply/solution counters.

The UI does not query Profiles, Media or Forum statistics. It receives only the member cards admitted by the FORUM-15B/15C/15D composition path.

`member_cards` are converted once per `ForumShowcase` into an `Arc<HashMap<String, ForumMemberCard>>` and placed in the Leptos owner context. Topic-feed cards, the selected opening post and reply cards then perform only in-memory author-ID lookups.

This keeps the UI lookup cost bounded and prevents prop-threading or per-row transport reads.

## Privacy and block behavior

The presentation rule is deliberately fail-closed:

- a topic/reply `authorId` is never rendered directly;
- `ForumAuthorBadge` renders only when the corresponding user ID exists in the privacy-filtered member-card context;
- missing, blocked, hidden, anonymous-unavailable or permission-unavailable member cards produce no author presentation;
- Forum counters are shown only as part of an admitted member card, so a statistics row cannot manufacture a visible identity;
- the UI does not invent a fallback handle/name from the raw Forum author UUID.

Anonymous Forum content therefore remains readable exactly as in FORUM-15D while authenticated viewers with admitted member cards receive the richer presentation.

## Member-card surface

The same compact member-card component is used on all three intended storefront surfaces:

1. topic-feed rows;
2. the selected topic/opening post;
3. each visible reply.

An admitted card shows:

- Profiles-owned display name and handle;
- initials as the current avatar fallback;
- Forum-owned topic, reply and solution counts.

The statistics labels are localized in the existing English and Russian storefront locale files.

The component carries a stable `forum-member-card` class so retained browser evidence can target the actual presentation later without coupling to Tailwind utility details.

## Avatar / Media boundary

The 15D transport currently carries `avatarMediaId`, not a Media-owned presentation URL/alt payload. FORUM-15E intentionally does **not** turn that ID into a guessed URL, call Media per card, or add a direct Media dependency.

Instead the compact card uses a deterministic initials fallback derived from the already-admitted display name, with handle fallback when needed. Media-backed avatar rendering remains open until the storefront has an appropriate owner-provided presentation asset contract that preserves the same bounded composition.

## No-N+1 presentation shape

FORUM-15D already bounded one storefront snapshot to at most one member-card batch/service call. FORUM-15E adds no network/database work on top of it:

- one member-card vector becomes one `Arc<HashMap>`;
- each visible topic/reply presentation performs only a map lookup;
- the shared `Arc` is cloned through Leptos context rather than cloning the whole card map per row;
- there is no per-author `fetch`, GraphQL call, server function or owner-service call in UI source.

Retained runtime/query-count evidence is still open and is not claimed by this source slice.

## Compatibility and scope

Existing category/topic/reply content rendering, routing, unread state and mark-read behavior are unchanged. The UI integration is additive: when `member_cards` is empty, the existing Forum content remains visible with no author badge.

This slice does not:

- broaden `forumMemberCards` permissions;
- add a public statistics endpoint;
- migrate the legacy storefront reply read surface;
- add Profiles or Media owner dependencies to `rustok-forum-storefront`;
- claim browser/runtime privacy evidence;
- claim Media avatar lifecycle/presentation eligibility.

## Remaining FORUM-15 work

FORUM-15 stays `in_progress`. Remaining work is now primarily retained evidence and any owner-safe presentation refinement:

- browser/runtime evidence for admitted, hidden and blocked profile presentation on the real storefront;
- retained query-count evidence demonstrating the bounded dual-path composition under real host execution;
- Media-backed avatar presentation once an appropriate owner presentation contract is available;
- review whether any additional permitted Forum activity/reputation presentation is required without moving ownership into Forum.

The canonical ledger wording remains materially true and is not rewritten for this bounded UI slice.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database scenario, migration, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-storefront-member-card-ui-source.mjs
```
