import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function CommerceLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='commerce'
      fallback={
        <ModuleUnavailable
          title='Commerce module is disabled'
          description='Enable the commerce module on the modules page to access shipping profiles, cart promotions, return decisions, and order-change operator routes.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
