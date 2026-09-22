import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function BlogLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='blog'
      fallback={
        <ModuleUnavailable
          title='Blog module is disabled'
          description='Enable the blog module on the modules page to access these routes.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
