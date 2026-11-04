import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { BrandsPage, listCatalogCategories, type CatalogCategorySummary } from '@rustok/product-admin';

export const metadata = {
  title: 'RusTok Admin: Brands'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  let categories: CatalogCategorySummary[] = [];
  try {
    categories = await listCatalogCategories(
      { token, tenantSlug, tenantId },
      'en'
    );
  } catch {
    // Graceful fallback if categories are not yet loaded
  }

  return (
    <PageContainer
      pageTitle='Brands'
      pageDescription='Manage brand directory, manufacturers, and category-scoped brand catalogs.'
    >
      <BrandsPage
        token={token}
        tenantSlug={tenantSlug}
        tenantId={tenantId}
        categories={categories}
      />
    </PageContainer>
  );
}
