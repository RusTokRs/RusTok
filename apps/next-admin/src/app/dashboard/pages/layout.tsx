import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function PagesLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='pages'
      fallback={
        <ModuleUnavailable
          title='Pages module is disabled'
          description='Enable the pages module on the modules page to access page builder and content management.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
