/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { normalizeLocaleTag } from "@rustok/next-fluent/utils";
import {
  assertPricingQuoteResponse,
  parsePricingQuantity,
  parsePricingUuid,
  parsePricingRouteQuery,
} from "@rustok/pricing-frontend";
import { mergeProductPricing } from "./pricing";
import type { StorefrontPricingQuery } from "@rustok/pricing-frontend";
import type {
  StorefrontPricingContext,
  StorefrontProductDetail,
  StorefrontEffectivePrice,
  StorefrontProductPricingDetail,
  StorefrontProductVariant,
} from "../api/types";

export const MAX_BUYER_VARIANTS = 256;
export const MAX_BUYER_AXES = 32;
export const MAX_BUYER_AXIS_VALUES = 256;

export type BuyerErrorCode = "projection" | "selection" | "request" | "quote";
export class ProductBuyerError extends Error {
  constructor(readonly code: BuyerErrorCode) {
    super(`Product buyer ${code} was refused`);
  }
}

export type BuyerAxes = Readonly<Record<string, string>>;
export type BuyerSelection = {
  values: BuyerAxes;
  variant: StorefrontProductVariant | null;
  status: "selected" | "incomplete" | "unavailable";
};
export type BuyerRoute = {
  selection: BuyerSelection;
  pricing: StorefrontPricingQuery;
};

function uuid(value: string): string {
  try {
    return parsePricingUuid(value);
  } catch {
    throw new ProductBuyerError("selection");
  }
}

/** A bounded structural guard; it never repairs unknown installed state or synthesizes tuples. */
export function assertBuyerProduct(product: StorefrontProductDetail): void {
  if (
    !Array.isArray(product.variantAxes) ||
    !Array.isArray(product.variants) ||
    product.variantAxes.length > MAX_BUYER_AXES ||
    product.variants.length < 1 ||
    product.variants.length > MAX_BUYER_VARIANTS
  )
    throw new ProductBuyerError("projection");
  const axes = new Map<string, Set<string>>();
  const axisIds = new Set<string>();
  try {
    if (uuid(product.id) !== product.id)
      throw new ProductBuyerError("projection");
    for (const axis of product.variantAxes) {
      const id = uuid(axis.attributeId);
      const configId = uuid(axis.id);
      if (
        id !== axis.attributeId ||
        configId !== axis.id ||
        axes.has(id) ||
        axisIds.has(configId) ||
        !axis.allowedValues.length ||
        axis.allowedValues.length > MAX_BUYER_AXIS_VALUES
      )
        throw new ProductBuyerError("projection");
      axisIds.add(configId);
      const options = new Set(
        axis.allowedValues.map((value) => uuid(value.optionId)),
      );
      if (
        options.size !== axis.allowedValues.length ||
        axis.allowedValues.some(
          (value) => uuid(value.optionId) !== value.optionId,
        )
      )
        throw new ProductBuyerError("projection");
      axes.set(id, options);
    }
    const variantIds = new Set<string>();
    const tuples = new Set<string>();
    for (const variant of product.variants) {
      const id = uuid(variant.id);
      if (
        id !== variant.id ||
        variantIds.has(id) ||
        !["deny", "continue"].includes(variant.inventoryPolicy) ||
        typeof variant.inStock !== "boolean" ||
        typeof variant.inventoryQuantity !== "number" ||
        !Number.isInteger(variant.inventoryQuantity) ||
        variant.inventoryQuantity < -2147483648 ||
        variant.inventoryQuantity > 2147483647 ||
        !Array.isArray(variant.prices) ||
        !Array.isArray(variant.axisValues) ||
        variant.axisValues.length !== axes.size
      )
        throw new ProductBuyerError("projection");
      variantIds.add(id);
      const values = new Map<string, string>();
      for (const value of variant.axisValues) {
        const attribute = uuid(value.attributeId);
        const option = uuid(value.optionId);
        if (
          attribute !== value.attributeId ||
          option !== value.optionId ||
          values.has(attribute) ||
          !axes.get(attribute)?.has(option)
        )
          throw new ProductBuyerError("projection");
        values.set(attribute, option);
      }
      const tuple = [...values]
        .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([attribute, option]) => `${attribute}:${option}`)
        .join(";");
      // combinationIdentity is an owner-maintained derived mirror, not another assignment source.
      if (variant.combinationIdentity !== (tuple || null) || tuples.has(tuple))
        throw new ProductBuyerError("projection");
      tuples.add(tuple);
    }
    if (!axes.size && product.variants.length !== 1)
      throw new ProductBuyerError("projection");
  } catch {
    throw new ProductBuyerError("projection");
  }
}

