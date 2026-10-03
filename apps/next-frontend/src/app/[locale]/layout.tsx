import type { ReactNode } from "react";
import { setRequestLocale } from "@rustok/next-fluent/server";
import { CartProvider, CartDrawer, CartTrigger } from "@rustok/cart-frontend";
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
    <CartProvider locale={locale} tenantSlug={tenantSlug}>
      {children}
      <CartTrigger locale={locale} />
      <CartDrawer locale={locale} />
    </CartProvider>
  );
}
