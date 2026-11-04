import type { ReactNode } from "react";
import { setRequestLocale } from "@rustok/next-fluent/server";
import { StorefrontProviders } from "@/widgets/providers/storefront-providers";
import { getStorefrontTenantSlug } from "@/shared/api/modules";

export default async function LocaleLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;

  setRequestLocale(locale);
  const tenantSlug = getStorefrontTenantSlug();

  return (
    <StorefrontProviders locale={locale} tenantSlug={tenantSlug}>
      {children}
    </StorefrontProviders>
  );
}
