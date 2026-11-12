import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function SeoLayout({ children }: { children: React.ReactNode }) {
  return (
    <ModuleGuard
      slug='seo'
      fallback={
        <ModuleUnavailable
          title='SEO module is disabled'
          description='Enable the SEO module on the modules page to manage indexes and metadata.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
