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

function parseUserAgent(ua: string | null): { device: string; isMobile: boolean } {
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
      <CardHeader className='flex flex-col sm:flex-row sm:items-center sm:justify-between pb-3 gap-2'>
        <div>
          <CardTitle className='text-base font-semibold flex items-center gap-2'>
            <IconShieldLock className='h-4 w-4 text-primary' />
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
              className={`h-3.5 w-3.5 mr-1 ${isLoading ? 'animate-spin' : ''}`}
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
              <IconTrash className='h-3.5 w-3.5 mr-1' />
              {isRevokingAll ? 'Revoking...' : 'Sign Out All Other Sessions'}
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent>
        {isLoading && sessions.length === 0 ? (
          <div className='py-8 text-center text-sm text-muted-foreground animate-pulse'>
            Loading active sessions...
          </div>
        ) : sessions.length === 0 ? (
          <div className='py-8 text-center text-sm text-muted-foreground'>
            No active sessions found.
          </div>
        ) : (
          <div className='divide-y rounded-lg border bg-card overflow-hidden'>
            {sessions.map((session) => {
              const { device, isMobile } = parseUserAgent(session.userAgent);
              const isCurrent = session.current;
              const isRevoking = revokingId === session.id;

              return (
                <div
                  key={session.id}
                  className='flex flex-col sm:flex-row sm:items-center justify-between p-3.5 gap-3 hover:bg-muted/40 transition-colors'
                >
                  <div className='flex items-start gap-3 min-w-0'>
                    <div className='p-2 rounded-md bg-muted text-muted-foreground flex-shrink-0 mt-0.5 sm:mt-0'>
                      {isMobile ? (
                        <IconDeviceMobile className='h-4 w-4' />
                      ) : (
                        <IconDeviceLaptop className='h-4 w-4' />
                      )}
                    </div>
                    <div className='space-y-1 min-w-0'>
                      <div className='flex items-center gap-2 flex-wrap'>
                        <span className='text-sm font-semibold text-foreground truncate'>
                          {device}
                        </span>
                        {isCurrent && (
                          <Badge
                            variant='outline'
                            className='border-emerald-500/30 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300 text-[10px] py-0'
                          >
                            Current Session
                          </Badge>
                        )}
                      </div>
                      <div className='flex items-center gap-2 text-xs text-muted-foreground font-mono flex-wrap'>
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
                              {new Date(session.lastUsedAt).toLocaleTimeString()}
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
                        className='text-destructive hover:bg-destructive/10 text-xs h-8 px-2.5'
                      >
                        {isRevoking ? 'Revoking...' : 'Revoke Session'}
                      </Button>
                    ) : (
                      <span className='text-xs text-muted-foreground italic px-2'>
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
