/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export interface ArtifactContent {
  html: string;
  css: string;
  headHtml?: string;
}

/**
 * Extracts scoped CSS (<style>) and inner body markup (<body>) from a compiled
 * static landing artifact HTML document.
 */
export function parseArtifactHtml(documentHtml: string): ArtifactContent {
  const styleMatch = documentHtml.match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  const bodyMatch = documentHtml.match(/<body[^>]*>([\s\S]*?)<\/body>/i);
  return {
    css: styleMatch ? styleMatch[1].trim() : "",
    html: bodyMatch ? bodyMatch[1].trim() : documentHtml,
  };
}
