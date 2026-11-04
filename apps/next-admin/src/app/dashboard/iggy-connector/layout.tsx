import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function IggyConnectorLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='iggy-connector'
      fallback={
        <ModuleUnavailable
          title='Iggy Connector module is disabled'
          description='Enable the Iggy connector module on the modules page to manage streaming pipelines.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
