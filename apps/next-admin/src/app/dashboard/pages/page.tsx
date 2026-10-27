import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { PagesPage } from '@rustok/pages-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Pages'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;
  // Origin of the Rust (FFA/Leptos) admin that owns the visual builder UI. When it
  // is not configured, the pages surface still works and simply hides builder links.
  const builderBaseUrl = process.env.NEXT_PUBLIC_RUST_ADMIN_URL ?? null;
  const storefrontBaseUrl =
    process.env.NEXT_PUBLIC_STOREFRONT_URL ??
    process.env.STOREFRONT_URL ??
    null;
  const previewSecret =
    process.env.REVALIDATE_SECRET ??
    process.env.PREVIEW_SECRET ??
    null;

  return (
    <PageContainer scrollable pageTitle="Pages" pageDescription="Pages and builder">
      <Suspense fallback={<p>Loading pages…</p>}>
        <PagesPage
          token={token}
          tenantSlug={tenantSlug}
          tenantId={tenantId}
          builderBaseUrl={builderBaseUrl}
          storefrontBaseUrl={storefrontBaseUrl}
          previewSecret={previewSecret}
        />
      </Suspense>
    </PageContainer>
  );
}
