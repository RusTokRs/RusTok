import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function MediaLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='media'
      fallback={
        <ModuleUnavailable
          title='Media module is disabled'
          description='Enable the media module on the modules page to access media asset management.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
