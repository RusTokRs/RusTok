import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { SearchParams } from 'nuqs/server';
import { CategoryTreeAdmin, fetchAdminCategoryTree } from '@rustok/forum-admin';

export const metadata = {
  title: 'Dashboard: Forum Category Management'
};

type PageProps = {
  searchParams: Promise<SearchParams>;
};

export default async function Page(props: PageProps) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;
  const gqlOpts = { token, tenantSlug, tenantId: tenantId ?? undefined };

  const categories = tenantId ? await fetchAdminCategoryTree(gqlOpts) : [];

  return (
    <PageContainer
      scrollable
      pageTitle='Forum Categories'
      pageDescription='Organize forum taxonomy, category hierarchy, subcategory trees, and direct topic posting policies.'
    >
      {tenantId ? (
        <CategoryTreeAdmin initialCategories={categories} gqlOpts={gqlOpts} />
      ) : (
        <div className='text-muted-foreground rounded-md border border-dashed p-6 text-sm'>
          Select a tenant before managing forum categories.
        </div>
      )}
    </PageContainer>
  );
}
