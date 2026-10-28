import type { BlogPublicComment } from '../api/posts';

export interface DisplayedComment {
  comment: BlogPublicComment;
  depth: number;
}

/**
 * Public comments can be paginated independently of their parents.
 * Always render every fetched comment, even when the parent is on a different
 * page; preserve all reachable nesting within the currently fetched page.
 */
export function orderThreadedComments(items: BlogPublicComment[]): DisplayedComment[] {
  const ids = new Set(items.map((item) => item.id));
  const children = new Map<string, BlogPublicComment[]>();
  const roots: BlogPublicComment[] = [];

  for (const comment of items) {
    const parent = comment.parentCommentId;
    if (!parent || !ids.has(parent) || parent === comment.id) {
      roots.push(comment);
    } else {
      const replies = children.get(parent) ?? [];
      replies.push(comment);
      children.set(parent, replies);
    }
  }

  const result: DisplayedComment[] = [];
  const seen = new Set<string>();
  const visit = (comment: BlogPublicComment, depth: number) => {
    if (seen.has(comment.id)) return;
    seen.add(comment.id);
    result.push({ comment, depth });
    children.get(comment.id)?.forEach((reply) => visit(reply, depth + 1));
  };
  roots.forEach((root) => visit(root, 0));
  // Defend against pre-existing cycles or malformed projections: never hide a row.
  items.forEach((comment) => visit(comment, 0));
  return result;
}

