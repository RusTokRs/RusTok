import Link from 'next/link';
import type React from 'react';
import { redirect } from 'next/navigation';

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { PageContainer } from '@/widgets/app-shell';
import {
  IconActivity,
  IconArrowUpRight,
  IconDatabase,
  IconPackage,
  IconReceipt,
  IconUsers
} from '@tabler/icons-react';

type DashboardStats = {
  totalUsers: number;
  totalPosts: number;
  totalOrders: number;
  totalRevenue: number;
  usersChange: number;
  postsChange: number;
  ordersChange: number;
  revenueChange: number;
};

type ActivityItem = {
  id: string;
  type: string;
  description: string;
  timestamp: string;
  user?: {
    id: string;
    name?: string | null;
  } | null;
};

type DashboardStatsResponse = {
  dashboardStats: DashboardStats | null;
};

type RecentActivityResponse = {
  recentActivity: ActivityItem[];
};

type EnabledModulesResponse = {
  enabledModules: string[];
};

const DASHBOARD_STATS_QUERY = `
query DashboardStats {
  dashboardStats {
    totalUsers
    totalPosts
    totalOrders
    totalRevenue
    usersChange
    postsChange
    ordersChange
    revenueChange
  }
}
`;

const RECENT_ACTIVITY_QUERY = `
query RecentActivity($limit: Int!) {
  recentActivity(limit: $limit) {
    id
    type
    description
    timestamp
    user { id name }
  }
}
`;

const ENABLED_MODULES_QUERY = `
query EnabledModules {
  enabledModules
}
`;

async function loadOverviewData(
  token?: string | null,
  tenantSlug?: string | null
) {
  if (!token || !tenantSlug) {
    return {
      stats: null,
      recentActivity: [],
      enabledModules: [],
      error: 'No active admin session'
    };
  }

  try {
    const [statsData, activityData, modulesData] = await Promise.all([
      graphqlRequest<undefined, DashboardStatsResponse>(
        DASHBOARD_STATS_QUERY,
        undefined,
        token,
        tenantSlug
      ),
      graphqlRequest<{ limit: number }, RecentActivityResponse>(
        RECENT_ACTIVITY_QUERY,
        { limit: 8 },
        token,
        tenantSlug
      ),
      graphqlRequest<undefined, EnabledModulesResponse>(
        ENABLED_MODULES_QUERY,
        undefined,
        token,
        tenantSlug
      )
    ]);

    return {
      stats: statsData.dashboardStats,
      recentActivity: activityData.recentActivity,
      enabledModules: modulesData.enabledModules,
      error: null
    };
  } catch (error) {
    return {
      stats: null,
      recentActivity: [],
      enabledModules: [],
      error:
        error instanceof Error ? error.message : 'Failed to load dashboard data'
    };
  }
}

function formatChange(value: number | undefined): string {
  const next = Number.isFinite(value) ? (value ?? 0) : 0;
  return `${next >= 0 ? '+' : ''}${next.toFixed(1)}%`;
}

