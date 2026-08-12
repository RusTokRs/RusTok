import { auth } from '@/auth';
import {
  ForumTopicSplit,
  listForumTopics
} from '../../../../../packages/forum/src';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Split Forum Topic | RusToK Admin'
};

export default async function ForumTopicSplitPage() {
  const session = await auth();
  const tenantId = session?.user?.tenantId ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const token = session?.user?.rustokToken ?? null;

  if (!tenantId) {
    return (
      <PageContainer
        pageTitle='Split Forum Topic'
        pageDescription='Create a new Forum topic from selected replies.'
      >
        <div className='rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive'>
          Tenant context is required.
        </div>
      </PageContainer>
    );
  }

  const topics = await listForumTopics(
    { token, tenantId, tenantSlug },
    { first: 100 }
  );

  return (
    <PageContainer
      scrollable
      pageTitle='Split Forum Topic'
      pageDescription='Move one parent-closed reply set into a new topic without changing reply identity.'
    >
      <ForumTopicSplit topics={topics} gqlOpts={{ tenantId, tenantSlug }} />
    </PageContainer>
  );
}
