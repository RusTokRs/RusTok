import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { fetchCurrentUser } from '@/shared/api/auth-api';
import { PagesPage } from '@rustok/pages-admin';
import { getLocale } from '@rustok/next-fluent/server';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Pages'
};

export default async function Page() {
  const session = await auth();
  const locale = await getLocale();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;
  // A failed permission lookup hides the authoring action; the server still enforces it.
  const currentUser = token ? await fetchCurrentUser(token, tenantSlug) : null;
  const canManageStarters = (currentUser?.permissions ?? []).some((value) =>
    ['pages:manage', 'pages:*'].includes(value.trim().toLowerCase())
  );
  // Origin of the Rust (FFA/Leptos) admin that owns the visual builder UI. When it
  // is not configured, the pages surface still works and simply hides builder links.
  const builderBaseUrl = process.env.NEXT_PUBLIC_RUST_ADMIN_URL ?? null;
  const storefrontBaseUrl =
    process.env.NEXT_PUBLIC_STOREFRONT_URL ??
    process.env.STOREFRONT_URL ??
    null;

  return (
    <PageContainer
      scrollable
      pageTitle='Pages'
      pageDescription='Pages and builder'
    >
      <Suspense fallback={<p>Loading pages…</p>}>
        <PagesPage
          token={token}
          tenantSlug={tenantSlug}
          tenantId={tenantId}
          locale={locale}
          canManageStarters={canManageStarters}
          builderBaseUrl={builderBaseUrl}
          storefrontBaseUrl={storefrontBaseUrl}
        />
      </Suspense>
    </PageContainer>
  );
}
