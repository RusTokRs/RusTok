'use client';

import { useDeferredValue, useEffectEvent } from 'react';
import * as React from 'react';
import { useKBar, useRegisterActions } from 'kbar';
import { useRouter } from 'next/navigation';
import { useSession } from 'next-auth/react';
import { graphqlRequest } from '@/shared/api/graphql';

const ADMIN_GLOBAL_SEARCH_QUERY = `
  query AdminGlobalSearch($input: SearchPreviewInput!) {
    adminGlobalSearch(input: $input) {
      queryLogId
      total
      tookMs
      engine
      items {
        id
        entityType
        sourceModule
        title
        snippet
        score
        locale
        url
        payload
      }
    }
  }
`;

const TRACK_SEARCH_CLICK_MUTATION = `
  mutation TrackSearchClick($input: TrackSearchClickInput!) {
    trackSearchClick(input: $input) {
      success
      tracked
    }
  }
`;

type AdminGlobalSearchItem = {
  id: string;
  entityType: string;
  sourceModule: string;
  title: string;
  snippet: string | null;
  score: number;
  locale: string | null;
  url: string | null;
  payload: string;
};

type AdminGlobalSearchResponse = {
  adminGlobalSearch: {
    queryLogId: string | null;
    items: AdminGlobalSearchItem[];
  };
};

type TrackSearchClickResponse = {
  trackSearchClick: {
    success: boolean;
    tracked: boolean;
  };
};

function parsePayload(payload: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(payload);
    return parsed && typeof parsed === 'object' ? parsed : null;
  } catch {
    return null;
  }
}

function buildSearchFallbackHref(
  query: string,
  item?: AdminGlobalSearchItem
): string {
  const params = new URLSearchParams();
  if (query.trim().length > 0) {
    params.set('q', query.trim());
  }
  if (item) {
    params.set('focusId', item.id);
    params.set('entityType', item.entityType);
    params.set('sourceModule', item.sourceModule);
  }

  const encoded = params.toString();
  return encoded.length > 0
    ? `/dashboard/search?${encoded}`
    : '/dashboard/search';
}

function resolveAdminHref(query: string, item: AdminGlobalSearchItem): string {
  const payload = parsePayload(item.payload);

  if (item.entityType === 'product') {
    return `/dashboard/product/${item.id}`;
  }

  if (item.entityType === 'node' && item.sourceModule === 'blog') {
    return `/dashboard/blog/${item.id}/edit`;
  }

  if (item.entityType === 'node' && item.sourceModule === 'forum') {
    return '/dashboard/forum/reply';
  }

  if (
    payload &&
    typeof payload.slug === 'string' &&
    item.sourceModule === 'pages'
  ) {
    return buildSearchFallbackHref(query, item);
  }

  return buildSearchFallbackHref(query, item);
}

export default function AdminGlobalSearchActions() {
  const router = useRouter();
  const { data: session } = useSession();
  const { searchQuery, currentRootActionId } = useKBar((state) => ({
    searchQuery: state.searchQuery,
    currentRootActionId: state.currentRootActionId
  }));
  const deferredSearch = useDeferredValue(searchQuery.trim());
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  const [results, setResults] = React.useState<AdminGlobalSearchItem[]>([]);
  const [queryLogId, setQueryLogId] = React.useState<string | null>(null);
  const requestSeq = React.useRef(0);

  const resetSearchState = React.useCallback(() => {
    setResults((current) => (current.length === 0 ? current : []));
    setQueryLogId((current) => (current === null ? current : null));
  }, []);

  const fetchSearchResults = useEffectEvent(async (query: string) => {
    if (!token || !tenantSlug) {
      return null;
    }

    return graphqlRequest<
      {
        input: {
          query: string;
          limit: number;
          offset: number;
        };
      },
      AdminGlobalSearchResponse
    >(
      ADMIN_GLOBAL_SEARCH_QUERY,
      {
        input: {
          query,
          limit: 8,
          offset: 0
        }
      },
      token,
      tenantSlug
    );
  });

  const trackClick = useEffectEvent(
    async (item: AdminGlobalSearchItem, href: string) => {
      if (!token || !tenantSlug || !queryLogId) {
        return;
      }

      try {
        await graphqlRequest<
          {
            input: {
              queryLogId: string;
              documentId: string;
              position: number;
              href: string;
            };
          },
          TrackSearchClickResponse
        >(
          TRACK_SEARCH_CLICK_MUTATION,
          {
            input: {
              queryLogId,
              documentId: item.id,
              position:
                results.findIndex((candidate) => candidate.id === item.id) + 1,
              href
            }
          },
          token,
          tenantSlug
        );
      } catch {
        // Click tracking is best-effort and must not block navigation.
      }
    }
  );

  React.useEffect(() => {
    if (currentRootActionId || deferredSearch.length < 2) {
      resetSearchState();
      return;
    }

    const currentRequest = requestSeq.current + 1;
    requestSeq.current = currentRequest;

    void (async () => {
      try {
        const data = await fetchSearchResults(deferredSearch);
        if (!data || requestSeq.current !== currentRequest) {
          return;
        }

        setResults(data.adminGlobalSearch.items);
        setQueryLogId(data.adminGlobalSearch.queryLogId);
      } catch {
        if (requestSeq.current === currentRequest) {
          resetSearchState();
        }
      }
    })();
  }, [currentRootActionId, deferredSearch, fetchSearchResults, resetSearchState]);

  const actions = React.useMemo(() => {
    if (currentRootActionId || deferredSearch.length < 2) {
      return [];
    }

    const dynamicActions = results.map((item) => {
      const href = resolveAdminHref(deferredSearch, item);

      return {
        id: `admin-search-${item.sourceModule}-${item.entityType}-${item.id}-${item.locale ?? 'default'}`,
        name: item.title,
        keywords: [
          item.title,
          item.sourceModule,
          item.entityType,
          item.locale ?? ''
        ]
          .filter(Boolean)
          .join(' '),
        section: 'Search',
        subtitle:
          item.snippet ??
          `${item.sourceModule} | ${item.entityType}${item.locale ? ` | ${item.locale}` : ''}`,
        perform: () => {
          void trackClick(item, href);
          router.push(href);
        }
      };
    });

    return [
      {
        id: `open-search-control-plane-${deferredSearch}`,
        name: `Open Search for "${deferredSearch}"`,
        keywords: `${deferredSearch} search control plane`,
        section: 'Search',
        subtitle: 'Open the full search control plane with this query',
        perform: () => router.push(buildSearchFallbackHref(deferredSearch))
      },
      ...dynamicActions
    ];
  }, [currentRootActionId, deferredSearch, results, router, trackClick]);

  useRegisterActions(actions, [actions]);

  return null;
}
