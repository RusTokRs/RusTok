/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslations } from "@rustok/next-fluent";
import {
  pricingContextQuery,
  type StorefrontPricingQuery,
} from "@rustok/pricing-frontend";
import { pricingExpectationFromCart, useCart } from "@rustok/cart-frontend";
import { Button } from "@/shared/ui/base/button";
import { Input, NativeSelect, Label } from "@rustok/ui-controls";
import {
  fetchStorefrontProductPricing,
  type ProductGraphqlExecutor,
} from "../api/products";
import type {
  StorefrontProductDetail,
  StorefrontProductPricingDetail,
} from "../api/types";
import {
  assertBuyerQuote,
  buildBuyerView,
  buyerAxisViews,
  buyerCurrencies,
  buyerHref,
  buyerQuoteKey,
  buyerInputInScope,
  createBuyerInput,
  updateBuyerInput,
  buyerCartRecovery,
  pickBuyerAxis,
  type BuyerErrorCode,
  type BuyerRoute,
  type BuyerReadStatus,
} from "../catalog/buyer";

export interface ProductBuyerPanelProps {
  product: StorefrontProductDetail;
  handle: string;
  locale: string;
  tenantSlug?: string | null;
  token?: string | null;
  graphql: ProductGraphqlExecutor;
  path: string;
  initialSearch: string;
  initialRoute: BuyerRoute | null;
  initialError?: BuyerErrorCode | null;
  initialQuote?: StorefrontProductPricingDetail | null;
  localePaths?: Readonly<Record<string, string>>;
}

export function ProductBuyerPanel(props: ProductBuyerPanelProps) {
  const messages = useTranslations("Product");
  if (!props.initialRoute)
    return (
      <section
        aria-live="polite"
        className="space-y-3 rounded-xl border border-destructive p-4"
      >
        <p role="alert">
          {messages(
            props.initialError === "projection"
              ? "buyerProjectionError"
              : "buyerRequestError",
          )}
        </p>
        <Button asChild variant="outline">
          <a href={props.path}>{messages("buyerReset")}</a>
        </Button>
      </section>
    );
  return (
    <ProductBuyerInteraction {...props} initialRoute={props.initialRoute} />
  );
}

