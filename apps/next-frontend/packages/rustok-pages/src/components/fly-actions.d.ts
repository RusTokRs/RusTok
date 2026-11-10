/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Type declarations for `fly-actions.js` — a byte-exact copy of the canonical
 * action runtime asset served by the Rust storefront
 * (`crates/ui/fly/browser/assets/fly-actions.js`).
 * `apps/next-frontend/scripts/verify-fly-actions-parity.mjs` fails when the
 * copies drift, so this module must stay a verbatim copy: never reformat,
 * lint, or "modernize" it.
 *
 * The asset is a self-starting, idempotent IIFE with no module exports:
 * importing it for side effects installs the document-level click/touch
 * listeners that materialized `data-fly-action` attributes rely on, exactly
 * like the `<script data-fly-actions>` tag the Rust storefront inlines.
 */

/** A materialized action payload parsed from a `data-fly-action` attribute. */
export type FlyActionPayload = {
  kind: string;
  [key: string]: unknown;
};

/** The manual-control adapter returned by `FlyActions.start`. */
export type FlyActionsAdapter = {
  stop(): void;
};

/** The `globalThis.FlyActions` surface the asset installs in browsers. */
export type FlyActionsApi = {
  start(doc?: Document): FlyActionsAdapter;
  handleAction(action: FlyActionPayload, element: Element): boolean;
  parseAction(element: Element): FlyActionPayload | null;
};

declare global {
  /**
   * Installed by the fly-actions runtime when the script executes in a
   * browser. Feature-detect before use; the value is `undefined` during SSR
   * and in hosts that never ran the asset.
   */
  var FlyActions: FlyActionsApi | undefined;
}
