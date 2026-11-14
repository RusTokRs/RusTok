/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { headers } from "next/headers";
import { CSP_NONCE_HEADER, isValidCspNonce } from "./csp";

/**
 * Reads the request-local nonce the proxy issued. A missing, malformed or
 * overlong value is answered as `undefined`: the renderer then emits no nonce
 * attribute rather than copying an untrusted header into markup. Absence is
 * safe because the proxy is the only producer of the enforcing CSP header.
 *
 * `headers()` opts the route into dynamic rendering. That is required: a
 * statically generated document cannot carry a per-request nonce.
 */
export async function getRequestCspNonce(): Promise<string | undefined> {
  const value = (await headers()).get(CSP_NONCE_HEADER);
  return isValidCspNonce(value) ? value : undefined;
}
