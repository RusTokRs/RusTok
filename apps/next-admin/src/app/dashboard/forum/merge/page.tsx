import { auth } from '@/auth';
import {
  ForumTopicMerge,
  listForumTopics
} from '../../../../../packages/forum/src';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Merge Forum Topics | RusToK Admin'
};

export default async function ForumTopicMergePage() {
  const session = await auth();
  const tenantId = session?.user?.tenantId ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const token = session?.user?.rustokToken ?? null;

  if (!tenantId) {
    return (
      <PageContainer
        pageTitle='Merge Forum Topics'
        pageDescription='Archive one Forum topic into a retained target.'
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
      pageTitle='Merge Forum Topics'
      pageDescription='Archive one source thread into a retained canonical target.'
    >
      <ForumTopicMerge topics={topics} gqlOpts={{ tenantId, tenantSlug }} />
    </PageContainer>
  );
}
