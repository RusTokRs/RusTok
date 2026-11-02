/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { normalizeLocaleTag } from "@rustok/next-fluent/utils";

import type {
  PricingChannelOption,
  PricingEffectivePrice,
  PricingProductDetail,
  PricingResolutionContext,
  PricingVariant,
  StorefrontPricingData,
  StorefrontPricingQuery,
} from "./types";

/** A UI intent key, not a price cache or reservation. Never log/persist it: it includes auth scope. */
export function pricingReadKey(
  query: StorefrontPricingQuery,
  tenant: string | null | undefined,
  token: string | null | undefined,
): string {
  return JSON.stringify([
    tenant ?? null,
    token ?? null,
    query.locale ?? null,
    query.selectedHandle ?? null,
    query.currencyCode ?? null,
    query.regionId ?? null,
    query.priceListId ?? null,
    query.channelId ?? null,
    query.channelSlug ?? null,
    typeof query.quantity === "number" && !Number.isFinite(query.quantity)
      ? String(query.quantity)
      : (query.quantity ?? null),
  ]);
}

/** Executor identity is opaque and never serialized; a host/provider switch is a new read scope. */
export interface PricingReadScope {
  key: string;
  executor: unknown;
}

export interface PricingSelection {
  scope: PricingReadScope;
  selectedHandle: string | null;
  context: PricingResolutionContext;
}

export function createPricingSelection(
  scope: PricingReadScope,
  data: StorefrontPricingData | null = null,
  query?: StorefrontPricingQuery,
): PricingSelection {
  return {
    scope,
    selectedHandle: query?.selectedHandle ?? data?.selectedHandle ?? null,
    context: query
      ? {
          currencyCode: query.currencyCode ?? "",
          quantity: query.quantity ?? 1,
          regionId: query.regionId,
          priceListId: query.priceListId,
          channelId: query.channelId,
          channelSlug: query.channelSlug,
        }
      : (data?.resolutionContext ?? { currencyCode: "", quantity: 1 }),
  };
}

function samePricingScope(a: PricingReadScope, b: PricingReadScope): boolean {
  return a.key === b.key && a.executor === b.executor;
}

/** Derive the new scope before its first IO, not in an effect after an old-selector request. */
export function pricingSelectionInScope(
  selection: PricingSelection,
  scope: PricingReadScope,
): PricingSelection {
  return samePricingScope(selection.scope, scope)
    ? selection
    : createPricingSelection(scope);
}

export function visiblePricingValue<T>(
  result: { scope: PricingReadScope; key: string; value: T | null },
  scope: PricingReadScope,
  key?: string,
): T | null {
  return samePricingScope(result.scope, scope) &&
    (key === undefined || result.key === key)
    ? result.value
    : null;
}

/** Inspection-only controls retained during a failed quote. Never retain effective prices here. */
export interface PricingDiscovery {
  availableChannels: StorefrontPricingData["availableChannels"];
  activePriceLists: StorefrontPricingData["activePriceLists"];
  currencies: string[];
}

export function pricingDiscovery(
  data: StorefrontPricingData | null,
): PricingDiscovery | null {
  if (!data) return null;
  return {
    availableChannels: data.availableChannels,
    activePriceLists: data.activePriceLists,
    currencies: [
      ...data.products.items.flatMap((product) => product.currencies),
      ...(data.selectedProduct?.variants.flatMap((variant) =>
        variant.prices.map((price) => price.currencyCode),
      ) ?? []),
    ],
  };
}

export function pricingContextQuery(
  context: PricingResolutionContext,
): StorefrontPricingQuery {
  return {
    currencyCode: context.currencyCode || undefined,
    regionId: context.regionId,
    priceListId: context.priceListId,
    channelId: context.channelId,
    channelSlug: context.channelSlug,
    // No currency means inspection, not a fabricated default-currency quantity quote.
    quantity: context.currencyCode ? context.quantity : undefined,
  };
}

export function updatePricingContext(
  current: PricingResolutionContext,
  update: Partial<PricingResolutionContext>,
  channels: PricingChannelOption[],
): PricingResolutionContext {
  const next = { ...current, ...update };
  if (Object.prototype.hasOwnProperty.call(update, "channelId")) {
    next.channelSlug =
      channels.find((channel) => channel.id === update.channelId)?.slug ?? null;
    // A previously selected list is not implicitly eligible for another channel.
    next.priceListId = null;
  }
  if (
    Object.prototype.hasOwnProperty.call(update, "currencyCode") &&
    next.currencyCode !== current.currencyCode
  ) {
    // Region currency is owner-constrained. Match the native currency links, which clear it.
    next.regionId = null;
  }
  if (update.currencyCode === "") {
    next.priceListId = null;
    next.regionId = null;
  }
  return next;
}

