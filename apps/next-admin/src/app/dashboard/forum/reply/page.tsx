import { auth } from '@/auth';
import { buttonVariants } from '@/shared/ui/shadcn/button';
import { listForumTopics } from '@/features/blog/api/forum';
import { cn } from '@/shared/lib/utils';
import { SearchParams } from 'nuqs/server';
import { PageContainer } from '@/widgets/app-shell';
import { ForumReplyEditor } from '@/features/blog';
import { listRouteQueryEntries, readRouteSelection } from '@/shared/lib/route-selection';

export const metadata = {
  title: 'Dashboard: Forum Reply Composer'
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
  const topics = tenantId ? await listForumTopics(gqlOpts) : [];
  const requestedTopicId = readRouteSelection(searchParams, 'topic_id');
  const selectedTopic = requestedTopicId
    ? topics.find((topic) => topic.id === requestedTopicId) ?? null
    : null;
  const preservedQueryEntries = listRouteQueryEntries(searchParams, ['topic_id']);

  return (
    <PageContainer
      scrollable
      pageTitle='Forum Reply Composer'
      pageDescription={
        selectedTopic
          ? `Draft rt_json_v1 replies for "${selectedTopic.title}".`
          : 'Draft rt_json_v1 replies for forum topics.'
      }
      pageHeaderAction={
        <form method='get' className='flex items-center gap-2'>
          {preservedQueryEntries.map(([key, value]) => (
            <input key={`${key}:${value}`} type='hidden' name={key} value={value} />
          ))}
          <select
            name='topic_id'
            defaultValue={selectedTopic?.id ?? ''}
            className='h-9 min-w-60 rounded-md border border-input bg-background px-3 text-sm'
          >
            {topics.length === 0 ? (
              <option value=''>No topics available</option>
            ) : (
              topics.map((topic) => (
                <option key={topic.id} value={topic.id}>
                  {topic.title}
                </option>
              ))
            )}
          </select>
          <button className={cn(buttonVariants({ variant: 'outline' }), 'h-9')} type='submit'>
            Open
          </button>
        </form>
      }
    >
      {selectedTopic ? (
        <ForumReplyEditor topicId={selectedTopic.id} gqlOpts={gqlOpts} />
      ) : (
        <div className='text-muted-foreground rounded-md border border-dashed p-6 text-sm'>
          Forum module has no selectable topics yet. Create a topic first, then reopen the reply editor.
        </div>
      )}
    </PageContainer>
  );
}
