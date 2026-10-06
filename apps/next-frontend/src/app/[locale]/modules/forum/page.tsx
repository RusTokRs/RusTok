import type { Metadata } from 'next';
import { ForumSection } from '@rustok/forum-frontend';
import { getStorefrontTenantId, getStorefrontTenantSlug } from '@/shared/api/modules';

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  return {
    title: 'Community Forum | RusToK',
    description: 'Community discussions and knowledge base',
  };
}

export default async function ForumPage({
  params,
  searchParams,
}: {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { locale } = await params;
  const resolvedSearchParams = await searchParams;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  const category =
    typeof resolvedSearchParams.category === 'string'
      ? resolvedSearchParams.category
      : null;
  const topic =
    typeof resolvedSearchParams.topic === 'string'
      ? resolvedSearchParams.topic
      : null;
  const view =
    resolvedSearchParams.view === 'categories' ? 'categories' : 'topics';

  return (
    <main className="min-h-screen bg-background py-8">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <ForumSection
          tenantId={tenantId}
          tenantSlug={tenantSlug}
          locale={locale}
          initialCategoryId={category}
          initialTopicId={topic}
          initialView={view}
        />
      </div>
    </main>
  );
}
