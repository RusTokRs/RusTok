export interface CategoryHierarchyNode {
  id: string;
  parentId: string | null;
  code: string;
  slug: string;
  name: string;
}

export interface CategoryHierarchyOption {
  id: string;
  label: string;
  depth: number;
}

/**
 * Builds display paths from Taxonomy-owned parent links.
 *
 * Product's persisted `path` is only a navigation projection and can lag
 * behind a Taxonomy reparent. It must not determine the visible category tree.
 */
export function buildCategoryHierarchyOptions(
  categories: readonly CategoryHierarchyNode[]
): CategoryHierarchyOption[] {
  const categoriesById = new Map<string, CategoryHierarchyNode>();
  for (const category of categories) {
    if (!categoriesById.has(category.id)) {
      categoriesById.set(category.id, category);
    }
  }

  return categories.map((category) => {
    const segments: string[] = [];
    const visited = new Set<string>();
    let current: CategoryHierarchyNode | undefined = category;

    while (current && !visited.has(current.id)) {
      visited.add(current.id);
      const label =
        current.slug.trim() || current.name.trim() || current.code.trim();
      if (label) segments.push(label);
      current = current.parentId
        ? categoriesById.get(current.parentId)
        : undefined;
    }

    const path = segments.reverse();
    return {
      id: category.id,
      label: path.join(' / ') || category.code,
      depth: Math.max(0, path.length - 1)
    };
  });
}
