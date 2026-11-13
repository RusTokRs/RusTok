/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

"use client";

// Byte-exact copy of the canonical Rust asset; its document listener is idempotent.
import "./fly-actions.js";

import { useLayoutEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { useTranslations } from "@rustok/next-fluent";
import { useCart } from "@rustok/cart-frontend";
import { installLandingBridge, type LandingBridgeError } from "./landing-bridge";

export type { FlyActionDetail } from "./landing-bridge";

export function FlyActionBridge() {
  const router = useRouter();
  const cart = useCart();
  const messages = useTranslations("PageBuilder");
  const [error, setError] = useState<LandingBridgeError | null>(null);
  const marker = useRef<HTMLSpanElement>(null);

  // Refresh handlers and counters before a committed capability change can be observed by
  // the browser or another host layout effect; the previous closure must not handle that scope.
  useLayoutEffect(() => {
    // The island is the landing body's sibling. Binding to that body avoids processing host
    // navigation and prevents multiple landing instances from handling the same event twice.
    const root = marker.current?.parentElement?.querySelector<HTMLElement>(":scope > .fly-page-body");
    if (!root) return;
    const uncertain = !cart.enabled || cart.actionError === "auth" ||
      cart.actionError === "restore" || cart.actionError === "unconfirmed";
    const selector = "[data-fly-island='cart_count']";
    const syncCounters = () => {
      for (const counter of root.querySelectorAll(selector)) {
        if (counter.closest("[data-fly-page-body], .fly-page-body") !== root) continue;
        const text = uncertain ? "—" : cart.isLoading || cart.isUpdating ? "…" : String(cart.itemCount);
        if (counter.textContent !== text) counter.textContent = text;
        counter.setAttribute("aria-busy", cart.isLoading || cart.isUpdating ? "true" : "false");
      }
    };
    syncCounters();
    // React can replace the injected landing HTML while preserving this island. Synchronize new
    // counters even if the Cart context itself did not change; disconnect on every cleanup.
    const observer = new MutationObserver((records) => {
      for (const record of records) {
        for (const node of record.addedNodes) {
          if (node.nodeType === 1) {
            const element = node as Element;
            if (element.matches(selector) || element.querySelector(selector)) {
              syncCounters();
              return;
            }
          }
        }
      }
    });
    observer.observe(root, { childList: true, subtree: true });
    const runCartAction = (action: () => void) => {
      if (!cart.enabled) {
        setError("cart_unavailable");
        return;
      }
      setError(null);
      action();
    };
    const removeBridge = installLandingBridge(root, {
      openCart: () => runCartAction(cart.openCart),
      closeCart: () => runCartAction(cart.closeCart),
      toggleCart: () => runCartAction(cart.toggleCart),
      reportError: setError,
      clearError: () => setError(null),
      navigate(href) {
        const url = new URL(href, window.location.href);
        if ((url.protocol === "http:" || url.protocol === "https:") && url.origin === window.location.origin) {
          router.push(`${url.pathname}${url.search}${url.hash}`);
        } else {
          window.location.assign(href);
        }
      },
    });
    return () => {
      observer.disconnect();
      removeBridge();
    };
  }, [cart, router]);

  return (
    <>
      <span ref={marker} hidden aria-hidden="true" />
      {error && (
        <p role="alert" data-fly-bridge-error={error}>
          {messages(error === "buyer_handoff_invalid" ? "buyerHandoffError" : "cartUnavailable")}
        </p>
      )}
    </>
  );
}
