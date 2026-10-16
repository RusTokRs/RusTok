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

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useCart } from "@rustok/cart-frontend";
import { initFlyActions } from "./fly-actions-runtime";

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

export function FlyActionBridge() {
  const router = useRouter();
  const cart = useCart();

  useEffect(() => {
    // 1. Initialize fly-actions runtime (idempotent)
    initFlyActions();

    // 2. Handle CustomEvents dispatched by fly actions
    const handleFlyAction = (e: Event) => {
      const customEvent = e as CustomEvent<FlyActionDetail>;
      const detail = customEvent.detail;
      if (!detail) return;

      const eventName = detail.event || detail.action || "";
      const data = detail.payload || detail.input || {};

      if (
        eventName === "add_to_cart" ||
        eventName === "cart:add" ||
        eventName === "buy_now"
      ) {
        const variantId = (data.variantId || data.productId || data.id) as string | undefined;
        const quantity = Number(data.quantity) || 1;
        if (variantId) {
          cart.addItem(variantId, quantity).then((success) => {
            if (success) {
              cart.openCart();
            }
          });
        }
      } else if (eventName === "cart:open" || eventName === "open_cart") {
        cart.openCart();
      } else if (eventName === "cart:close" || eventName === "close_cart") {
        cart.closeCart();
      } else if (eventName === "cart:toggle" || eventName === "toggle_cart") {
        cart.toggleCart();
      } else if (eventName === "navigate") {
        const href = (data.href || data.url) as string | undefined;
        if (href && typeof href === "string") {
          router.push(href);
        }
      }
    };

    // 3. Sync cart count islands with live cart state
    const counters = document.querySelectorAll("[data-fly-island='cart_count']");
    for (const counter of counters) {
      counter.textContent = String(cart.itemCount);
    }

    // 4. Progressive enhancement: intercept internal <a> clicks for client-side navigation
    const handleLinkClick = (e: MouseEvent) => {
      const target = e.target as Element | null;
      if (!target || typeof target.closest !== "function") return;

      // Handle cart trigger islands
      if (target.closest("[data-fly-island='cart_trigger']")) {
        e.preventDefault();
        cart.toggleCart();
        return;
      }

      const anchor = target.closest("a");
      if (!anchor) return;

      // Don't intercept target="_blank", downloads, or action triggers
      if (
        anchor.target === "_blank" ||
        anchor.hasAttribute("download") ||
        anchor.hasAttribute("data-fly-action")
      ) {
        return;
      }

      const href = anchor.getAttribute("href");
      if (!href) return;

      // Intercept in-page hash links for smooth scrolling
      if (href.startsWith("#")) {
        const el = document.querySelector(href);
        if (el) {
          e.preventDefault();
          el.scrollIntoView({ behavior: "smooth" });
        }
        return;
      }

      // Only intercept same-origin relative URLs
      if (href.startsWith("/") && !href.startsWith("//")) {
        e.preventDefault();
        router.push(href);
      }
    };

    window.addEventListener("fly:action", handleFlyAction as EventListener);
    document.addEventListener("click", handleLinkClick, { capture: false });

    return () => {
      window.removeEventListener("fly:action", handleFlyAction as EventListener);
      document.removeEventListener("click", handleLinkClick);
    };
  }, [cart, router]);

  return null;
}
