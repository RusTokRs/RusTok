import { PageContainer } from '@/widgets/app-shell';
import { PostFormPage } from '@rustok/blog-admin';
import { Suspense } from 'react';

export const metadata = {
  title: 'Dashboard: New Post'
};

export default function Page() {
  return (
    <PageContainer scrollable pageTitle='Create Post'>
      <Suspense fallback={<div>Loading form...</div>}>
        <PostFormPage />
      </Suspense>
    </PageContainer>
  );
}
