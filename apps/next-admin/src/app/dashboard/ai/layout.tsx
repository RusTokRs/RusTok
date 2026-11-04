import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function AiLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='ai'
      fallback={
        <ModuleUnavailable
          title='AI module is disabled'
          description='Enable the AI module on the modules page to manage models and operator tools.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
