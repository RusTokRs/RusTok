'use client';

import { useEffect, useState } from 'react';
import { useSession } from 'next-auth/react';
import { toast } from 'sonner';

import { OAuthApp } from '@/entities/oauth-app';
import {
  CreateAppDialog,
  EditAppDialog,
  RevokeAppDialog,
  RotateSecretDialog
} from '@/features/oauth-apps';
import { listOAuthApps } from '@/shared/api/oauth-apps';
import { OAuthAppsTable } from '@/widgets/oauth-apps-table';
import { Button } from '@/shared/ui/shadcn/button';

export default function OAuthAppsPage() {
  const { data: session } = useSession();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;

  const [apps, setApps] = useState<OAuthApp[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const [createOpen, setCreateOpen] = useState(false);
  const [editApp, setEditApp] = useState<OAuthApp | null>(null);
  const [rotateApp, setRotateApp] = useState<OAuthApp | null>(null);
  const [revokeApp, setRevokeApp] = useState<OAuthApp | null>(null);

  useEffect(() => {
    if (!token) {
      return;
    }

    let cancelled = false;

    const loadApps = async () => {
      setIsLoading(true);
      try {
        const nextApps = await listOAuthApps(token, tenantSlug);
        if (!cancelled) {
          setApps(nextApps);
        }
      } catch (error) {
        if (!cancelled) {
          toast.error(
            error instanceof Error
              ? error.message
              : 'Failed to load app connections'
          );
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };

    void loadApps();

    return () => {
      cancelled = true;
    };
  }, [tenantSlug, token]);

  const upsertApp = (nextApp: OAuthApp) => {
    setApps((current) => {
      const existing = current.find((app) => app.id === nextApp.id);
      if (!existing) {
        return [nextApp, ...current];
      }

      return current.map((app) => (app.id === nextApp.id ? nextApp : app));
    });
  };

  const handleCreateSuccess = (app: OAuthApp) => {
    upsertApp(app);
  };

  const handleUpdateSuccess = (app: OAuthApp) => {
    upsertApp(app);
    setEditApp(null);
  };

  const handleRotateSuccess = (app: OAuthApp) => {
    upsertApp(app);
  };

  const handleRevokeSuccess = (appId: string) => {
    setApps((current) => current.filter((app) => app.id !== appId));
    setRevokeApp(null);
  };

  const canCreate = Boolean(token && tenantSlug);

  return (
    <div className='space-y-6'>
      <div className='flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between'>
        <div>
          <h2 className='text-2xl font-bold tracking-tight'>
            OAuth App Connections
          </h2>
          <p className='text-muted-foreground'>
            Manage manual integrations, review manifest-managed frontends, and
            rotate client credentials.
          </p>
        </div>
        <Button onClick={() => setCreateOpen(true)} disabled={!canCreate}>
          Create New App
        </Button>
      </div>

      <OAuthAppsTable
        apps={apps}
        isLoading={isLoading}
        onEditApp={setEditApp}
        onRotateSecret={setRotateApp}
        onRevokeApp={setRevokeApp}
      />

      <CreateAppDialog
        token={token}
        tenantSlug={tenantSlug}
        open={createOpen}
        onOpenChange={setCreateOpen}
        onSuccess={handleCreateSuccess}
      />

      <EditAppDialog
        token={token}
        tenantSlug={tenantSlug}
        app={editApp}
        open={!!editApp}
        onOpenChange={(open) => !open && setEditApp(null)}
        onSuccess={handleUpdateSuccess}
      />

      <RotateSecretDialog
        token={token}
        tenantSlug={tenantSlug}
        app={rotateApp}
        open={!!rotateApp}
        onOpenChange={(open) => !open && setRotateApp(null)}
        onSuccess={handleRotateSuccess}
      />

      <RevokeAppDialog
        token={token}
        tenantSlug={tenantSlug}
        app={revokeApp}
        open={!!revokeApp}
        onOpenChange={(open) => !open && setRevokeApp(null)}
        onSuccess={handleRevokeSuccess}
      />
    </div>
  );
}
