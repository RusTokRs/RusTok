import { auth } from '@/auth';
import { buttonVariants } from '@/shared/ui/shadcn/button';
import { cn } from '@/shared/lib/utils';
import { getPage, listPages } from '@/features/blog/api/pages';
import Link from 'next/link';
import { SearchParams } from 'nuqs/server';
import { PageContainer } from '@/widgets/app-shell';
import { PageBuilder } from '@/features/blog';
import {
  buildRouteSelectionHref,
  listRouteQueryEntries,
  readRouteSelection
} from '@/shared/lib/route-selection';

export const metadata = {
  title: 'Dashboard: Page Builder'
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
  const gqlOpts = { token, tenantSlug, tenantId: tenantId ?? undefined };
  const pages = tenantId ? await listPages(gqlOpts) : [];
  const requestedPageId = readRouteSelection(searchParams, 'page_id');
  const selectedPageId =
    requestedPageId && pages.some((page) => page.id === requestedPageId)
      ? requestedPageId
      : undefined;
  const selectedPage = selectedPageId
    ? await getPage(selectedPageId, gqlOpts)
    : null;
  const preservedQueryEntries = listRouteQueryEntries(searchParams, ['page_id']);

  return (
    <PageContainer
      scrollable
      pageTitle='Page Builder'
      pageDescription={
        selectedPage?.translation?.title
          ? `Edit GrapesJS project data for "${selectedPage.translation.title}".`
          : 'Edit GrapesJS project data for Pages module payloads.'
      }
      pageHeaderAction={
        <form method='get' className='flex items-center gap-2'>
          {preservedQueryEntries.map(([key, value]) => (
            <input key={`${key}:${value}`} type='hidden' name={key} value={value} />
          ))}
          <select
            name='page_id'
            defaultValue={selectedPageId ?? ''}
            className='h-9 min-w-60 rounded-md border border-input bg-background px-3 text-sm'
          >
            {pages.length === 0 ? (
              <option value=''>No pages available</option>
            ) : (
              pages.map((page) => (
                <option key={page.id} value={page.id}>
                  {page.title ?? page.slug ?? page.id}
                </option>
              ))
            )}
          </select>
          <button className={cn(buttonVariants({ variant: 'outline' }), 'h-9')} type='submit'>
            Open
          </button>
          {selectedPageId && (
            <Link
              href={buildRouteSelectionHref(
                '/dashboard/blog/page-builder',
                searchParams,
                'page_id',
                selectedPageId
              )}
              className={cn(buttonVariants({ variant: 'ghost' }), 'h-9 px-3')}
            >
              Refresh
            </Link>
          )}
        </form>
      }
    >
      {selectedPageId ? (
        <PageBuilder
          key={selectedPageId}
          pageId={selectedPageId}
          initialBody={selectedPage?.body ?? null}
          initialBlocks={selectedPage?.blocks ?? []}
          initialLocale={selectedPage?.translation?.locale ?? undefined}
          pageTitle={selectedPage?.translation?.title ?? null}
          gqlOpts={gqlOpts}
        />
      ) : (
        <div className='text-muted-foreground rounded-md border border-dashed p-6 text-sm'>
          Pages module has no selectable pages yet. Create a page first, then reopen the builder.
        </div>
      )}
    </PageContainer>
  );
}
