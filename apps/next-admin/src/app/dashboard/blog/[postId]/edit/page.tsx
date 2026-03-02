import { PageContainer } from '@/widgets/app-shell';
import { PostFormPage } from '@rustok/blog-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: Edit Post'
};

type PageProps = {
  params: Promise<{ postId: string }>;
};

export default async function Page(props: PageProps) {
  const { postId } = await props.params;

  return (
    <PageContainer scrollable pageTitle='Edit Post'>
      <Suspense fallback={<div>Loading form...</div>}>
        <PostFormPage postId={postId} />
      </Suspense>
    </PageContainer>
  );
}