export function pricingCurrencies(
  discovery: PricingDiscovery | null,
  selected: string,
): string[] {
  const currencies = new Set(discovery?.currencies ?? []);
  if (selected) currencies.add(selected); // Retain explicit unpriced intent; do not select another currency.
  return [...currencies].sort();
}

export function pricingVariantView(
  variant: PricingVariant,
  context?: PricingResolutionContext | null,
) {
  const basePrice = context?.currencyCode
    ? (variant.prices.find(
        (price) => price.currencyCode === context.currencyCode,
      ) ?? null)
    : (variant.prices[0] ?? null);
  return {
    basePrice, // Separate inspection column only, never a substitute selling price.
    effective: variant.effectivePrice ?? null,
    onSale: variant.effectivePrice?.onSale === true,
  };
}

/** Canonical UUID syntax admission shared by price/SKU requests; identity remains owner-owned. */
export function parsePricingUuid(value: string): string {
  let normalized = value.trim().toLowerCase();
  if (normalized.startsWith("urn:uuid:")) normalized = normalized.slice(9);
  else if (normalized.startsWith("{") && normalized.endsWith("}"))
    normalized = normalized.slice(1, -1);
  if (/^[0-9a-f]{32}$/.test(normalized))
    normalized = `${normalized.slice(0, 8)}-${normalized.slice(8, 12)}-${normalized.slice(12, 16)}-${normalized.slice(16, 20)}-${normalized.slice(20)}`;
  if (
    !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(normalized) ||
    normalized === "00000000-0000-0000-0000-000000000000"
  )
    throw new Error("A non-nil pricing/SKU UUID is required");
  return normalized;
}

/** Omission is not an explicit blank selector. Never erase a malformed buyer context. */
export function normalizePricingQuery(
  query: StorefrontPricingQuery,
): StorefrontPricingQuery {
  const uuid = (value: string | null | undefined) =>
    value == null ? value : parsePricingUuid(value);
  const currencyCode =
    query.currencyCode == null
      ? undefined
      : query.currencyCode.trim().toUpperCase();
  if (currencyCode !== undefined && !/^[A-Z]{3}$/.test(currencyCode))
    throw new Error("Pricing currency must be a three-letter code");
  const channelSlug =
    query.channelSlug == null
      ? query.channelSlug
      : query.channelSlug.trim().toLowerCase();
  if (channelSlug === "") throw new Error("Pricing channel must not be empty");
  if (
    query.quantity != null &&
    (!Number.isInteger(query.quantity) ||
      query.quantity < 1 ||
      query.quantity > 2147483647)
  )
    throw new Error("Pricing quantity must be a positive 32-bit integer");
  const locale =
    query.locale == null ? undefined : normalizeLocaleTag(query.locale);
  if (query.locale != null && (!locale || query.locale.length > 64))
    throw new Error("Pricing locale is invalid");
  const normalized = {
    ...query,
    currencyCode,
    channelSlug,
    locale,
    regionId: uuid(query.regionId),
    priceListId: uuid(query.priceListId),
    channelId: uuid(query.channelId),
  };
  if (
    !currencyCode &&
    (normalized.regionId != null ||
      normalized.priceListId != null ||
      query.quantity != null)
  )
    throw new Error("A currency is required for the requested pricing context");
  return normalized;
}

export function validatePricingQuery(query: StorefrontPricingQuery): void {
  normalizePricingQuery(query);
}

/** RFC3339 wire time only. Date.parse alone accepts prose and normalizes impossible dates. */
function validResolvedAt(value: unknown): value is string {
  if (typeof value !== "string") return false;
  const parts =
    /^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(?:[Zz]|([+-])(\d{2}):(\d{2}))$/.exec(
      value,
    );
  if (!parts) return false;
  const [, y, m, d, h, minute, second, , offsetHour, offsetMinute] = parts;
  const year = Number(y),
    month = Number(m),
    day = Number(d);
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return (
    month >= 1 &&
    month <= 12 &&
    day >= 1 &&
    day <= days[month - 1] &&
    Number(h) < 24 &&
    Number(minute) < 60 &&
    Number(second) <= 60 &&
    (offsetHour === undefined ||
      (Number(offsetHour) < 24 && Number(offsetMinute) < 60)) &&
    Number.isFinite(
      Date.parse(
        value
          .toUpperCase()
          .replace(/(T\d{2}:\d{2}:)60/, (_, prefix: string) => `${prefix}59`),
      ),
    )
  );
}

