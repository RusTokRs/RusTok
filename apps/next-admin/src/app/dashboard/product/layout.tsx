import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function ProductLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='product'
      fallback={
        <ModuleUnavailable
          title='Product module is disabled'
          description='Enable the product module on the modules page to access catalog and inventory routes.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
