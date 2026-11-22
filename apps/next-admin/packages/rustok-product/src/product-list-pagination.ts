/**
 * Product owner pagination bounds shared by the module transport and the Next
 * server route. GraphQL Int page indexes must fit the signed 32-bit domain;
 * the Product catalog read port accepts at most 100 products per page.
 */
export const MAX_PRODUCT_ADMIN_PAGE = 2_147_483_647;
export const MAX_PRODUCT_ADMIN_PAGE_SIZE = 100;
export const DEFAULT_PRODUCT_ADMIN_PAGE_SIZE = 20;

function normalizedPageNumber(
  value: string | number | null | undefined,
  fallback: number,
  maximum: number
): number {
  if (value == null) return fallback;
  if (typeof value === 'string' && !/^\d+$/.test(value.trim())) {
    return fallback;
  }
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return fallback;
  }
  if (numeric > maximum) {
    return maximum;
  }
  if (!Number.isSafeInteger(numeric) || numeric <= 0) {
    return fallback;
  }
  return numeric;
}

export function normalizeProductAdminPagination(input: {
  page?: number | string | null;
  perPage?: number | string | null;
}): { page: number; perPage: number } {
  return {
    page: normalizedPageNumber(input.page, 1, MAX_PRODUCT_ADMIN_PAGE),
    perPage: normalizedPageNumber(
      input.perPage,
      DEFAULT_PRODUCT_ADMIN_PAGE_SIZE,
      MAX_PRODUCT_ADMIN_PAGE_SIZE
    )
  };
}
