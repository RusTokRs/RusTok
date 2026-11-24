'use client';

import { useState, useMemo } from 'react';
import { PageContainer } from '@/widgets/app-shell';

interface BillingRecord {
  id: string;
  title: string;
  status: 'active' | 'draft' | 'archived';
  createdAt: string;
  canEdit: boolean;
  canDelete: boolean;
}

const INITIAL_RECORDS: BillingRecord[] = [
  {
    id: 'rec-1',
    title: 'Default B2B Billing & Settlements Workspace',
    status: 'active',
    createdAt: '2026-01-01',
    canEdit: true,
    canDelete: true,
  },
  {
    id: 'rec-2',
    title: 'Secondary B2B Billing & Settlements Profile',
    status: 'draft',
    createdAt: '2026-01-02',
    canEdit: true,
    canDelete: true,
  },
];

export default function BillingPage() {
  const [searchQuery, setSearchQuery] = useState('');
  const [records] = useState<BillingRecord[]>(INITIAL_RECORDS);

  const filteredRecords = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return records;
    return records.filter(
      (r) =>
        r.title.toLowerCase().includes(q) ||
        r.status.toLowerCase().includes(q)
    );
  }, [searchQuery, records]);

  return (
    <PageContainer
      scrollable
      pageTitle="B2B Billing & Settlements"
      pageDescription="B2B Billing & Settlements capability module for RusToK"
    >
      <div className="space-y-6">
        <div className="flex flex-col sm:flex-row gap-4 items-start sm:items-center justify-between">
          <div className="relative w-72">
            <input
              type="text"
              placeholder="Search records..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full text-xs rounded-lg border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary transition-all"
            />
          </div>
          <button
            type="button"
            className="px-4 py-2 text-sm font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors shadow-sm"
            onClick={() => {
              console.info('create new billing record initiated');
            }}
          >
            Create New
          </button>
        </div>

        <div className="text-xs text-muted-foreground">
          Search indexed globally via <code className="px-1 py-0.5 bg-muted rounded">rustok-search</code>
        </div>

        <div className="rounded-xl border border-border overflow-hidden bg-card shadow-sm">
          <table className="w-full text-sm text-left">
            <thead className="bg-muted/50 text-xs uppercase text-muted-foreground border-b border-border">
              <tr>
                <th className="p-4 w-12 text-center">
                  <input type="checkbox" className="rounded border-border" />
                </th>
                <th className="p-4 font-medium">Title</th>
                <th className="p-4 font-medium">Status</th>
                <th className="p-4 font-medium">Created At</th>
                <th className="p-4 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {filteredRecords.length === 0 ? (
                <tr>
                  <td colSpan={5} className="p-8 text-center text-muted-foreground text-sm">
                    No records found
                  </td>
                </tr>
              ) : (
                filteredRecords.map((row) => (
                  <tr key={row.id} className="hover:bg-muted/30 transition-colors">
                    <td className="p-4 w-12 text-center">
                      <input type="checkbox" className="rounded border-border" />
                    </td>
                    <td className="p-4 font-medium text-foreground">{row.title}</td>
                    <td className="p-4">
                      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-secondary text-secondary-foreground">
                        {row.status}
                      </span>
                    </td>
                    <td className="p-4 text-xs text-muted-foreground">{row.createdAt}</td>
                    <td className="p-4">
                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          className="text-xs text-primary hover:underline font-medium disabled:opacity-50"
                          disabled={!row.canEdit}
                          onClick={() => console.info('edit action triggered', row.id)}
                        >
                          Edit
                        </button>
                        <button
                          type="button"
                          className="text-xs text-destructive hover:underline font-medium disabled:opacity-50"
                          disabled={!row.canDelete}
                          onClick={() => console.info('delete action triggered', row.id)}
                        >
                          Delete
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </PageContainer>
  );
}