function formatTimeAgo(value: string): string {
  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) return value;

  const diffMs = Date.now() - timestamp.getTime();
  const diffMinutes = Math.max(0, Math.floor(diffMs / 60_000));
  if (diffMinutes < 1) return 'just now';
  if (diffMinutes < 60) return `${diffMinutes}m ago`;

  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours}h ago`;

  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 30) return `${diffDays}d ago`;

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(timestamp);
}

function StatCard({
  title,
  value,
  change,
  helper,
  icon
}: {
  title: string;
  value: string;
  change: number | undefined;
  helper: string;
  icon: React.ReactNode;
}) {
  return (
    <Card className='from-primary/5 to-card @container/card bg-gradient-to-t shadow-xs'>
      <CardHeader>
        <CardDescription>{title}</CardDescription>
        <CardTitle className='text-2xl font-semibold tabular-nums @[250px]/card:text-3xl'>
          {value}
        </CardTitle>
        <div className='bg-muted text-muted-foreground col-start-2 row-span-2 row-start-1 self-start justify-self-end rounded-lg p-3'>
          {icon}
        </div>
      </CardHeader>
      <CardFooter className='flex-col items-start gap-1.5 text-sm'>
        <div className='line-clamp-1 flex items-center gap-2 font-medium'>
          <Badge variant='outline'>{formatChange(change)}</Badge>
          <span>{helper}</span>
        </div>
      </CardFooter>
    </Card>
  );
}

export default async function DashboardPage() {
  const session = await auth();
  if (!session) redirect('/auth/sign-in');

  const token = session.user?.rustokToken ?? null;
  const tenantSlug = session.user?.tenantSlug ?? null;
  const userName = session.user?.name ?? session.user?.email ?? 'Admin';
  const { stats, recentActivity, enabledModules, error } =
    await loadOverviewData(token, tenantSlug);

  return (
    <PageContainer
      pageTitle={`Welcome back, ${userName}`}
      pageDescription='Live RusTok workspace overview from apps/server.'
    >
      <div className='flex flex-1 flex-col gap-6'>
        {error && (
          <Card className='border-destructive/30 bg-destructive/5'>
            <CardHeader>
              <CardTitle>Dashboard data is unavailable</CardTitle>
              <CardDescription>{error}</CardDescription>
            </CardHeader>
          </Card>
        )}

        <div className='grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4'>
          <StatCard
            title='Total users'
            value={(stats?.totalUsers ?? 0).toLocaleString()}
            change={stats?.usersChange}
            helper='vs previous period'
            icon={<IconUsers className='size-5' />}
          />
          <StatCard
            title='Content nodes'
            value={(stats?.totalPosts ?? 0).toLocaleString()}
            change={stats?.postsChange}
            helper='vs previous period'
            icon={<IconDatabase className='size-5' />}
          />
          <StatCard
            title='Orders'
            value={(stats?.totalOrders ?? 0).toLocaleString()}
            change={stats?.ordersChange}
            helper='vs previous period'
            icon={<IconReceipt className='size-5' />}
          />
          <StatCard
            title='Revenue snapshot'
            value={`$${(stats?.totalRevenue ?? 0).toLocaleString()}`}
            change={stats?.revenueChange}
            helper='from order events'
            icon={<IconActivity className='size-5' />}
          />
        </div>

        <div className='grid grid-cols-1 gap-4 xl:grid-cols-[1.35fr_0.65fr]'>
          <Card>
            <CardHeader>
              <CardTitle>Recent activity</CardTitle>
              <CardDescription>
                Tenant-scoped events surfaced by the RusTok backend.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {recentActivity.length === 0 ? (
                <div className='text-muted-foreground rounded-lg border border-dashed p-6 text-sm'>
                  No recent activity yet.
                </div>
              ) : (
                <div className='divide-y'>
                  {recentActivity.map((item) => (
                    <div
                      key={item.id}
                      className='flex items-start justify-between gap-4 py-3 first:pt-0 last:pb-0'
                    >
                      <div className='min-w-0'>
                        <div className='flex items-center gap-2'>
                          <Badge variant='secondary'>{item.type}</Badge>
                          <span className='truncate font-medium'>
                            {item.description}
                          </span>
                        </div>
                        <p className='text-muted-foreground mt-1 text-sm'>
                          by {item.user?.name ?? 'System'}
                        </p>
                      </div>
                      <span className='text-muted-foreground shrink-0 text-xs'>
                        {formatTimeAgo(item.timestamp)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <div className='grid gap-4'>
            <Card>
              <CardHeader>
                <CardTitle>Enabled modules</CardTitle>
                <CardDescription>
                  Modules active for this workspace runtime.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <div className='flex items-end justify-between gap-4'>
                  <div>
                    <div className='text-3xl font-semibold'>
                      {enabledModules.length}
                    </div>
                    <p className='text-muted-foreground text-sm'>
                      modules currently enabled
                    </p>
                  </div>
                  <IconPackage className='text-muted-foreground size-8' />
                </div>
                <div className='mt-4 flex flex-wrap gap-2'>
                  {enabledModules.slice(0, 10).map((slug) => (
                    <Badge key={slug} variant='outline'>
                      {slug}
                    </Badge>
                  ))}
                  {enabledModules.length > 10 && (
                    <Badge variant='secondary'>
                      +{enabledModules.length - 10}
                    </Badge>
                  )}
                </div>
              </CardContent>
              <CardFooter>
                <Button asChild variant='outline' className='w-full'>
                  <Link href='/dashboard/modules'>
                    Open modules
                    <IconArrowUpRight className='ml-2 size-4' />
                  </Link>
                </Button>
              </CardFooter>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Quick actions</CardTitle>
                <CardDescription>
                  Operator surfaces that already use RusTok APIs.
                </CardDescription>
              </CardHeader>
              <CardContent className='grid gap-2'>
                <Button asChild variant='secondary'>
                  <Link href='/dashboard/users'>Manage users</Link>
                </Button>
                <Button asChild variant='secondary'>
                  <Link href='/dashboard/apps'>App connections</Link>
                </Button>
                <Button asChild variant='secondary'>
                  <Link href='/dashboard/search'>Search diagnostics</Link>
                </Button>
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </PageContainer>
  );
}
