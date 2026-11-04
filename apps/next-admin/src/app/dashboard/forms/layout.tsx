import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function FormsLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='forms'
      fallback={
        <ModuleUnavailable
          title='Forms module is disabled'
          description='Enable the forms module on the modules page to view form submissions.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
