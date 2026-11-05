import React from 'react';

interface PricingPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default function PricingPage({
  token: _token,
  tenantSlug: _tenantSlug,
  tenantId: _tenantId
}: PricingPageProps) {
  return (
    <section className="space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Pricing & Price Lists</h1>
          <p className="text-muted-foreground text-sm">
            Manage product variant base prices, channel overrides, quantity tiers, and price list rules.
          </p>
        </div>
      </div>
      <div className="rounded-lg border bg-card p-6 text-card-foreground shadow-sm">
        <h2 className="text-lg font-semibold mb-2">Price Lists & Rules</h2>
        <p className="text-sm text-muted-foreground">
          Configure active promotional and channel price lists with percentage or fixed discount rules.
        </p>
      </div>
    </section>
  );
}

