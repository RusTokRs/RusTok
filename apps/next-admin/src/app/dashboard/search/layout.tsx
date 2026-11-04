import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function SearchLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='search'
      fallback={
        <ModuleUnavailable
          title='Search module is disabled'
          description='Enable the search module on the modules page to access search control plane and diagnostics.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
