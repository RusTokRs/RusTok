import { notFound } from 'next/navigation';
import { auth } from '@/auth';
import { fetchEnabledModules } from '@/shared/api/modules';
import { ModulePolicyError } from './module-policy-error';

export async function ModuleGuard({
  slug,
  children,
  fallback
}: {
  slug: string;
  children: React.ReactNode;
  fallback?: React.ReactNode;
}) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  if (!token || !tenantSlug) return <ModulePolicyError />;

  let enabledModules: string[];
  try {
    enabledModules = await fetchEnabledModules({ token, tenantSlug });
  } catch (error) {
    console.error('Failed to fetch module availability', error);
    return <ModulePolicyError />;
  }

  if (!enabledModules.includes(slug)) {
    if (fallback) {
      return <>{fallback}</>;
    }
    notFound();
  }

  return <>{children}</>;
}
