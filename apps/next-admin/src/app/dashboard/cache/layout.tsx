import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function CacheLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <ModuleGuard
      slug='cache'
      fallback={
        <ModuleUnavailable
          title='Cache module is disabled'
          description='Enable the cache module on the modules page to manage cache tiers.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
