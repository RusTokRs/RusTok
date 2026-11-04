import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function BrandsLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='brand'
      fallback={
        <ModuleUnavailable
          title='Brand module is disabled'
          description='Enable the brand module on the modules page to manage brands.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