export function selectBuyerTuple(
  product: StorefrontProductDetail,
  values: BuyerAxes,
): BuyerSelection {
  assertBuyerProduct(product);
  const axes = product.variantAxes;
  for (const [attribute, option] of Object.entries(values)) {
    const axis = axes.find((axis) => axis.attributeId === attribute);
    if (!axis?.allowedValues.some((value) => value.optionId === option))
      throw new ProductBuyerError("selection");
  }
  if (Object.keys(values).length !== axes.length)
    return { values, variant: null, status: "incomplete" };
  const variant =
    product.variants.find((variant) =>
      variant.axisValues.every(
        (value) => values[value.attributeId] === value.optionId,
      ),
    ) ?? null;
  return { values, variant, status: variant ? "selected" : "unavailable" };
}

export function pickBuyerAxis(
  product: StorefrontProductDetail,
  current: BuyerAxes,
  attributeId: string,
  optionId: string,
): BuyerSelection {
  const next = { ...current };
  if (optionId) next[attributeId] = optionId;
  else delete next[attributeId];
  return selectBuyerTuple(product, next);
}

export function buyerOptionExists(
  product: StorefrontProductDetail,
  values: BuyerAxes,
  attributeId: string,
  optionId: string,
): boolean {
  return product.variants.some((variant) =>
    variant.axisValues.every((assignment) =>
      assignment.attributeId === attributeId
        ? assignment.optionId === optionId
        : !values[assignment.attributeId] ||
          values[assignment.attributeId] === assignment.optionId,
    ),
  );
}

/** A sole observed catalog currency may initialize a selector, never a purchase amount. */
export function buyerCurrencies(product: StorefrontProductDetail): string[] {
  return [
    ...new Set(
      product.variants.flatMap((variant) =>
        variant.prices.map((price) => price.currencyCode),
      ),
    ),
  ]
    .filter((currency) => /^[A-Z]{3}$/.test(currency))
    .sort();
}

export function parseBuyerRoute(
  product: StorefrontProductDetail,
  params: Record<string, string | string[] | undefined>,
  locale: string,
): BuyerRoute {
  assertBuyerProduct(product);
  try {
    if (
      Object.keys(params).length > 256 ||
      Object.values(params).some(Array.isArray)
    )
      throw new ProductBuyerError("request");
    const encoded = new URLSearchParams(
      Object.entries(params).filter(
        (entry): entry is [string, string] => typeof entry[1] === "string",
      ),
    );
    if (encoded.toString().length > 16 * 1024)
      throw new ProductBuyerError("request");
    const currencies = buyerCurrencies(product);
    const currency =
      params.currency === undefined && currencies.length === 1
        ? currencies[0]
        : params.currency;
    if (Array.isArray(params.quantity)) throw new ProductBuyerError("request");
    const quantity = parsePricingQuantity(params.quantity);
    const pricing = parsePricingRouteQuery(
      {
        ...params,
        // Product quantity may be selected before currency; no Pricing read occurs until currency
        // is selected. Region/list selectors still require the owner's explicit currency context.
        currency,
        quantity: currency ? params.quantity : undefined,
      },
      locale,
    );
    pricing.quantity = quantity;
    const values: Record<string, string> = {};
    for (const [key, value] of Object.entries(params)) {
      if (!key.startsWith("axis.")) continue;
      if (typeof value !== "string") throw new ProductBuyerError("selection");
      const attribute = uuid(key.slice(5));
      if (Object.hasOwn(values, attribute))
        throw new ProductBuyerError("selection");
      values[attribute] = uuid(value);
    }
    const requested = params.variant_id;
    if (requested !== undefined) {
      if (typeof requested !== "string")
        throw new ProductBuyerError("selection");
      const variant = product.variants.find(
        (variant) => variant.id === uuid(requested),
      );
      if (!variant) throw new ProductBuyerError("selection");
      const tuple = Object.fromEntries(
        variant.axisValues.map((value) => [value.attributeId, value.optionId]),
      );
      if (
        Object.entries(values).some(
          ([attribute, option]) => tuple[attribute] !== option,
        )
      )
        throw new ProductBuyerError("selection");
      return { selection: selectBuyerTuple(product, tuple), pricing };
    }
    return { selection: selectBuyerTuple(product, values), pricing };
  } catch (error) {
    throw error instanceof ProductBuyerError
      ? error
      : new ProductBuyerError("request");
  }
}

