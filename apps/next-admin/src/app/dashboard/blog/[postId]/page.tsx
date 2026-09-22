import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { PostDetailPage } from '../../../../../packages/blog/src';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Post Detail'
};

type PageProps = {
  params: Promise<{ postId: string }>;
};

export default async function Page(props: PageProps) {
  const { postId } = await props.params;
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer scrollable pageTitle='Post Detail'>
      <Suspense fallback={<div>Loading post...</div>}>
        <PostDetailPage
          postId={postId}
          token={token}
          tenantSlug={tenantSlug}
          tenantId={tenantId}
        />
      </Suspense>
    </PageContainer>
  );
}
