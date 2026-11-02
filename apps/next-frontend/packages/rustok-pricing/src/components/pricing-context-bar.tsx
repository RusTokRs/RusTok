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

import React from "react";
import { useTranslations } from "@rustok/next-fluent";
import type {
  PricingChannelOption,
  PricingPriceListOption,
  PricingResolutionContext,
} from "../types";

export function PricingContextBar({
  availableCurrencies,
  availableChannels,
  activePriceLists,
  context,
  onChange,
  locale = "en",
}: {
  availableCurrencies: string[];
  availableChannels: PricingChannelOption[];
  activePriceLists: PricingPriceListOption[];
  context?: PricingResolutionContext | null;
  onChange: (updated: Partial<PricingResolutionContext>) => void;
  locale?: string | null;
}): React.JSX.Element {
  const pricingMessages = useTranslations("Pricing");
  const isRu = locale === "ru";

  return (
    <div className="flex flex-wrap items-center gap-4 rounded-2xl border border-border bg-card/60 p-4">
      {availableChannels.length > 0 && (
        <label className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
          <span>{isRu ? "Канал:" : "Channel:"}</span>
          <select
            value={context?.channelId ?? ""}
            onChange={(e) => onChange({ channelId: e.target.value || null })}
            className="rounded-xl border border-border bg-background px-3 py-1.5 text-xs text-foreground outline-none focus:border-primary"
          >
            <option value="">{pricingMessages("currentChannel")}</option>
            {availableChannels.map((channel) => (
              <option key={channel.id} value={channel.id}>
                {channel.name} ({channel.slug})
              </option>
            ))}
          </select>
        </label>
      )}

      {activePriceLists.length > 0 && (
        <label className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
          <span>{isRu ? "Прайс-лист:" : "Price List:"}</span>
          <select
            disabled={!context?.currencyCode}
            value={context?.priceListId ?? ""}
            onChange={(e) => onChange({ priceListId: e.target.value || null })}
            className="rounded-xl border border-border bg-background px-3 py-1.5 text-xs text-foreground outline-none focus:border-primary"
          >
            <option value="">{isRu ? "По умолчанию" : "Default"}</option>
            {activePriceLists.map((list) => (
              <option key={list.id} value={list.id}>
                {list.name} ({list.listType})
              </option>
            ))}
          </select>
        </label>
      )}

      <label className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
        <span>{isRu ? "Валюта:" : "Currency:"}</span>
        <select
          value={context?.currencyCode ?? ""}
          onChange={(e) => onChange({ currencyCode: e.target.value })}
          className="rounded-xl border border-border bg-background px-3 py-1.5 text-xs text-foreground outline-none focus:border-primary"
        >
          <option value="">—</option>
          {availableCurrencies.map((currency) => (
            <option key={currency} value={currency}>
              {currency}
            </option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
        <span>{isRu ? "Количество:" : "Quantity:"}</span>
        <input
          disabled={!context?.currencyCode}
          type="number"
          min="1"
          max="2147483647"
          step="1"
          value={context?.quantity ?? 1}
          onChange={(e) => onChange({ quantity: e.target.valueAsNumber })}
          className="w-16 rounded-xl border border-border bg-background px-2.5 py-1.5 text-center text-xs text-foreground outline-none focus:border-primary"
        />
      </label>
    </div>
  );
}
