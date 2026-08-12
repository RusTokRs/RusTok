import { auth } from '@/auth';
import {
  ForumTopicReplyRange,
  listForumTopics
} from '../../../../../packages/forum/src';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Move Forum Reply Range | RusToK Admin'
};

export default async function ForumTopicReplyRangePage() {
  const session = await auth();
  const tenantId = session?.user?.tenantId ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const token = session?.user?.rustokToken ?? null;

  if (!tenantId) {
    return (
      <PageContainer
        pageTitle='Move Forum Reply Range'
        pageDescription='Move an inclusive reply-position range into an existing topic.'
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
      pageTitle='Move Forum Reply Range'
      pageDescription='Move an exact owner-position range into an existing topic without transport-local movement policy.'
    >
      <ForumTopicReplyRange
        topics={topics}
        gqlOpts={{ tenantId, tenantSlug }}
      />
    </PageContainer>
  );
}
