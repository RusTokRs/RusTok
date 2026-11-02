# @rustok/comments-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/comments-frontend` (`apps/next-frontend/packages/rustok-comments`) provides the reusable, domain-agnostic storefront comment authoring component for RusToK. It decouples comment authoring (rich text editing, validation, busy state management, and command idempotency) from target domain models (Blog, Product Reviews, Forum).

### Public Surface (`src/index.ts`)
- **Components:**
  - `CommentComposer`: Interactive rich text comment authoring form.
- **Props:**
  - `contentLocale: string`: Language code for the document content.
  - `canSubmit: boolean`: Authentication gate; renders sign-in notice when false.
  - `onSubmit: (document: RichTextDocument, commandId: string) => Promise<void>`: Target-bound async mutation dispatcher.

---

## 2. Key Contracts, Routes & Layouts

### Component Architecture
- **Rich Text Profile:** Configured strictly with `profile="comment"` (not the larger `discussion` profile), targeting the isolated frame `/richtext/frame`.
- **Idempotency Lifecycle:**
  - A new command ID (`crypto.randomUUID()`) is minted upon first submission.
  - If submission fails, the same `commandId` is retained for retry to ensure idempotency.
  - If the user modifies the draft document (`onChange`), the retained `commandId` is immediately invalidated (`setCommandId(null)`).
- **Validation**:
  - Rejects empty rich text documents (`!richTextDocumentHasText(document)`) before triggering `onSubmit`.
  - Clears previous error banners upon user typing.

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Never accepts backend URLs, GraphQL clients, `target_type`, `target_id`, or tenant IDs directly.
- Consuming modules (e.g. `@rustok/blog-frontend`) wrap `CommentComposer` and supply an owner-bound `onSubmit` callback.
- Communicates with `@rustok/richtext` over postMessage in an isolated iframe.

---

## 4. Verification & Testing Evidence

- **Unit Tests**: `comment-composer.test.mjs` verifies idempotency lifecycle (retry reuse and edit reset), empty document validation, and auth gate behavior.
- **Next.js Host Typecheck**: `npm run typecheck:next-frontend` completes with 0 errors.
- **Next.js Host Linter**: `npm run lint` passes with 0 warnings.

---

## 5. Anti-Patterns & Operational Invariants

1. **No Ad-Hoc Domain Couplings**: Never inject module-specific parameters (`postId`, `productId`) into `CommentComposer`; bind them in the parent module's `onSubmit`.
2. **No Command ID Reuse Across Content Changes**: A modified draft must receive a fresh command ID upon submission.
3. **Never Submit Empty Documents**: White-space only or empty rich text documents must be rejected locally before dispatching network requests.
4. **Isolated Rich Text Profile**: Comment composer must always use `profile="comment"`.
