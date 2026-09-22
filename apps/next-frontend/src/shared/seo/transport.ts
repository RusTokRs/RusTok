import { resolveLocale } from "../../i18n";
import { getStorefrontTenantSlug } from "@/shared/api/modules";

import type { SeoPageContext } from "./metadata";

const FALLBACK_API_BASE_URL =
  process.env.NEXT_PUBLIC_API_URL ??
  process.env.RUSTOK_API_URL ??
  "http://localhost:5150";

const SEO_PAGE_CONTEXT_QUERY = `
  query SeoPageContext($route: String!, $locale: String) {
    seoPageContext(route: $route, locale: $locale) {
      route {
        targetKind
        targetId
        requestedLocale
        effectiveLocale
        canonicalUrl
        redirect {
          targetUrl
          statusCode
        }
        alternates {
          locale
          href
          xDefault
        }
      }
      document {
        title
        description
        robots {
          index
          follow
          noarchive
          nosnippet
          noimageindex
          notranslate
          maxSnippet
          maxImagePreview
          maxVideoPreview
          custom
        }
        openGraph {
          title
          description
          kind
          siteName
          url
          locale
          images {
            url
            alt
            width
            height
            mimeType
          }
        }
        twitter {
          card
          title
          description
          site
          creator
          images {
            url
            alt
            width
            height
            mimeType
          }
        }
        verification {
          google
          yandex
          yahoo
          other {
            name
            value
          }
        }
        pagination {
          prevUrl
          nextUrl
        }
        structuredDataBlocks {
          id
          schemaKind
          schemaType
          kind
          source
          payload
        }
        metaTags {
          name
          property
          httpEquiv
          content
        }
        linkTags {
          rel
          href
          hreflang
          media
          mimeType
          title
        }
      }
    }
  }
`;

export type SeoTransportErrorCode =
  | "BAD_USER_INPUT"
  | "PERMISSION_DENIED"
  | "NOT_FOUND"
  | "UNAUTHENTICATED"
  | "INTERNAL_ERROR"
  | "HTTP_ERROR"
  | "TRANSPORT_ERROR";

export class SeoTransportError extends Error {
  public readonly code: SeoTransportErrorCode;
  public readonly cause?: unknown;

  constructor(message: string, code: SeoTransportErrorCode, cause?: unknown) {
    super(message);
    this.name = "SeoTransportError";
    this.code = code;
    this.cause = cause;
  }
}

type SeoTransportOptions = {
  locale?: string | null;
  route: string;
  tenantSlug?: string | null;
  apiBaseUrl?: string;
  graphqlUrl?: string;
  preferRest?: boolean;
};

type SeoTransportResult = {
  context: SeoPageContext | null;
  transport: "rest" | "graphql";
};

type GraphqlLikeErrorRecord = {
  message?: string;
  extensions?: {
    code?: string;
  };
};

type SeoRestErrorEnvelope = {
  errors?: GraphqlLikeErrorRecord[];
  error?: {
    code?: string;
    message?: string;
  };
  message?: string;
};

type SeoPageContextGraphqlResponse = {
  seoPageContext: SeoPageContext | null;
};

type GraphqlResponse<T> = {
  data?: T;
  errors?: GraphqlLikeErrorRecord[];
};

function resolveApiBaseUrl(explicit?: string): string {
  return (explicit ?? FALLBACK_API_BASE_URL).replace(/\/$/, "");
}

function resolveGraphqlUrl(apiBaseUrl: string, explicit?: string): string {
  return explicit ?? `${apiBaseUrl}/api/graphql`;
}

function toCamelKey(value: string): string {
  return value.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

function camelize<T>(value: unknown): T {
  if (Array.isArray(value)) {
    return value.map((item) => camelize(item)) as T;
  }
  if (value && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>).reduce(
      (acc, [key, current]) => {
        acc[toCamelKey(key)] = camelize(current);
        return acc;
      },
      {} as Record<string, unknown>,
    ) as T;
  }
  return value as T;
}

function statusCodeToGraphqlCode(status: number): SeoTransportErrorCode {
  if (status === 400) return "BAD_USER_INPUT";
  if (status === 401) return "UNAUTHENTICATED";
  if (status === 403) return "PERMISSION_DENIED";
  if (status === 404) return "NOT_FOUND";
  if (status >= 500) return "INTERNAL_ERROR";
  return "HTTP_ERROR";
}

function normalizeGraphqlCode(code?: string): SeoTransportErrorCode {
  if (code === "BAD_USER_INPUT") return "BAD_USER_INPUT";
  if (code === "PERMISSION_DENIED") return "PERMISSION_DENIED";
  if (code === "NOT_FOUND") return "NOT_FOUND";
  if (code === "UNAUTHENTICATED" || code === "UNAUTHORIZED") {
    return "UNAUTHENTICATED";
  }
  if (code === "INTERNAL_ERROR") return "INTERNAL_ERROR";
  return "HTTP_ERROR";
}

function parseSeoRestErrorPayload(payload: unknown): GraphqlLikeErrorRecord | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }

  const envelope = payload as SeoRestErrorEnvelope;
  if (envelope.errors?.length) {
    return envelope.errors[0];
  }

  if (envelope.error) {
    return {
      message: envelope.error.message,
      extensions: {
        code: envelope.error.code,
      },
    };
  }

  if (envelope.message) {
    return {
      message: envelope.message,
    };
  }

  return null;
}

