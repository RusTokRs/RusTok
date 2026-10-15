import type { ForumCategoryListItem } from '../api/forum';

export interface CategoryWithChildren {
  category: ForumCategoryListItem;
  subcategories: Array<ForumCategoryListItem & { displayName: string }>;
  totalTopicCount: number;
  totalReplyCount: number;
}

/** Flatten every descendant under its root so the overview can render a
 * bounded category tree of any owner-supported depth as searchable pills. */
export function buildCategoryOverviewHierarchy(
  categories: ForumCategoryListItem[],
): CategoryWithChildren[] {
  const byId = new Map(categories.map((category) => [category.id, category]));
  const children = new Map<string, ForumCategoryListItem[]>();
  for (const category of categories) {
    if (!category.parentId || !byId.has(category.parentId)) continue;
    const siblings = children.get(category.parentId) ?? [];
    siblings.push(category);
    children.set(category.parentId, siblings);
  }

  const visited = new Set<string>();
  const group = (root: ForumCategoryListItem): CategoryWithChildren => {
    const subcategories: CategoryWithChildren['subcategories'] = [];
    visited.add(root.id);
    const collect = (parent: ForumCategoryListItem, path: string) => {
      for (const child of children.get(parent.id) ?? []) {
        if (visited.has(child.id)) continue;
        visited.add(child.id);
        const displayName = `${path} / ${child.name}`;
        subcategories.push({ ...child, displayName });
        collect(child, displayName);
      }
    };
    collect(root, root.name);
    return {
      category: root,
      subcategories,
      totalTopicCount: root.topicCount + subcategories.reduce((sum, item) => sum + item.topicCount, 0),
      totalReplyCount: root.replyCount + subcategories.reduce((sum, item) => sum + item.replyCount, 0),
    };
  };

  const roots = categories.filter((category) => !category.parentId || !byId.has(category.parentId));
  const result = roots.filter((root) => !visited.has(root.id)).map(group);
  // A malformed orphan/cycle must not silently remove an otherwise visible
  // category. The owner normally rejects cycles, but the widget stays bounded.
  for (const category of categories) {
    if (!visited.has(category.id)) result.push(group(category));
  }
  return result;
}

/** A root match means the entire visible branch matches. Otherwise retain
 * only descendant matches while keeping their root as navigation context. */
export function filterCategoryOverviewHierarchy(
  hierarchy: CategoryWithChildren[],
  searchQuery: string,
): CategoryWithChildren[] {
  const q = searchQuery.trim().toLowerCase();
  if (!q) return hierarchy;

  return hierarchy.flatMap((item) => {
    const root = item.category;
    const matchesRoot = root.name.toLowerCase().includes(q) ||
      root.slug.toLowerCase().includes(q) ||
      (root.description?.toLowerCase().includes(q) ?? false);
    if (matchesRoot) return [item];

    const subcategories = item.subcategories.filter((sub) =>
      sub.displayName.toLowerCase().includes(q) ||
      sub.slug.toLowerCase().includes(q) ||
      (sub.description?.toLowerCase().includes(q) ?? false)
    );
    return subcategories.length ? [{ ...item, subcategories }] : [];
  });
}
