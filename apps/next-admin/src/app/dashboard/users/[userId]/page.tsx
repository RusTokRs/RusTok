import { PageContainer } from '@/widgets/app-shell';
import { Metadata } from 'next';
import UserDetailView from '@/features/users/components/user-detail-view';

export const metadata: Metadata = {
  title: 'Dashboard : User Detail',
  description: 'View user details'
};

export default async function UserDetailPage({
  params
}: {
  params: Promise<{ userId: string }>;
}) {
  const { userId } = await params;
  return (
    <PageContainer
      pageTitle='User Detail'
      pageDescription='View and manage user information'
    >
      <UserDetailView userId={userId} />
    </PageContainer>
  );
}
