import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function PricingLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='pricing'
      fallback={
        <ModuleUnavailable
          title='Pricing module is disabled'
          description='Enable the pricing module on the modules page to access pricing and price list routes.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