function ProductBuyerInteraction({
  product,
  handle,
  locale,
  tenantSlug,
  token,
  graphql,
  path,
  initialSearch,
  initialRoute,
  initialQuote = null,
  localePaths = {},
}: ProductBuyerPanelProps & { initialRoute: BuyerRoute }) {
  const messages = useTranslations("Product");
  const cartMessages = useTranslations("Cart");
  const cartState = useCart();
  const cartCanStartPurchase =
    cartState.enabled &&
    !cartState.isLoading &&
    !cartState.isUpdating &&
    cartState.actionError !== "auth" &&
    cartState.actionError !== "restore" &&
    cartState.actionError !== "unconfirmed";
  const scope = useMemo(
    () => ({ product, handle, locale, tenantSlug }),
    [product, handle, locale, tenantSlug],
  );
  const [storedInput, setInput] = useState(() =>
    createBuyerInput(scope, initialRoute),
  );
  const input = buyerInputInScope(storedInput, scope, initialRoute);
  const { selection, request, quantityText } = input;
  const axes = useMemo(
    () => buyerAxisViews(product, selection.values),
    [product, selection.values],
  );
  useEffect(() => {
    setInput((previous) => (previous.scope === scope ? previous : input));
  }, [scope, input]);
  const [retry, setRetry] = useState(0);
  const quantity = useMemo(
    () =>
      buildBuyerView(product, selection, quantityText, request, null, "idle")
        .quantity,
    [product, selection, quantityText, request],
  );
  const query = useMemo<StorefrontPricingQuery>(
    () => ({ ...request, quantity: quantity ?? undefined }),
    [request, quantity],
  );
  const readKey = JSON.stringify([
    buyerQuoteKey(product.id, selection.variant?.id ?? null, query),
    input.revision,
  ]);
  // Cart capability, restore, projection or refusal changes create a new quote scope without
  // erasing the user's selected tuple or quantity. Only product/route scope resets the input.
  const quoteScope = useMemo(
    () => ({
      scope,
      token,
      graphql,
      cart: cartState.cart,
      enabled: cartState.enabled,
      loading: cartState.isLoading,
      error: cartState.actionError,
    }),
    [
      scope,
      token,
      graphql,
      cartState.cart,
      cartState.enabled,
      cartState.isLoading,
      cartState.actionError,
    ],
  );
  const [added, setAdded] = useState<{
    scope: typeof quoteScope;
    key: string;
  } | null>(null);
  const [read, setRead] = useState<{
    scope: typeof quoteScope;
    key: string;
    retry: number;
    data: StorefrontProductPricingDetail | null;
    status: BuyerReadStatus;
  }>({
    scope: quoteScope,
    key: readKey,
    retry,
    data: initialQuote,
    status: initialQuote ? "ready" : "idle",
  });
  const visible =
    read.scope === quoteScope && read.key === readKey && read.retry === retry;
  const data =
    visible && quantity !== null && selection.status === "selected"
      ? read.data
      : null;
  const seed = useRef({ scope: quoteScope, key: readKey, used: false });
  const status = visible ? read.status : "pending";
  const view = buildBuyerView(
    product,
    selection,
    quantityText,
    query,
    data,
    status,
  );
  const cartSearch = new URLSearchParams(initialSearch);
  if (!cartState.isLoading) {
    if (cartState.reference) cartSearch.set("cart_id", cartState.reference);
    else cartSearch.delete("cart_id");
  }
  const shareHref = buyerHref(
    path,
    cartSearch.toString(),
    selection,
    query,
    quantityText,
  );
  const [savedLink, setSavedLink] = useState<{
    scope: typeof scope;
    href: string;
    ok: boolean;
  } | null>(null);
  const linkReady =
    savedLink?.scope === scope && savedLink.href === shareHref && savedLink.ok;
  const linkFailed =
    savedLink?.scope === scope && savedLink.href === shareHref && !savedLink.ok;
  const cartExpectation =
    cartState.cart && view.quantity !== null
      ? pricingExpectationFromCart(cartState.cart, view.quantity)
      : null;
  const cartRecovery = cartExpectation
    ? buyerCartRecovery(cartExpectation.locale, locale, localePaths)
    : null;
  const recoveryHref =
    cartExpectation && cartRecovery?.kind === "navigate"
      ? buyerHref(
          cartRecovery.path,
          cartSearch.toString(),
          selection,
          pricingContextQuery(cartExpectation),
          quantityText,
        )
      : null;
  const currencies = buyerCurrencies(product);

  useEffect(() => {
    // Native history is supported by the Next router. It avoids an unrelated server navigation
    // while preserving a reloadable link; a partial tuple never becomes another SKU UUID.
    try {
      window.history.replaceState(null, "", shareHref + window.location.hash);
      setSavedLink({ scope, href: shareHref, ok: true });
    } catch {
      setSavedLink({ scope, href: shareHref, ok: false });
    }
  }, [scope, shareHref]);

  useEffect(() => {
    if (
      !query.currencyCode ||
      quantity === null ||
      selection.status !== "selected"
    ) {
      setRead({
        scope: quoteScope,
        key: readKey,
        retry,
        data: null,
        status: "idle",
      });
      return;
    }
    if (
      !seed.current.used &&
      input.revision === 0 &&
      retry === 0 &&
      seed.current.scope === quoteScope &&
      seed.current.key === readKey &&
      initialQuote
    ) {
      seed.current.used = true;
      return;
    }
    seed.current.used = true;
    let cancelled = false;
    setRead({
      scope: quoteScope,
      key: readKey,
      retry,
      data: null,
      status: "pending",
    });
    void fetchStorefrontProductPricing(
      graphql,
      handle,
      locale,
      {
        currencyCode: query.currencyCode,
        regionId: query.regionId ?? undefined,
        priceListId: query.priceListId ?? undefined,
        channelId: query.channelId ?? undefined,
        channelSlug: query.channelSlug ?? undefined,
        quantity,
      },
      tenantSlug,
      token,
    )
      .then((quote) => {
        if (!quote) throw new Error("Pricing observation is missing");
        assertBuyerQuote(product, quote, query);
        if (!cancelled)
          setRead({
            scope: quoteScope,
            key: readKey,
            retry,
            data: quote,
            status: "ready",
          });
      })
      .catch(() => {
        if (!cancelled)
          setRead({
            scope: quoteScope,
            key: readKey,
            retry,
            data: null,
            status: "failed",
          });
      });
    return () => {
      cancelled = true;
    };
    // The immutable key includes every price selector, SKU and quantity. `scope` includes
    // product projection, tenant, actor, locale and executor; obsolete responses cannot publish.
  }, [
    quoteScope,
    readKey,
    retry,
    input.revision,
    query,
    quantity,
    selection.status,
    graphql,
    handle,
    locale,
    tenantSlug,
    token,
    product,
    initialQuote,
  ]);

  async function add() {
    if (
      !linkReady ||
      !cartCanStartPurchase ||
      !view.canPurchase ||
      !selection.variant ||
      view.quantity === null ||
      !data?.resolutionContext
    )
      return;
    const accepted = await cartState.addItem(
      selection.variant.id,
      view.quantity,
      { ...data.resolutionContext, locale },
    );
    if (accepted) {
      setAdded({ scope: quoteScope, key: readKey });
      cartState.openCart();
    }
  }

  const statuses = {
    invalidQuantity: "buyerInvalidQuantity",
    chooseAxes: "buyerChooseAxes",
    missingTuple: "buyerMissingTuple",
    chooseCurrency: "buyerChooseCurrency",
    loading: "buyerLoading",
    unavailable: "pricingUnavailable",
    unpriced: "pricingUnpriced",
    preview: "buyerPreview",
    stock: "buyerStock",
    ready: "buyerReady",
  } as const;
  const errors = {
    auth: "buyerAuthRefused",
    restore: "buyerCartRestore",
    context: "buyerCartContext",
    invalid: "buyerRequestError",
    unconfirmed: "buyerCartUnconfirmed",
  } as const;

  return (
    <section
      className="space-y-5 border-t border-border pt-5"
      aria-busy={status === "pending"}
    >
      {axes.map((axis) => (
        <div key={axis.attributeId} className="space-y-2">
          <Label
            htmlFor={`axis-${axis.attributeId}`}
            className="text-sm font-medium"
          >
            {axis.name}
          </Label>
          <NativeSelect
            id={`axis-${axis.attributeId}`}
            name={`axis.${axis.attributeId}`}
            value={selection.values[axis.attributeId] ?? ""}
            disabled={cartState.isUpdating}
            onChange={(event) =>
              setInput(
                updateBuyerInput(input, {
                  selection: pickBuyerAxis(
                    product,
                    selection.values,
                    axis.attributeId,
                    event.target.value,
                  ),
                }),
              )
            }
          >
            <option value="">{messages("buyerNotSelected")}</option>
            {axis.options.map((value) => (
              <option
                key={value.optionId}
                value={value.optionId}
                disabled={value.disabled}
              >
                {value.value}
              </option>
            ))}
          </NativeSelect>
        </div>
      ))}
      {selection.variant && (
        <p className="text-sm">
          {messages("sku")}:{" "}
          <span className="font-mono">
            {selection.variant.sku || selection.variant.id}
          </span>
        </p>
      )}
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-2">
          <Label
            htmlFor="product-buyer-currency"
            className="text-sm font-medium"
          >
            {messages("buyerCurrency")}
          </Label>
          <NativeSelect
            id="product-buyer-currency"
            value={request.currencyCode ?? ""}
            disabled={cartState.isUpdating}
            onChange={(event) =>
              setInput(
                updateBuyerInput(input, {
                  request: {
                    ...request,
                    currencyCode: event.target.value || undefined,
                    regionId: null,
                    priceListId: null,
                  },
                }),
              )
            }
          >
            <option value="">{messages("buyerNotSelected")}</option>
            {[
              ...new Set([
                ...currencies,
                ...(request.currencyCode ? [request.currencyCode] : []),
              ]),
            ]
              .sort()
              .map((currency) => (
                <option key={currency}>{currency}</option>
              ))}
          </NativeSelect>
        </div>
        <div className="space-y-2">
          <Label
            htmlFor="product-buyer-quantity"
            className="text-sm font-medium"
          >
            {messages("quantity")}
          </Label>
          <Input
            id="product-buyer-quantity"
            type="text"
            inputMode="numeric"
            value={quantityText}
            disabled={cartState.isUpdating}
            onChange={(event) =>
              setInput(
                updateBuyerInput(input, { quantityText: event.target.value }),
              )
            }
            aria-invalid={view.quantity === null}
            aria-describedby="product-buyer-status"
          />
        </div>
      </div>
      <div aria-live="polite" aria-atomic="true" className="space-y-2">
        {view.price && (
          <div className="flex flex-wrap items-baseline gap-3">
            <p className="text-3xl font-bold">
              {view.price.amount} {view.price.currencyCode}
            </p>
            {view.price.onSale && view.price.compareAtAmount && (
              <del className="text-muted-foreground">
                {view.price.compareAtAmount} {view.price.currencyCode}
              </del>
            )}
          </div>
        )}
        <p id="product-buyer-status" className="text-sm text-muted-foreground">
          {messages(statuses[view.status])}
        </p>
        {data?.resolvedAt && view.price && (
          <p className="text-xs text-muted-foreground">
            {messages("buyerObservation")}{" "}
            <time dateTime={data.resolvedAt}>{data.resolvedAt}</time>
          </p>
        )}
      </div>
      {linkFailed && <p role="alert">{messages("buyerLinkError")}</p>}
      {!cartState.enabled && (
        <p role="status">{cartMessages("disabledError")}</p>
      )}
      {cartState.actionError && (
        <div
          role="alert"
          className="space-y-2 rounded-lg border border-destructive p-3 text-sm"
        >
          <p>{messages(errors[cartState.actionError])}</p>
          <Button
            variant="outline"
            onClick={() => {
              cartState.openCart();
              void cartState.refreshCart();
            }}
          >
            {messages("buyerInspectCart")}
          </Button>
          {(cartState.actionError === "restore" ||
            cartState.actionError === "unconfirmed") && (
            <Button variant="outline" onClick={cartState.clearCart}>
              {messages("buyerResetCartReference")}
            </Button>
          )}
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        <Button
          onClick={() => void add()}
          disabled={!linkReady || !cartCanStartPurchase || !view.canPurchase}
        >
          {cartState.isUpdating
            ? messages("buyerAdding")
            : added?.scope === quoteScope && added.key === readKey
              ? messages("addedToCart")
              : messages("addToCart")}
        </Button>
        {view.status === "unavailable" && (
          <Button
            variant="outline"
            onClick={() => setRetry((value) => value + 1)}
          >
            {messages("buyerRetry")}
          </Button>
        )}
        {cartExpectation && cartRecovery?.kind === "inline" && (
          <Button
            variant="outline"
            disabled={cartState.isUpdating}
            onClick={() =>
              setInput(
                updateBuyerInput(input, {
                  request: {
                    ...pricingContextQuery(cartExpectation),
                    locale: cartExpectation.locale,
                  },
                }),
              )
            }
          >
            {messages("buyerUseCartContext")}
          </Button>
        )}
        {recoveryHref && (
          <Button asChild variant="outline">
            <a href={recoveryHref}>{messages("buyerUseCartContext")}</a>
          </Button>
        )}
        {cartRecovery?.kind === "unavailable" && (
          <p role="status">{messages("buyerCartLocale")}</p>
        )}
        {view.status === "preview" && (
          <Button
            variant="outline"
            onClick={() =>
              setInput(
                updateBuyerInput(input, {
                  request: { ...request, priceListId: null },
                }),
              )
            }
          >
            {messages("buyerOrdinaryQuote")}
          </Button>
        )}
        <Button asChild variant="outline">
          <a href={shareHref}>{messages("buyerSelectionLink")}</a>
        </Button>
      </div>
    </section>
  );
}
