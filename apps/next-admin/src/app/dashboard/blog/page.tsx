import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { buttonVariants } from '@/shared/ui/shadcn/button';
import { DataTableSkeleton } from '@/widgets/data-table';
import { PostsPage } from '@/features/blog';
import { cn } from '@/shared/lib/utils';
import { IconPlus } from '@tabler/icons-react';
import Link from 'next/link';
import { SearchParams } from 'nuqs/server';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Blog Posts'
};

type PageProps = {
  searchParams: Promise<SearchParams>;
};

export default async function Page(props: PageProps) {
  const searchParams = await props.searchParams;
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer
      scrollable={false}
      pageTitle='Blog Posts'
      pageDescription='Manage blog posts'
      pageHeaderAction={
        <Link
          href='/dashboard/blog/new'
          className={cn(buttonVariants(), 'text-xs md:text-sm')}
        >
          <IconPlus className='mr-2 h-4 w-4' /> New Post
        </Link>
      }
    >
      <Suspense
        fallback={
          <DataTableSkeleton columnCount={6} rowCount={8} filterCount={2} />
        }
      >
        <PostsPage
          searchParams={{
            page: searchParams.page as string | undefined,
            perPage: searchParams.perPage as string | undefined,
            title: searchParams.title as string | undefined,
            status: searchParams.status as string | undefined
          }}
          token={token}
          tenantSlug={tenantSlug}
          tenantId={tenantId}
        />
      </Suspense>
    </PageContainer>
  );
}
