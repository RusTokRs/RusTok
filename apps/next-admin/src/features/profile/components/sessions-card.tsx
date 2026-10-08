'use client';

import * as React from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  IconDeviceLaptop,
  IconDeviceMobile,
  IconRefresh,
  IconShieldLock,
  IconTrash
} from '@tabler/icons-react';
import { toast } from 'sonner';

export interface SessionRecord {
  id: string;
  ipAddress: string | null;
  userAgent: string | null;
  lastUsedAt: string | null;
  expiresAt: string;
  createdAt: string;
  current: boolean;
}

interface SessionsResponse {
  sessions: {
    sessions: SessionRecord[];
  };
}

const SESSIONS_QUERY = `
query Sessions($limit: Int) {
  sessions(limit: $limit) {
    sessions {
      id
      ipAddress
      userAgent
      lastUsedAt
      expiresAt
      createdAt
      current
    }
  }
}
`;

const REVOKE_SESSION_MUTATION = `
mutation RevokeSession($sessionId: String!) {
  revokeSession(sessionId: $sessionId) {
    success
    revoked
  }
}
`;

const REVOKE_ALL_SESSIONS_MUTATION = `
mutation RevokeAllSessions {
  revokeAllSessions {
    success
    revokedCount
  }
}
`;

interface SessionsCardProps {
  token?: string | null;
  tenantSlug?: string | null;
}

function parseUserAgent(ua: string | null): {
  device: string;
  isMobile: boolean;
} {
  if (!ua) return { device: 'Unknown Device', isMobile: false };
  const isMobile = /mobile|android|iphone|ipad/i.test(ua);
  let browser = 'Browser';
  if (/chrome|crios/i.test(ua)) browser = 'Chrome';
  else if (/firefox|fxios/i.test(ua)) browser = 'Firefox';
  else if (/safari/i.test(ua)) browser = 'Safari';
  else if (/edg/i.test(ua)) browser = 'Edge';

  let os = 'Unknown OS';
  if (/windows/i.test(ua)) os = 'Windows';
  else if (/macintosh|mac os/i.test(ua)) os = 'macOS';
  else if (/linux/i.test(ua)) os = 'Linux';
  else if (/android/i.test(ua)) os = 'Android';
  else if (/iphone|ipad/i.test(ua)) os = 'iOS';

  return { device: `${browser} on ${os}`, isMobile };
}

