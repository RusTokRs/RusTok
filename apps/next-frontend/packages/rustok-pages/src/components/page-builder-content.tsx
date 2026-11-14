/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { FlyActionBridge } from "./fly-action-bridge";
import { scopeLandingCss } from "./css-scope";

export { scopeLandingCss };

/**
 * Host-owned containment for the landing wrapper. It used to be a JSX `style`
 * attribute, which a nonce-only `style-src-attr 'none'` policy drops. Moving it
 * into the nonce-bearing stylesheet keeps the landing isolated under strict CSP
 * without weakening the policy for inline attributes.
 */
const LANDING_CONTAINMENT_CSS =
  ".fly-page-container{isolation:isolate;contain:style layout}";

export interface PageBuilderContentProps {
  html: string;
  css: string;
  className?: string;
  /** Request-local CSP nonce issued by the storefront proxy. */
  nonce?: string;
}

export function PageBuilderContent({
  html,
  css,
  className = "",
  nonce,
}: PageBuilderContentProps) {
  // Pure derivation: this component is rendered as a Server Component on the
  // published page, where client hooks are unavailable. It is cheap enough to
  // compute once per request and stays deterministic across SSR and hydration.
  const scopedCss = scopeLandingCss(css, ".fly-page-container");
  const landingCss = `${LANDING_CONTAINMENT_CSS}\n${scopedCss}`;

  return (
    <div className={`fly-page-container ${className}`}>
      <style
        data-fly-landing-styles="true"
        nonce={nonce}
        dangerouslySetInnerHTML={{ __html: landingCss }}
      />
      <div
        className="fly-page-body"
        data-fly-page-body="true"
        dangerouslySetInnerHTML={{ __html: html }}
      />
      <FlyActionBridge />
    </div>
  );
}
