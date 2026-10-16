/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { useMemo } from "react";
import { FlyActionBridge } from "./fly-action-bridge";
import { scopeLandingCss } from "./css-scope";

export { scopeLandingCss };

export interface PageBuilderContentProps {
  html: string;
  css: string;
  className?: string;
}

export function PageBuilderContent({
  html,
  css,
  className = "",
}: PageBuilderContentProps) {
  const scopedCss = useMemo(() => scopeLandingCss(css, ".fly-page-container"), [css]);

  return (
    <div
      className={`fly-page-container ${className}`}
      style={{ isolation: "isolate", contain: "style layout" }}
    >
      {scopedCss ? <style dangerouslySetInnerHTML={{ __html: scopedCss }} /> : null}
      <div
        className="fly-page-body"
        dangerouslySetInnerHTML={{ __html: html }}
      />
      <FlyActionBridge />
    </div>
  );
}