export function buyerQuantity(text: string): number | null {
  try {
    return parsePricingQuantity(text) ?? null;
  } catch {
    return null;
  }
}

export function buyerHasStock(
  variant: StorefrontProductVariant | null,
  quantity: number | null,
): boolean {
  return (
    !!variant &&
    quantity !== null &&
    variant.inStock &&
    (variant.inventoryPolicy === "continue" ||
      (variant.inventoryQuantity ?? 0) >= quantity)
  );
}

/** Complete UUID correlation makes an omitted/foreign SKU a refusal, not an unpriced result. */
export function assertBuyerQuote(
  product: StorefrontProductDetail,
  quote: StorefrontProductPricingDetail,
  request: StorefrontPricingQuery,
): void {
  assertPricingQuoteResponse(quote, request);
  const ids = new Set(quote.variants.map((variant) => variant.id));
  if (
    quote.id !== product.id ||
    ids.size !== quote.variants.length ||
    ids.size !== product.variants.length ||
    product.variants.some((variant) => !ids.has(variant.id))
  ) {
    throw new ProductBuyerError("quote");
  }
}

export function buyerQuoteKey(
  productId: string,
  variantId: string | null,
  request: StorefrontPricingQuery,
): string {
  return JSON.stringify([
    productId,
    variantId,
    request.currencyCode ?? null,
    request.regionId ?? null,
    request.priceListId ?? null,
    request.channelId ?? null,
    request.channelSlug ?? null,
    request.quantity ?? 1,
  ]);
}

export function buyerCanPurchase(
  selection: BuyerSelection,
  quantity: number | null,
  context: StorefrontPricingContext | null | undefined,
): boolean {
  return (
    selection.status === "selected" &&
    buyerHasStock(selection.variant, quantity) &&
    !!context &&
    context.quantity === quantity &&
    context.priceListId === null
  );
}

/** Preserve unrelated host query state; only real tuples receive a canonical SKU UUID. */
export function buyerHref(
  path: string,
  search: string,
  selection: BuyerSelection,
  request: StorefrontPricingQuery,
  quantityText: string,
): string {
  const params = new URLSearchParams(search);
  for (const key of [...params.keys()])
    if (key === "variant_id" || key.startsWith("axis.")) params.delete(key);
  if (selection.variant) params.set("variant_id", selection.variant.id);
  else
    for (const [attribute, option] of Object.entries(selection.values).sort())
      params.set(`axis.${attribute}`, option);
  for (const [key, value] of [
    ["currency", request.currencyCode],
    ["region_id", request.regionId],
    ["price_list_id", request.priceListId],
    ["channel_id", request.channelId],
    ["channel_slug", request.channelSlug],
  ] as const) {
    if (value) params.set(key, value);
    else params.delete(key);
  }
  params.set("quantity", quantityText);
  const suffix = params.toString();
  return suffix ? `${path}?${suffix}` : path;
}

export type BuyerReadStatus = "pending" | "ready" | "failed" | "idle";
export type BuyerViewStatus =
  | "invalidQuantity"
  | "chooseAxes"
  | "missingTuple"
  | "chooseCurrency"
  | "loading"
  | "unavailable"
  | "unpriced"
  | "preview"
  | "stock"
  | "ready";
