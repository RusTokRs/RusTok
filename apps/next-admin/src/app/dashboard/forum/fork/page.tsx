import { auth } from '@/auth';
import {
  ForumTopicFork,
  listForumTopics
} from '../../../../../packages/forum/src';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Fork Forum Topic | RusToK Admin'
};

export default async function ForumTopicForkPage() {
  const session = await auth();
  const tenantId = session?.user?.tenantId ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const token = session?.user?.rustokToken ?? null;

  if (!tenantId) {
    return (
      <PageContainer
        pageTitle='Fork Forum Topic'
        pageDescription='Create a new Forum topic by copying one reply branch.'
      >
        <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-lg border p-4 text-sm'>
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
      pageTitle='Fork Forum Topic'
      pageDescription='Copy one reply branch into a new topic while preserving the source thread.'
    >
      <ForumTopicFork topics={topics} gqlOpts={{ tenantId, tenantSlug }} />
    </PageContainer>
  );
}
