import Link from 'next/link';

import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { StatCard } from '@/widgets/stats-card';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  listForumCategories,
  listForumTopics
} from '@rustok/forum-admin';
import {
  IconMessages,
  IconCategory,
  IconCheck,
  IconMessage2
} from '@tabler/icons-react';

export const metadata = {
  title: 'Dashboard: Forum'
};

const MODERATION_TOOLS = [
  { title: 'Categories', href: '/dashboard/forum/categories', hint: 'Taxonomy and posting policies' },
  { title: 'Topic Composer', href: '/dashboard/forum/topic', hint: 'Create a new topic' },
  { title: 'Reply Composer', href: '/dashboard/forum/reply', hint: 'Post a moderated reply' },
  { title: 'Merge Topics', href: '/dashboard/forum/merge', hint: 'Combine duplicate threads' },
  { title: 'Split Replies', href: '/dashboard/forum/split', hint: 'Move replies into a new topic' },
  { title: 'Fork Branch', href: '/dashboard/forum/fork', hint: 'Copy a reply branch aside' },
  { title: 'Reply Range', href: '/dashboard/forum/reply-range', hint: 'Select and move a range' },
  { title: 'Rename Slug', href: '/dashboard/forum/rename-slug', hint: 'Retitle a topic slug' }
];

export default async function ForumPage() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  const opts = { token, tenantSlug, tenantId };

  let categories: Awaited<ReturnType<typeof listForumCategories>> = [];
  let topics: Awaited<ReturnType<typeof listForumTopics>> = [];
  let error: string | null = null;

  if (tenantId) {
    try {
      [categories, topics] = await Promise.all([
        listForumCategories(opts),
        listForumTopics(opts)
      ]);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load forum overview.';
    }
  }

  const categoryName = new Map(categories.map((category) => [category.id, category.name]));
  const replies = topics.reduce((sum, topic) => sum + (topic.replyCount ?? 0), 0);
  const solved = topics.filter((topic) => topic.solutionReplyId != null).length;
  const recentTopics = topics.slice(0, 10);

  return (
    <PageContainer
      scrollable
      pageTitle='Forum'
      pageDescription='Community health at a glance and quick access to moderation tools.'
    >
      <div className='flex flex-1 flex-col gap-6'>
        {!tenantId ? (
          <div className='text-muted-foreground rounded-md border border-dashed p-6 text-sm'>
            Select a tenant before managing the forum.
          </div>
        ) : error ? (
          <Card className='border-destructive/50'>
            <CardContent className='text-destructive py-6 text-sm'>
              {error}
            </CardContent>
          </Card>
        ) : (
          <>
            <div className='grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4'>
              <StatCard
                title='Categories'
                value={categories.length.toLocaleString()}
                helper='forum taxonomy nodes'
                icon={<IconCategory className='size-5' />}
              />
              <StatCard
                title='Topics'
                value={topics.length.toLocaleString()}
                helper='across all locales'
                icon={<IconMessages className='size-5' />}
              />
              <StatCard
                title='Replies'
                value={replies.toLocaleString()}
                helper='total thread activity'
                icon={<IconMessage2 className='size-5' />}
              />
              <StatCard
                title='Solved topics'
                value={solved.toLocaleString()}
                helper='with an accepted solution'
                icon={<IconCheck className='size-5' />}
              />
            </div>

            <div className='grid grid-cols-1 gap-4 xl:grid-cols-[1.35fr_0.65fr]'>
              <Card>
                <CardHeader>
                  <CardTitle>Topics</CardTitle>
                  <CardDescription>
                    Latest forum threads with category and solution state.
                  </CardDescription>
                </CardHeader>
                <CardContent className='flex flex-col gap-2'>
                  {recentTopics.length === 0 ? (
                    <p className='text-muted-foreground text-sm'>
                      No forum topics yet. Create the first one from the Topic
                      Composer.
                    </p>
                  ) : (
                    recentTopics.map((topic) => (
                      <div
                        key={topic.id}
                        className='flex items-center justify-between gap-3 rounded-md border p-2.5'
                      >
                        <div className='flex min-w-0 flex-col gap-0.5'>
                          <span className='text-foreground truncate text-sm font-medium'>
                            {topic.title}
                          </span>
                          <span className='text-muted-foreground text-[11px]'>
                            {categoryName.get(topic.categoryId) ?? 'Uncategorized'}{' '}
                            · {topic.replyCount ?? 0}{' '}
                            {(topic.replyCount ?? 0) === 1 ? 'reply' : 'replies'}
                          </span>
                        </div>
                        <div className='flex shrink-0 items-center gap-1.5'>
                          <Badge variant='outline' className='font-mono text-[10px]'>
                            {topic.effectiveLocale}
                          </Badge>
                          {topic.solutionReplyId != null ? (
                            <Badge
                              variant='outline'
                              className='border-emerald-500/30 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300'
                            >
                              Solved
                            </Badge>
                          ) : null}
                        </div>
                      </div>
                    ))
                  )}
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <CardTitle>Moderation tools</CardTitle>
                  <CardDescription>
                    Operator surfaces for forum maintenance.
                  </CardDescription>
                </CardHeader>
                <CardContent className='grid grid-cols-1 gap-2'>
                  {MODERATION_TOOLS.map((tool) => (
                    <Link
                      key={tool.href}
                      href={tool.href}
                      className='hover:bg-accent flex flex-col rounded-md border p-2.5 transition'
                    >
                      <span className='text-foreground text-sm font-medium'>
                        {tool.title}
                      </span>
                      <span className='text-muted-foreground text-[11px]'>
                        {tool.hint}
                      </span>
                    </Link>
                  ))}
                </CardContent>
              </Card>
            </div>
          </>
        )}
      </div>
    </PageContainer>
  );
}
