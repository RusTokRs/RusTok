import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { ForumReplyEditor } from '@rustok/blog-admin';

export const metadata = {
  title: 'Dashboard: Forum Reply Composer'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Forum Reply Composer'
      pageDescription='Draft rt_json_v1 replies for forum topics.'
    >
      <ForumReplyEditor
        topicId='00000000-0000-0000-0000-000000000000'
        gqlOpts={{ token, tenantSlug, tenantId: tenantId ?? undefined }}
      />
    </PageContainer>
  );
}
