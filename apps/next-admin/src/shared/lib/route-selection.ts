import { SearchParams } from 'nuqs/server';

export function readRouteSelection(
  searchParams: SearchParams,
  key: string
): string | undefined {
  const raw = searchParams[key];
  if (typeof raw !== 'string') {
    return undefined;
  }

  const trimmed = raw.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

export function listRouteQueryEntries(
  searchParams: SearchParams,
  excludedKeys: string[] = []
): Array<[string, string]> {
  const excluded = new Set(excludedKeys);

  return Object.entries(searchParams).flatMap(([key, raw]) => {
    if (excluded.has(key)) {
      return [];
    }

    if (typeof raw !== 'string') {
      return [];
    }

    const trimmed = raw.trim();
    return trimmed.length > 0 ? [[key, trimmed] as [string, string]] : [];
  });
}

export function buildRouteSelectionHref(
  pathname: string,
  searchParams: SearchParams,
  key: string,
  value: string
): string {
  const params = new URLSearchParams(
    listRouteQueryEntries(searchParams, [key])
  );
  params.set(key, value);
  return `${pathname}?${params.toString()}`;
}
