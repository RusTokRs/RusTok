import { Metadata } from 'next';
import SignUpViewPage from '@/features/auth/components/sign-up-view';

export const metadata: Metadata = {
  title: 'Sign Up | RusTok Admin',
  description: 'Create a new RusTok Admin account.'
};

export default function Page() {
  return <SignUpViewPage />;
}
