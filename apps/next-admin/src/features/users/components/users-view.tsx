'use client';

import * as React from 'react';
import { graphqlRequest } from '@/shared/api/graphql';
import { useSession } from 'next-auth/react';
import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { toast } from 'sonner';
import { User, UsersResponse } from '@/entities/user';
import { UsersTable } from './users-table';

const USERS_QUERY = `
query Users($pagination: PaginationInput, $filter: UsersFilter, $search: String) {
  users(pagination: $pagination, filter: $filter, search: $search) {
    edges { node { id email name role status createdAt tenantName } }
    pageInfo { totalCount }
  }
}`;

import { useTranslations } from '@rustok/next-fluent';

export default function UsersView() {
  const t = useTranslations('users');
  const { data: session } = useSession();
  const token = session?.user?.rustokToken;
  const tenantSlug = session?.user?.tenantSlug;
  const searchParams = useSearchParams();

  const [users, setUsers] = useState<User[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  const page = Math.max(1, Number(searchParams.get('page')) || 1);
  const perPage = Math.max(1, Number(searchParams.get('perPage')) || 12);
  const search = searchParams.get('email') || searchParams.get('search') || '';
  const roleFilter = searchParams.get('role') || '';
  const statusFilter = searchParams.get('status') || '';

  const fetchUsers = async () => {
    if (!token) return;
    setIsLoading(true);
    try {
      const after =
        page > 1 ? btoa(String((page - 1) * perPage - 1)) : undefined;
      const data = await graphqlRequest<object, UsersResponse>(
        USERS_QUERY,
        {
          pagination: { first: perPage, after },
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
      toast.error(t('toast.load.error'));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchUsers();
  }, [token, tenantSlug, page, perPage, search, roleFilter, statusFilter]);

  return (
    <div className='flex flex-1 flex-col space-y-4'>
      <UsersTable data={users} totalItems={totalCount} />
    </div>
  );
}
