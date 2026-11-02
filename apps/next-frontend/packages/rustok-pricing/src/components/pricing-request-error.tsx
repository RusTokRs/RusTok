/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

"use client";

import { useTranslations } from "@rustok/next-fluent";
import { Button } from "@/shared/ui/base/button";

export function PricingRequestError({ resetHref }: { resetHref: string }) {
  const messages = useTranslations("Pricing");
  return (
    <section
      role="alert"
      className="space-y-4 rounded-2xl border border-destructive/30 bg-destructive/10 p-6 text-sm text-destructive"
    >
      <p>{messages("invalidRequest")}</p>
      <Button asChild variant="outline">
        <a href={resetHref}>{messages("resetContext")}</a>
      </Button>
    </section>
  );
}
