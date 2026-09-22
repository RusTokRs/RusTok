import { auth } from '@/auth';
import {
  ForumTopicSlugRename,
  listForumTopics
} from '../../../../../packages/forum/src';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Rename Forum Topic Route | RusToK Admin'
};

export default async function ForumTopicSlugRenamePage() {
  const session = await auth();
  const tenantId = session?.user?.tenantId ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const token = session?.user?.rustokToken ?? null;

  if (!tenantId) {
    return (
      <PageContainer
        pageTitle='Rename Forum Topic Route'
        pageDescription='Change one existing localized topic slug.'
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
      pageTitle='Rename Forum Topic Route'
      pageDescription='Preserve an existing localized route as an immutable redirect while changing its canonical slug.'
    >
      <ForumTopicSlugRename
        topics={topics}
        gqlOpts={{ tenantId, tenantSlug }}
      />
    </PageContainer>
  );
}