/** Validate wire quote/inspection mode, currency, quantity and observation presence.
 * Scope/identity completion and money arithmetic remain exclusively backend-owner work.
 */
export function assertPricingQuoteResponse(
  product: Pick<PricingProductDetail, "resolutionContext" | "resolvedAt"> & {
    variants: {
      effectivePrice?: Pick<PricingEffectivePrice, "currencyCode"> | null;
    }[];
  },
  query: StorefrontPricingQuery,
): void {
  const requested = normalizePricingQuery(query);
  const currency = requested.currencyCode ?? null;
  const context = product.resolutionContext;
  if (
    context === undefined ||
    product.resolvedAt === undefined ||
    product.variants.some((variant) => variant.effectivePrice === undefined)
  ) {
    throw new Error("Pricing response is missing quote observation metadata");
  }
  if (!currency) {
    if (
      context !== null ||
      product.resolvedAt !== null ||
      product.variants.some((variant) => variant.effectivePrice)
    ) {
      throw new Error("Pricing returned a quote for an inspection request");
    }
    return;
  }
  if (
    !context ||
    context.currencyCode !== currency ||
    context.quantity !== (query.quantity ?? 1) ||
    !validResolvedAt(product.resolvedAt) ||
    product.variants.some(
      (variant) =>
        variant.effectivePrice &&
        variant.effectivePrice.currencyCode !== currency,
    )
  ) {
    throw new Error(
      "Pricing quote response does not match the requested currency and quantity",
    );
  }
  if (
    context.regionId === undefined ||
    context.priceListId === undefined ||
    context.channelId === undefined ||
    context.channelSlug === undefined ||
    context.regionId !== (requested.regionId ?? null) ||
    context.priceListId !== (requested.priceListId ?? null) ||
    (requested.channelId != null &&
      context.channelId !== requested.channelId) ||
    (requested.channelSlug != null &&
      context.channelSlug?.toLowerCase() !== requested.channelSlug)
  ) {
    throw new Error(
      "Pricing quote response does not match the requested scope",
    );
  }
  // Completion is owner-owned, but malformed or missing canonical selector halves are not a quote.
  for (const id of [context.regionId, context.priceListId, context.channelId])
    if (id !== null && parsePricingUuid(id) !== id)
      throw new Error("Pricing returned a noncanonical UUID");
  if (
    context.channelSlug !== null &&
    (!context.channelSlug ||
      context.channelSlug.trim().toLowerCase() !== context.channelSlug)
  )
    throw new Error("Pricing returned an empty channel");
}

/** Partial GraphQL data plus errors is a failed read, not a successful not-found/unpriced quote. */
export function assertPricingGraphqlSuccess(response: {
  errors?: readonly unknown[];
}): void {
  if (response.errors?.length) {
    throw new Error("Pricing request could not be completed");
  }
}

/** Quantity transport parsing, never money arithmetic or a silent parseInt/default fallback. */
export function parsePricingQuantity(
  value: string | undefined,
): number | undefined {
  if (value === undefined) return undefined;
  const raw = value.trim();
  if (!/^[0-9]{1,10}$/.test(raw))
    throw new Error("Pricing quantity must be a positive 32-bit integer");
  const quantity = Number(raw);
  if (quantity < 1 || quantity > 2147483647)
    throw new Error("Pricing quantity must be a positive 32-bit integer");
  return quantity;
}

/** Canonical Pricing route keys, shared with the Rust pricing storefront (currency, not currency_code). */
export function parsePricingRouteQuery(
  params: Record<string, string | string[] | undefined>,
  locale: string,
): StorefrontPricingQuery {
  const scalar = (name: string): string | undefined => {
    const value = params[name];
    if (Array.isArray(value))
      throw new Error("Pricing query parameters must be single-valued");
    return value;
  };
  const text = (name: string) => scalar(name)?.trim() || undefined;
  return normalizePricingQuery({
    selectedHandle: text("handle"),
    locale,
    currencyCode: scalar("currency"),
    regionId: scalar("region_id"),
    priceListId: scalar("price_list_id"),
    channelId: scalar("channel_id"),
    channelSlug: scalar("channel_slug"),
    quantity: parsePricingQuantity(scalar("quantity")),
  });
}
