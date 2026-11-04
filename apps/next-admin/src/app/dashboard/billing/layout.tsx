import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function BillingLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='billing'
      fallback={
        <ModuleUnavailable
          title='Billing module is disabled'
          description='Enable the billing module on the modules page to manage subscriptions and invoices.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