export interface BuyerView {
  quantity: number | null;
  price: StorefrontEffectivePrice | null;
  status: BuyerViewStatus;
  canPurchase: boolean;
}
export function buildBuyerView(
  product: StorefrontProductDetail,
  selection: BuyerSelection,
  quantityText: string,
  request: StorefrontPricingQuery,
  quote: StorefrontProductPricingDetail | null,
  readStatus: BuyerReadStatus,
): BuyerView {
  const quantity = buyerQuantity(quantityText);
  let observed = false;
  if (readStatus === "ready" && quote) {
    try {
      assertBuyerQuote(product, quote, request);
      observed = true;
    } catch {
      /* Explicit unavailable state below. */
    }
  }
  const variant = mergeProductPricing(
    product.variants,
    quote?.variants ?? [],
  ).find((variant) => variant.id === selection.variant?.id);
  const price =
    observed &&
    quantity !== null &&
    selection.status === "selected" &&
    request.currencyCode &&
    quote?.resolutionContext?.quantity === quantity
      ? (variant?.effectivePrice ?? null)
      : null;
  const status =
    quantity === null
      ? "invalidQuantity"
      : selection.status === "incomplete"
        ? "chooseAxes"
        : selection.status === "unavailable"
          ? "missingTuple"
          : !request.currencyCode
            ? "chooseCurrency"
            : readStatus === "pending"
              ? "loading"
              : !observed ||
                  !quote?.resolutionContext ||
                  variant?.pricingStatus === "unavailable"
                ? "unavailable"
                : !price
                  ? "unpriced"
                  : quote.resolutionContext.priceListId !== null
                    ? "preview"
                    : !buyerHasStock(selection.variant, quantity)
                      ? "stock"
                      : "ready";
  return {
    quantity,
    price,
    status,
    canPurchase:
      status === "ready" &&
      buyerCanPurchase(selection, quantity, quote?.resolutionContext),
  };
}

export function createBuyerInput<Scope>(scope: Scope, route: BuyerRoute) {
  return {
    scope,
    revision: 0,
    selection: route.selection,
    request: route.pricing,
    quantityText: String(route.pricing.quantity ?? 1),
  };
}
export function buyerInputInScope<Scope>(
  input: ReturnType<typeof createBuyerInput<Scope>>,
  scope: Scope,
  initial: BuyerRoute,
) {
  return input.scope === scope ? input : createBuyerInput(scope, initial);
}

/** Derive availability from actual tuples once per axis, not once per Cartesian candidate. */
export function buyerAxisViews(
  product: StorefrontProductDetail,
  values: BuyerAxes,
) {
  return [...product.variantAxes]
    .sort(
      (a, b) =>
        a.position - b.position || a.attributeId.localeCompare(b.attributeId),
    )
    .map((axis) => {
      const available = new Set<string>();
      for (const variant of product.variants) {
        if (
          variant.axisValues.every(
            (assignment) =>
              assignment.attributeId === axis.attributeId ||
              !values[assignment.attributeId] ||
              values[assignment.attributeId] === assignment.optionId,
          )
        ) {
          const option = variant.axisValues.find(
            (assignment) => assignment.attributeId === axis.attributeId,
          );
          if (option) available.add(option.optionId);
        }
      }
      return {
        ...axis,
        options: [...axis.allowedValues]
          .sort(
            (a, b) =>
              a.position - b.position || a.optionId.localeCompare(b.optionId),
          )
          .map((option) => ({
            ...option,
            disabled: !available.has(option.optionId),
          })),
      };
    });
}

/** Every input edit creates a new observation cycle, including A → B → A. */
export function updateBuyerInput<Scope>(
  input: ReturnType<typeof createBuyerInput<Scope>>,
  patch: Partial<
    Pick<
      ReturnType<typeof createBuyerInput<Scope>>,
      "selection" | "request" | "quantityText"
    >
  >,
) {
  return { ...input, ...patch, revision: input.revision + 1 };
}

/** Exact locale only: an en-US cart must not be silently treated as an en cart. */
export function buyerCartRecovery(
  locale: string,
  currentLocale: string,
  paths: Readonly<Record<string, string>>,
) {
  const canonical = normalizeLocaleTag(locale);
  if (!canonical) return { kind: "unavailable" } as const;
  if (canonical === normalizeLocaleTag(currentLocale))
    return { kind: "inline" } as const;
  const path = Object.entries(paths).find(
    ([key]) => normalizeLocaleTag(key) === canonical,
  )?.[1];
  return path
    ? ({ kind: "navigate", path } as const)
    : ({ kind: "unavailable" } as const);
}
