import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function ForumLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='forum'
      fallback={
        <ModuleUnavailable
          title='Forum module is disabled'
          description='Enable the forum module on the modules page to access these routes.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
