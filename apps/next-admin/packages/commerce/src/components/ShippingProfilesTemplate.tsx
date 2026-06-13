'use client';

import React from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { PageContainer } from '@/widgets/app-shell';
import {
  listShippingProfiles,
  createShippingProfile,
  updateShippingProfile,
  deactivateShippingProfile,
  reactivateShippingProfile,
  GqlOpts
} from '../api';
import type { ShippingProfile, ShippingProfileTranslation } from '../types';

export function ShippingProfilesTemplate({ opts }: { opts: GqlOpts }) {
  const [profiles, setProfiles] = React.useState<ShippingProfile[]>([]);
  const [total, setTotal] = React.useState(0);
  const [page, setPage] = React.useState(1);
  const [hasNext, setHasNext] = React.useState(false);
  const [search, setSearch] = React.useState('');
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [feedback, setFeedback] = React.useState<string | null>(null);

  // Form states
  const [editingProfile, setEditingProfile] = React.useState<ShippingProfile | null>(null);
  const [isFormOpen, setIsFormOpen] = React.useState(false);
  const [slug, setSlug] = React.useState('');
  const [nameRu, setNameRu] = React.useState('');
  const [descRu, setDescRu] = React.useState('');
  const [nameEn, setNameEn] = React.useState('');
  const [descEn, setDescEn] = React.useState('');
  const [metadataStr, setMetadataStr] = React.useState('{}');

  const loadData = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listShippingProfiles(opts, {
        page,
        perPage: 10,
        search: search.trim() || undefined
      });
      setProfiles(result.items);
      setTotal(result.total);
      setHasNext(result.hasNext);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load shipping profiles.');
    } finally {
      setLoading(false);
    }
  }, [opts, page, search]);

  React.useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleToggleActive = async (profile: ShippingProfile) => {
    setError(null);
    setFeedback(null);
    try {
      if (profile.active) {
        await deactivateShippingProfile(opts, profile.id);
        setFeedback(`Deactivated shipping profile: ${profile.slug}`);
      } else {
        await reactivateShippingProfile(opts, profile.id);
        setFeedback(`Reactivated shipping profile: ${profile.slug}`);
      }
      void loadData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update active state.');
    }
  };

  const handleEditClick = (profile: ShippingProfile) => {
    setEditingProfile(profile);
    setSlug(profile.slug);
    
    const ruTrans = profile.translations.find((t) => t.locale === 'ru');
    const enTrans = profile.translations.find((t) => t.locale === 'en');
    setNameRu(ruTrans?.name || '');
    setDescRu(ruTrans?.description || '');
    setNameEn(enTrans?.name || '');
    setDescEn(enTrans?.description || '');
    
    setMetadataStr(profile.metadata || '{}');
    setIsFormOpen(true);
  };

  const handleCreateClick = () => {
    setEditingProfile(null);
    setSlug('');
    setNameRu('');
    setDescRu('');
    setNameEn('');
    setDescEn('');
    setMetadataStr('{}');
    setIsFormOpen(true);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setFeedback(null);

    const translations: ShippingProfileTranslation[] = [];
    if (nameRu.trim()) {
      translations.push({ locale: 'ru', name: nameRu.trim(), description: descRu.trim() || null });
    }
    if (nameEn.trim()) {
      translations.push({ locale: 'en', name: nameEn.trim(), description: descEn.trim() || null });
    }

    if (translations.length === 0) {
      setError('Please add at least one localized profile name.');
      return;
    }

    try {
      if (editingProfile) {
        await updateShippingProfile(opts, editingProfile.id, {
          slug: slug.trim(),
          translations,
          metadata: metadataStr
        });
        setFeedback(`Updated shipping profile: ${slug}`);
      } else {
        await createShippingProfile(opts, {
          slug: slug.trim(),
          translations,
          metadata: metadataStr
        });
        setFeedback(`Created shipping profile: ${slug}`);
      }
      setIsFormOpen(false);
      void loadData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save shipping profile.');
    }
  };

  return (
    <PageContainer
      pageTitle="Shipping Profiles"
      pageDescription="Configure shipping methods, rates, and profiles globally."
      pageHeaderAction={
        <Button onClick={handleCreateClick} size="sm">
          Create Profile
        </Button>
      }
    >
      <div className="space-y-6">
        {feedback && (
          <div className="rounded-lg border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
            {feedback}
          </div>
        )}
        {error && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {error}
          </div>
        )}

        {isFormOpen && (
          <Card className="border-primary/20 bg-primary/5">
            <CardHeader>
              <CardTitle className="text-base">
                {editingProfile ? `Edit Profile: ${editingProfile.slug}` : 'Create New Shipping Profile'}
              </CardTitle>
              <CardDescription>
                Define a slug, metadata and translations for different locales.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={handleSubmit} className="space-y-4">
                <div className="grid gap-4 md:grid-cols-2">
                  <div className="space-y-2">
                    <label className="text-xs font-semibold">Slug (Unique identifier)</label>
                    <Input
                      required
                      placeholder="e.g. express-delivery"
                      value={slug}
                      onChange={(e) => setSlug(e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-xs font-semibold">Metadata JSON</label>
                    <Input
                      placeholder="{}"
                      value={metadataStr}
                      onChange={(e) => setMetadataStr(e.target.value)}
                    />
                  </div>
                </div>

                <div className="border-t pt-4">
                  <h4 className="mb-2 text-sm font-semibold">Translations</h4>
                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="space-y-3 rounded-lg border bg-background p-3">
                      <h5 className="text-xs font-bold text-muted-foreground">Russian (RU)</h5>
                      <div className="space-y-2">
                        <label className="text-[10px] uppercase font-bold text-muted-foreground">Name</label>
                        <Input
                          placeholder="Название профиля"
                          value={nameRu}
                          onChange={(e) => setNameRu(e.target.value)}
                        />
                      </div>
                      <div className="space-y-2">
                        <label className="text-[10px] uppercase font-bold text-muted-foreground">Description</label>
                        <Input
                          placeholder="Описание"
                          value={descRu}
                          onChange={(e) => setDescRu(e.target.value)}
                        />
                      </div>
                    </div>

                    <div className="space-y-3 rounded-lg border bg-background p-3">
                      <h5 className="text-xs font-bold text-muted-foreground">English (EN)</h5>
                      <div className="space-y-2">
                        <label className="text-[10px] uppercase font-bold text-muted-foreground">Name</label>
                        <Input
                          placeholder="Profile Name"
                          value={nameEn}
                          onChange={(e) => setNameEn(e.target.value)}
                        />
                      </div>
                      <div className="space-y-2">
                        <label className="text-[10px] uppercase font-bold text-muted-foreground">Description</label>
                        <Input
                          placeholder="Description"
                          value={descEn}
                          onChange={(e) => setDescEn(e.target.value)}
                        />
                      </div>
                    </div>
                  </div>
                </div>

                <div className="flex gap-2">
                  <Button type="submit">Save Profile</Button>
                  <Button type="button" variant="outline" onClick={() => setIsFormOpen(false)}>
                    Cancel
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        )}

        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <CardTitle className="text-base">Active Profiles</CardTitle>
              <CardDescription>Listed in order of creation.</CardDescription>
            </div>
            <div className="flex items-center gap-2">
              <Input
                placeholder="Search slug..."
                className="w-48 h-8"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
              <Button size="sm" variant="outline" onClick={loadData}>
                Filter
              </Button>
            </div>
          </CardHeader>
          <CardContent>
            {loading ? (
              <div className="py-8 text-center text-sm text-muted-foreground animate-pulse">
                Loading profiles...
              </div>
            ) : (
              <div className="rounded-md border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Slug</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>RU Name</TableHead>
                      <TableHead>EN Name</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {profiles.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={5} className="text-center py-6 text-sm text-muted-foreground">
                          No shipping profiles found.
                        </TableCell>
                      </TableRow>
                    ) : (
                      profiles.map((profile) => {
                        const ruTrans = profile.translations.find((t) => t.locale === 'ru');
                        const enTrans = profile.translations.find((t) => t.locale === 'en');
                        return (
                          <TableRow key={profile.id}>
                            <TableCell className="font-medium">{profile.slug}</TableCell>
                            <TableCell>
                              <Badge variant={profile.active ? 'default' : 'secondary'}>
                                {profile.active ? 'Active' : 'Inactive'}
                              </Badge>
                            </TableCell>
                            <TableCell>{ruTrans?.name || '-'}</TableCell>
                            <TableCell>{enTrans?.name || '-'}</TableCell>
                            <TableCell className="flex items-center justify-end gap-2">
                              <Button
                                size="sm"
                                variant="outline"
                                onClick={() => handleEditClick(profile)}
                              >
                                Edit
                              </Button>
                              <Button
                                size="sm"
                                variant={profile.active ? 'destructive' : 'default'}
                                onClick={() => handleToggleActive(profile)}
                              >
                                {profile.active ? 'Deactivate' : 'Reactivate'}
                              </Button>
                            </TableCell>
                          </TableRow>
                        );
                      })
                    )}
                  </TableBody>
                </Table>
              </div>
            )}

            <div className="mt-4 flex items-center justify-end gap-2">
              <Button
                size="sm"
                variant="outline"
                disabled={page <= 1}
                onClick={() => setPage((p) => p - 1)}
              >
                Previous
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={!hasNext}
                onClick={() => setPage((p) => p + 1)}
              >
                Next
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </PageContainer>
  );
}
