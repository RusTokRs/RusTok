import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function EventsLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='events'
      fallback={
        <ModuleUnavailable
          title='Events module is disabled'
          description='Enable the events module on the modules page to manage outbox and delivery.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
