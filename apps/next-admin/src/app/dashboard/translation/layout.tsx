import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function TranslationLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='translation'
      fallback={
        <ModuleUnavailable
          title='Translation module is disabled'
          description='Enable the Translation module on the modules page to access these routes.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
