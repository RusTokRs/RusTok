'use client';

/**
 * Reply deep links are authored by the forum module: the notification fan-out emits
 * `/modules/forum?category=…&topic=…&reply=<uuid>` and the search projection appends
 * `?reply=<uuid>` to the canonical topic route. The value therefore arrives from a URL and is
 * untrusted input, so it is validated as a UUID before it is interpolated into a DOM selector, and
 * it is resolved against the post each thread renderer tags with
 * `data-forum-post data-target-kind="REPLY" data-target-id`.
 */
const FORUM_REPLY_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

const FOCUS_HIGHLIGHT_MS = 2600;

/**
 * Thread replies are cursor-paged, so a linked post can sit past the page the surface loaded.
 * Forward pages are walked through the same owner read, bounded so one view cannot turn into an
 * unbounded fetch loop; a post outside the bound is reported instead of guessed.
 */
export const MAX_ANCHOR_REPLY_PAGES = 20;

export function isForumReplyId(value: string | null | undefined): value is string {
  return typeof value === 'string' && FORUM_REPLY_ID_PATTERN.test(value);
}

/**
 * Scrolls the linked post into view and marks it for a moment. Returns a cleanup callback when the
 * post is rendered, or `null` when the id is unusable or the post is not in the document.
 */
export function focusForumReplyPost(replyId: string): (() => void) | null {
  if (!isForumReplyId(replyId)) {
    return null;
  }
  const target = document.querySelector<HTMLElement>(
    `[data-forum-post][data-target-kind="REPLY"][data-target-id="${replyId}"]`
  );
  if (!target) {
    return null;
  }
  target.scrollIntoView({ behavior: 'smooth', block: 'center' });
  target.dataset.focused = 'true';
  const timer = window.setTimeout(() => {
    delete target.dataset.focused;
  }, FOCUS_HIGHLIGHT_MS);
  return () => window.clearTimeout(timer);
}
