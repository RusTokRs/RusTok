import { Metadata } from 'next';
import SignInViewPage from '@/features/auth/components/sign-in-view';

export const metadata: Metadata = {
  title: 'Sign In | RusTok Admin',
  description: 'Sign in to RusTok Admin panel.'
};

export default function Page() {
  return <SignInViewPage />;
}
