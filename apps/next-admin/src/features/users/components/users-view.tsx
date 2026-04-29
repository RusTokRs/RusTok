'use client';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { graphqlRequest } from '@/shared/api/graphql';
import { useSession } from 'next-auth/react';
import { useEffect, useState } from 'react';
import Link from 'next/link';
import { toast } from 'sonner';

import { User, UsersResponse } from '@/entities/user';

const USERS_QUERY = `
query Users($pagination: PaginationInput, $filter: UsersFilter, $search: String) {
  users(pagination: $pagination, filter: $filter, search: $search) {
    edges { node { id email name role status createdAt tenantName } }
    pageInfo { totalCount }
  }
}`;

const PAGE_SIZE = 12;

export default function UsersView() {
  const { data: session } = useSession();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;

  const [users, setUsers] = useState<User[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState('');
  const [roleFilter, setRoleFilter] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const fetchUsers = async () => {
    if (!token) return;
    setIsLoading(true);
    try {
      const after =
        page > 1 ? btoa(String((page - 1) * PAGE_SIZE - 1)) : undefined;
      const data = await graphqlRequest<object, UsersResponse>(
        USERS_QUERY,
        {
          pagination: { first: PAGE_SIZE, after },
          filter: {
            role: roleFilter ? roleFilter.toUpperCase() : undefined,
            status: statusFilter ? statusFilter.toUpperCase() : undefined
          },
          search: search || undefined
        },
        token,
        tenantSlug
      );
      setUsers(data.users.edges.map((e) => e.node));
      setTotalCount(data.users.pageInfo.totalCount);
    } catch {
      toast.error('Failed to load users');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchUsers();
  }, [token, page]);

  const totalPages = Math.ceil(totalCount / PAGE_SIZE);

  return (
    <div className='space-y-4'>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          setPage(1);
          fetchUsers();
        }}
        className='grid gap-3 md:grid-cols-3'
      >
        <Input
          placeholder='Search by email or name...'
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Input
          placeholder='Filter by role (e.g. ADMIN)'
          value={roleFilter}
          onChange={(e) => setRoleFilter(e.target.value)}
        />
        <div className='flex gap-2'>
          <Input
            placeholder='Filter by status (e.g. ACTIVE)'
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
          />
          <Button type='submit' variant='outline'>
            Search
          </Button>
        </div>
      </form>
      <p className='text-muted-foreground text-xs'>Total: {totalCount} users</p>
      {isLoading ? (
        <p className='text-muted-foreground text-sm'>Loading...</p>
      ) : (
        <div className='rounded-md border'>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Email</TableHead>
                <TableHead>Name</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Created</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {users.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={5}
                    className='text-muted-foreground text-center text-sm'
                  >
                    No users found
                  </TableCell>
                </TableRow>
              ) : (
                users.map((user) => (
                  <TableRow key={user.id}>
                    <TableCell>
                      <Link
                        href={`/dashboard/users/${user.id}`}
                        className='text-primary hover:underline'
                      >
                        {user.email}
                      </Link>
                    </TableCell>
                    <TableCell>{user.name || '—'}</TableCell>
                    <TableCell>{user.role}</TableCell>
                    <TableCell>
                      <Badge
                        variant={
                          user.status === 'ACTIVE' ? 'default' : 'secondary'
                        }
                      >
                        {user.status}
                      </Badge>
                    </TableCell>
                    <TableCell className='text-muted-foreground text-xs'>
                      {user.createdAt
                        ? new Date(user.createdAt).toLocaleDateString()
                        : '—'}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      )}
      {totalPages > 1 && (
        <div className='flex items-center gap-3'>
          <Button
            variant='outline'
            size='sm'
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page <= 1}
          >
            Previous
          </Button>
          <span className='text-muted-foreground text-xs'>
            Page {page} of {totalPages}
          </span>
          <Button
            variant='outline'
            size='sm'
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page >= totalPages}
          >
            Next
          </Button>
        </div>
      )}
    </div>
  );
}
