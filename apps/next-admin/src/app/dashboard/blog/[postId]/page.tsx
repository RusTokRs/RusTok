import { PageContainer } from '@/widgets/app-shell';
import { PostDetailPage } from '@rustok/blog-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Post Detail'
};

type PageProps = {
  params: Promise<{ postId: string }>;
};

export default async function Page(props: PageProps) {
  const { postId } = await props.params;

  return (
    <PageContainer scrollable pageTitle='Post Detail'>
      <Suspense fallback={<div>Loading post...</div>}>
        <PostDetailPage postId={postId} />
      </Suspense>
    </PageContainer>
  );
}
