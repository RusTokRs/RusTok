import { auth } from '@/auth';
import { cn } from '@/shared/lib/utils';
import {
  listRouteQueryEntries,
  readRouteSelection
} from '@/shared/lib/route-selection';
import { buttonVariants } from '@/shared/ui/shadcn/button';
import { PageContainer } from '@/widgets/app-shell';
import Link from 'next/link';
import { SearchParams } from 'nuqs/server';
import {
  ForumTopicEditor,
  getForumTopic,
  listForumCategories,
  listForumTopics
} from '../../../../../packages/forum/src';

export const metadata = {
  title: 'Dashboard: Forum Topic Composer'
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
  const [topics, categories] = tenantId
    ? await Promise.all([
        listForumTopics(gqlOpts),
        listForumCategories(gqlOpts)
      ])
    : [[], []];
  const requestedTopicId = readRouteSelection(searchParams, 'topic_id');
  const selectedTopicSummary = requestedTopicId
    ? (topics.find((topic) => topic.id === requestedTopicId) ?? null)
    : null;
  const selectedTopic = selectedTopicSummary
    ? await getForumTopic(
        selectedTopicSummary.id,
        gqlOpts,
        selectedTopicSummary.locale
      )
    : null;
  const preservedQueryEntries = listRouteQueryEntries(searchParams, [
    'topic_id'
  ]);

  return (
    <PageContainer
      scrollable
      pageTitle='Forum Topic Composer'
      pageDescription={
        selectedTopic
          ? 'Edit the selected forum topic translation.'
          : 'Create a forum topic with the shared richtext editor.'
      }
      pageHeaderAction={
        <div className='flex items-center gap-2'>
          <form method='get' className='flex items-center gap-2'>
            {preservedQueryEntries.map(([key, value]) => (
              <input
                key={`${key}:${value}`}
                type='hidden'
                name={key}
                value={value}
              />
            ))}
            <select
              name='topic_id'
              defaultValue={selectedTopicSummary?.id ?? ''}
              className='border-input bg-background h-9 min-w-60 rounded-md border px-3 text-sm'
            >
              <option value=''>New topic</option>
              {topics.map((topic) => (
                <option
                  key={topic.id}
                  value={topic.id}
                  lang={topic.effectiveLocale}
                  dir='auto'
                >
                  {topic.title}
                </option>
              ))}
            </select>
            <button
              className={cn(buttonVariants({ variant: 'outline' }), 'h-9')}
              type='submit'
            >
              Open
            </button>
          </form>
          {selectedTopic && (
            <Link
              href='/dashboard/forum/topic'
              className={cn(buttonVariants(), 'h-9')}
            >
              New topic
            </Link>
          )}
        </div>
      }
    >
      {tenantId ? (
        <ForumTopicEditor
          key={selectedTopic?.id ?? 'new'}
          initialData={selectedTopic}
          categories={categories}
          gqlOpts={gqlOpts}
        />
      ) : (
        <div className='text-muted-foreground rounded-md border border-dashed p-6 text-sm'>
          Select a tenant before creating or editing forum topics.
        </div>
      )}
    </PageContainer>
  );
}
