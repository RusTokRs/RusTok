import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function EmailLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='email'
      fallback={
        <ModuleUnavailable
          title='Email module is disabled'
          description='Enable the email module on the modules page to configure delivery settings.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