export function SessionsCard({ token, tenantSlug }: SessionsCardProps) {
  const [sessions, setSessions] = React.useState<SessionRecord[]>([]);
  const [isLoading, setIsLoading] = React.useState(true);
  const [revokingId, setRevokingId] = React.useState<string | null>(null);
  const [isRevokingAll, setIsRevokingAll] = React.useState(false);

  const fetchSessions = React.useCallback(async () => {
    if (!token) return;
    setIsLoading(true);
    try {
      const data = await graphqlRequest<{ limit?: number }, SessionsResponse>(
        SESSIONS_QUERY,
        { limit: 50 },
        token,
        tenantSlug
      );
      setSessions(data.sessions.sessions || []);
    } catch {
      toast.error('Failed to load active sessions');
    } finally {
      setIsLoading(false);
    }
  }, [token, tenantSlug]);

  React.useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  const handleRevoke = async (sessionId: string) => {
    if (!token) return;
    setRevokingId(sessionId);
    try {
      const data = await graphqlRequest<
        { sessionId: string },
        { revokeSession: { success: boolean; revoked: boolean } }
      >(REVOKE_SESSION_MUTATION, { sessionId }, token, tenantSlug);

      if (data.revokeSession.success) {
        toast.success('Session revoked successfully');
        setSessions((prev) => prev.filter((s) => s.id !== sessionId));
      } else {
        toast.error('Could not revoke session');
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      toast.error(`Failed to revoke session: ${message}`);
    } finally {
      setRevokingId(null);
    }
  };

  const handleRevokeAllOther = async () => {
    if (!token) return;
    setIsRevokingAll(true);
    try {
      const data = await graphqlRequest<
        Record<string, never>,
        { revokeAllSessions: { success: boolean; revokedCount: number } }
      >(REVOKE_ALL_SESSIONS_MUTATION, {}, token, tenantSlug);

      if (data.revokeAllSessions.success) {
        toast.success(
          `Revoked ${data.revokeAllSessions.revokedCount} other sessions`
        );
        fetchSessions();
      } else {
        toast.error('Failed to revoke all sessions');
      }
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      toast.error(`Revoke failed: ${message}`);
    } finally {
      setIsRevokingAll(false);
    }
  };

  const otherSessionsCount = sessions.filter((s) => !s.current).length;

  return (
    <Card className='col-span-full'>
      <CardHeader className='flex flex-col gap-2 pb-3 sm:flex-row sm:items-center sm:justify-between'>
        <div>
          <CardTitle className='flex items-center gap-2 text-base font-semibold'>
            <IconShieldLock className='text-primary h-4 w-4' />
            Active Sessions ({sessions.length})
          </CardTitle>
          <CardDescription>
            Devices and locations currently authenticated with your account.
          </CardDescription>
        </div>
        <div className='flex items-center gap-2'>
          <Button
            variant='outline'
            size='sm'
            onClick={fetchSessions}
            disabled={isLoading}
            className='h-8 px-2.5 text-xs'
          >
            <IconRefresh
              className={`mr-1 h-3.5 w-3.5 ${isLoading ? 'animate-spin' : ''}`}
            />
            Refresh
          </Button>
          {otherSessionsCount > 0 && (
            <Button
              variant='destructive'
              size='sm'
              onClick={handleRevokeAllOther}
              disabled={isRevokingAll}
              className='h-8 px-2.5 text-xs'
            >
              <IconTrash className='mr-1 h-3.5 w-3.5' />
              {isRevokingAll ? 'Revoking...' : 'Sign Out All Other Sessions'}
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent>
        {isLoading && sessions.length === 0 ? (
          <div className='text-muted-foreground animate-pulse py-8 text-center text-sm'>
            Loading active sessions...
          </div>
        ) : sessions.length === 0 ? (
          <div className='text-muted-foreground py-8 text-center text-sm'>
            No active sessions found.
          </div>
        ) : (
          <div className='bg-card divide-y overflow-hidden rounded-lg border'>
            {sessions.map((session) => {
              const { device, isMobile } = parseUserAgent(session.userAgent);
              const isCurrent = session.current;
              const isRevoking = revokingId === session.id;

              return (
                <div
                  key={session.id}
                  className='hover:bg-muted/40 flex flex-col justify-between gap-3 p-3.5 transition-colors sm:flex-row sm:items-center'
                >
                  <div className='flex min-w-0 items-start gap-3'>
                    <div className='bg-muted text-muted-foreground mt-0.5 flex-shrink-0 rounded-md p-2 sm:mt-0'>
                      {isMobile ? (
                        <IconDeviceMobile className='h-4 w-4' />
                      ) : (
                        <IconDeviceLaptop className='h-4 w-4' />
                      )}
                    </div>
                    <div className='min-w-0 space-y-1'>
                      <div className='flex flex-wrap items-center gap-2'>
                        <span className='text-foreground truncate text-sm font-semibold'>
                          {device}
                        </span>
                        {isCurrent && (
                          <Badge
                            variant='outline'
                            className='border-emerald-500/30 bg-emerald-50 py-0 text-[10px] text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300'
                          >
                            Current Session
                          </Badge>
                        )}
                      </div>
                      <div className='text-muted-foreground flex flex-wrap items-center gap-2 font-mono text-xs'>
                        <span>IP: {session.ipAddress || 'Unknown IP'}</span>
                        <span>•</span>
                        <span>
                          Signed in:{' '}
                          {new Date(session.createdAt).toLocaleDateString()}
                        </span>
                        {session.lastUsedAt && (
                          <>
                            <span>•</span>
                            <span>
                              Last used:{' '}
                              {new Date(
                                session.lastUsedAt
                              ).toLocaleTimeString()}
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                  </div>

                  <div>
                    {!isCurrent ? (
                      <Button
                        variant='ghost'
                        size='sm'
                        onClick={() => handleRevoke(session.id)}
                        disabled={isRevoking}
                        className='text-destructive hover:bg-destructive/10 h-8 px-2.5 text-xs'
                      >
                        {isRevoking ? 'Revoking...' : 'Revoke Session'}
                      </Button>
                    ) : (
                      <span className='text-muted-foreground px-2 text-xs italic'>
                        Active now
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
