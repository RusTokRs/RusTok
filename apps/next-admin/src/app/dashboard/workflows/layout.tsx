import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function WorkflowsLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='workflow'
      fallback={
        <ModuleUnavailable
          title='Workflow module is disabled'
          description='Enable the workflow module on the modules page to manage automated workflows.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
