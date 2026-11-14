/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export interface FlyActionDetail {
  kind: string;
  source?: HTMLElement;
  sourceId?: string | null;
  event?: string;
  payload?: Record<string, unknown>;
  provider?: string;
  action?: string;
  input?: Record<string, unknown>;
}

export type LandingBridgeError = "buyer_handoff_invalid" | "cart_unavailable";

export interface LandingBridgeHandlers {
  openCart(): void;
  closeCart(): void;
  toggleCart(): void;
  navigate(href: string): void;
  reportError(error: LandingBridgeError): void;
  clearError(): void;
}

/** Validate the opaque payload of a custom `navigate` event before giving it to a router.
 * Keep the allowed forms aligned with Fly's authored navigation URL policy. */
export function safeNavigationHref(raw: unknown): string | null {
  if (typeof raw !== "string") return null;
  const href = raw.trim();
  if (!href || href.length > 2048 || href.startsWith("//") || /[\s\\]/.test(href)) return null;
  for (const character of href) {
    const code = character.charCodeAt(0);
    if (code < 32 || (code >= 127 && code <= 159)) return null;
  }
  if (href.startsWith("/") || href.startsWith("#") || href.startsWith("?")) return href;
  if (/^(mailto|tel):.+/i.test(href)) return href;
  if (/^https?:\/\//i.test(href)) {
    try {
      const url = new URL(href);
      return (url.protocol === "https:" || url.protocol === "http:") && url.hostname ? href : null;
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * A landing has no owner-confirmed Product/Pricing quote. Hand off the exact SKU and quantity to
 * a host-supplied Product buyer URL instead of creating a cart with invented pricing context.
 * The buyer validates the SKU, reads Pricing, and requires a separate purchase confirmation.
 */
export function buyerHandoffHref(data: Record<string, unknown>, origin: string): string | null {
  const href = data.buyerHref;
  const variant = data.variantId;
  if (typeof href !== "string" || !href.startsWith("/") || !safeNavigationHref(href) ||
      typeof variant !== "string" ||
      !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(variant) ||
      /^0{8}(?:-0{4}){3}-0{12}$/.test(variant)) return null;
  const rawQuantity = data.quantity === undefined ? 1 : data.quantity;
  if ((typeof rawQuantity !== "number" &&
       (typeof rawQuantity !== "string" || !/^[0-9]{1,10}$/.test(rawQuantity))) ||
      !Number.isInteger(Number(rawQuantity)) || Number(rawQuantity) < 1 ||
      Number(rawQuantity) > 2147483647) return null;

  try {
    const url = new URL(href, origin);
    // The shared landing payload uses /[locale]/products/[handle] (the unprefixed form is
    // handed to this host's locale middleware). Leptos maps it to its module Product route.
    const route = /^\/(?:([^/]+)\/)?products\/([^/]+)\/?$/.exec(url.pathname);
    if (url.origin !== origin || !route) return null;
    // The host resolves locale support. Reject malformed or encoded path separators without
    // maintaining a second, narrower list of locales in this bridge.
    for (const segment of [route[1], route[2]]) {
      if (segment === undefined) continue;
      const decoded = decodeURIComponent(segment);
      if (!decoded || decoded.includes("/") || decoded.includes("\\")) return null;
      for (const character of decoded) {
        const code = character.charCodeAt(0);
        if (code < 32 || (code >= 127 && code <= 159)) return null;
      }
    }
    const keys = new Set<string>();
    for (const key of url.searchParams.keys()) {
      // Never overwrite a contradictory selection, cart recovery reference, or duplicate key.
      if (keys.has(key) || key === "handle" || key === "variant_id" || key === "quantity" ||
          key === "cart_id" || key.startsWith("axis.")) return null;
      keys.add(key);
    }
    url.searchParams.set("variant_id", variant.toLowerCase());
    url.searchParams.set("quantity", String(Number(rawQuantity)));
    return `${url.pathname}${url.search}${url.hash}`;
  } catch {
    return null;
  }
}

const LANDING_ROOT = "[data-fly-page-body], .fly-page-body";

function belongsToLanding(element: Element, root: HTMLElement): boolean {
  return element.closest(LANDING_ROOT) === root;
}

/** Install one bridge on this landing, never on the host document or a different landing. */
export function installLandingBridge(root: HTMLElement, handlers: LandingBridgeHandlers): () => void {
  const handleFlyAction = (event: Event) => {
    if (!(event.target instanceof Element) || !belongsToLanding(event.target, root)) return;
    const detail = (event as CustomEvent<FlyActionDetail>).detail;
    // Provider actions belong to their named provider, not to a similarly named host event.
    if (!detail || detail.kind !== "emit_event") return;
    const eventName = detail.event || "";
    const data = detail.payload || {};

    if (eventName === "add_to_cart" || eventName === "cart:add" || eventName === "buy_now") {
      const href = buyerHandoffHref(data, root.ownerDocument.defaultView?.location.origin ?? "");
      if (!href) {
        handlers.reportError("buyer_handoff_invalid");
        return;
      }
      handlers.clearError();
      handlers.navigate(href);
    } else if (eventName === "cart:open" || eventName === "open_cart") {
      handlers.openCart();
    } else if (eventName === "cart:close" || eventName === "close_cart") {
      handlers.closeCart();
    } else if (eventName === "cart:toggle" || eventName === "toggle_cart") {
      handlers.toggleCart();
    } else if (eventName === "navigate") {
      const href = safeNavigationHref(data.href || data.url);
      if (href) handlers.navigate(href);
    }
  };

  const handleLinkClick = (event: MouseEvent) => {
    const target = event.target;
    if (!(target instanceof Element) || !belongsToLanding(target, root) || event.defaultPrevented ||
        event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const trigger = target.closest("[data-fly-island='cart_trigger']");
    // A Fly action on the same click will be handled by the canonical runtime at document
    // level. Let that action own the click instead of toggling the cart twice.
    const action = target.closest("[data-fly-action]");
    if (action && root.contains(action)) return;
    if (trigger && root.contains(trigger)) {
      event.preventDefault();
      handlers.toggleCart();
      return;
    }
    const anchor = target.closest("a");
    if (!anchor || !root.contains(anchor) || anchor.target && anchor.target !== "_self" ||
        anchor.hasAttribute("download") || anchor.hasAttribute("data-fly-action")) return;
    const href = anchor.getAttribute("href");
    if (!href) return;
    if (href.startsWith("#") && href.length > 1) {
      let id: string;
      try {
        id = decodeURIComponent(href.slice(1));
      } catch {
        return;
      }
      const destination = Array.from(root.querySelectorAll("[id], a[name]"))
        .find((element) => belongsToLanding(element, root) &&
          (element.id === id || element.getAttribute("name") === id));
      if (destination) {
        event.preventDefault();
        destination.scrollIntoView({ behavior: "smooth" });
      }
    } else if (href.startsWith("/") && safeNavigationHref(href)) {
      event.preventDefault();
      handlers.navigate(href);
    }
  };

  root.addEventListener("fly:action", handleFlyAction);
  root.addEventListener("click", handleLinkClick);
  return () => {
    root.removeEventListener("fly:action", handleFlyAction);
    root.removeEventListener("click", handleLinkClick);
  };
}