function shouldFallbackToGraphql(error: unknown): boolean {
  if (!(error instanceof SeoTransportError)) {
    return true;
  }

  return (
    error.code === "NOT_FOUND" ||
    error.code === "HTTP_ERROR" ||
    error.code === "INTERNAL_ERROR" ||
    error.code === "TRANSPORT_ERROR"
  );
}

function normalizeRoute(route: string): string {
  const normalized = route.trim();
  if (!normalized) {
    throw new SeoTransportError(
      "SEO route cannot be empty",
      "BAD_USER_INPUT",
    );
  }
  if (normalized.startsWith("/")) {
    return normalized;
  }
  return `/${normalized}`;
}

function buildSeoHeaders(
  locale: string,
  tenantSlug: string | null,
): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: "application/json",
  };

  if (tenantSlug) {
    headers["X-Tenant-Slug"] = tenantSlug;
  }

  if (locale) {
    headers["Accept-Language"] = locale;
  }

  return headers;
}

async function buildSeoRestError(response: Response): Promise<SeoTransportError> {
  let payload: unknown;

  try {
    payload = await response.json();
  } catch {
    payload = null;
  }

  const parsed = parseSeoRestErrorPayload(payload);
  const code = normalizeGraphqlCode(parsed?.extensions?.code);
  const normalizedMessage = parsed?.message?.trim();
  const fallbackCode = statusCodeToGraphqlCode(response.status);

  return new SeoTransportError(
    normalizedMessage && normalizedMessage.length > 0
      ? normalizedMessage
      : `SEO REST request failed with ${response.status}`,
    parsed ? code : fallbackCode,
  );
}

async function fetchSeoPageContextRest(
  normalizedLocale: string,
  route: string,
  tenantSlug: string | null,
  apiBaseUrl: string,
): Promise<SeoPageContext | null> {
  const url = new URL("/api/seo/page-context", apiBaseUrl);
  url.searchParams.set("route", route);

  let response: Response;
  try {
    response = await fetch(url.toString(), {
      method: "GET",
      headers: buildSeoHeaders(normalizedLocale, tenantSlug),
      cache: "no-store",
    });
  } catch (error) {
    throw new SeoTransportError(
      "SEO REST request failed before receiving a response",
      "TRANSPORT_ERROR",
      error,
    );
  }

  if (!response.ok) {
    throw await buildSeoRestError(response);
  }

  const payload = await response.json();
  return camelize<SeoPageContext>(payload);
}

async function fetchSeoPageContextGraphql(
  options: SeoTransportOptions,
  normalizedLocale: string,
  route: string,
  tenantSlug: string | null,
  apiBaseUrl: string,
): Promise<SeoPageContext | null> {
  const requestBody = {
    query: SEO_PAGE_CONTEXT_QUERY,
    variables: {
      route,
      locale: normalizedLocale,
    },
  };

  const graphqlUrl = resolveGraphqlUrl(apiBaseUrl, options.graphqlUrl);

  let response: Response;
  try {
    response = await fetch(graphqlUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...buildSeoHeaders(normalizedLocale, tenantSlug),
      },
      body: JSON.stringify(requestBody),
      cache: "no-store",
    });
  } catch (error) {
    throw new SeoTransportError(
      "SEO GraphQL request failed before receiving a response",
      "TRANSPORT_ERROR",
      error,
    );
  }

  if (!response.ok) {
    throw new SeoTransportError(
      `SEO GraphQL request failed with ${response.status}`,
      statusCodeToGraphqlCode(response.status),
    );
  }

  const payload = (await response.json()) as GraphqlResponse<SeoPageContextGraphqlResponse>;

  if (payload.errors?.length) {
    const firstError = payload.errors[0];
    const normalizedMessage = firstError.message?.trim();
    throw new SeoTransportError(
      normalizedMessage && normalizedMessage.length > 0
        ? normalizedMessage
        : "SEO GraphQL request failed",
      normalizeGraphqlCode(firstError.extensions?.code),
    );
  }

  if (!payload.data) {
    throw new SeoTransportError(
      "SEO GraphQL returned no data",
      "INTERNAL_ERROR",
    );
  }

  return payload.data.seoPageContext;
}

export async function fetchSeoPageContext(
  options: SeoTransportOptions,
): Promise<SeoTransportResult> {
  const normalizedLocale = resolveLocale(options.locale);
  const route = normalizeRoute(options.route);
  const tenantSlug = options.tenantSlug ?? getStorefrontTenantSlug();
  const apiBaseUrl = resolveApiBaseUrl(options.apiBaseUrl);

  if (options.preferRest !== false) {
    try {
      const context = await fetchSeoPageContextRest(
        normalizedLocale,
        route,
        tenantSlug,
        apiBaseUrl,
      );
      return {
        context,
        transport: "rest",
      };
    } catch (error) {
      if (!shouldFallbackToGraphql(error)) {
        throw error;
      }
    }
  }

  const context = await fetchSeoPageContextGraphql(
    options,
    normalizedLocale,
    route,
    tenantSlug,
    apiBaseUrl,
  );

  return {
    context,
    transport: "graphql",
  };
}
