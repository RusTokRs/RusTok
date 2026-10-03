import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { listProducts } from '@rustok/product-admin';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { PageContainer } from '@/widgets/app-shell';
import Link from 'next/link';
import { Plus, PackageOpen, ExternalLink, Search } from 'lucide-react';

export const metadata = {
  title: 'RusTok Admin: Catalog'
};

type PageProps = {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

function pickParam(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}

function toPositiveInt(value: string | undefined, fallback: number): number {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleDateString() : '-';
}

function getStatusBadge(status: string) {
  const s = status.toLowerCase();
  if (s === 'active' || s === 'published') {
    return (
      <Badge
        variant='outline'
        className='border-emerald-500/30 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300'
      >
        {status}
      </Badge>
    );
  }
  if (s === 'draft') {
    return <Badge variant='secondary'>{status}</Badge>;
  }
  if (s === 'archived') {
    return (
      <Badge variant='outline' className='text-muted-foreground'>
        {status}
      </Badge>
    );
  }
  return <Badge variant='outline'>{status}</Badge>;
}

export default async function ProductPage({ searchParams }: PageProps) {
  const session = await auth();
  const params = await searchParams;
  const page = toPositiveInt(pickParam(params.page), 1);
  const perPage = toPositiveInt(pickParam(params.perPage), 20);
  const search = pickParam(params.search)?.trim() || undefined;
  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  let result: Awaited<ReturnType<typeof listProducts>> | null = null;
  let error: string | null = null;

  try {
    result = await listProducts(opts, { page, perPage, search });
  } catch (err) {
    error = err instanceof Error ? err.message : 'Failed to load products.';
  }

  const products = result?.items ?? [];
  const total = result?.total ?? 0;
  const hasPrevious = page > 1;
  const hasNext = Boolean(result?.hasNext);
  const baseParams = new URLSearchParams();
  if (search) baseParams.set('search', search);
  baseParams.set('perPage', String(perPage));

  function pageHref(nextPage: number): string {
    const next = new URLSearchParams(baseParams);
    next.set('page', String(nextPage));
    return `/dashboard/product?${next.toString()}`;
  }

  return (
    <PageContainer
      pageTitle='Product catalog'
      pageDescription='Module-owned RusTok product read-side backed by GraphQL.'
      pageHeaderAction={
        <Button asChild>
          <Link href='/dashboard/product/new'>
            <Plus className='mr-1.5 h-4 w-4' />
            Add Product
          </Link>
        </Button>
      }
    >
      <div className='space-y-4'>
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>Catalog filters</CardTitle>
          </CardHeader>
          <CardContent>
            <form
              className='grid gap-3 md:grid-cols-[1fr_auto]'
              action='/dashboard/product'
            >
              <div className='relative'>
                <Search className='text-muted-foreground absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2' />
                <Input
                  name='search'
                  defaultValue={search ?? ''}
                  placeholder='Search products by localized title...'
                  className='pl-9'
                />
              </div>
              <input type='hidden' name='perPage' value={perPage} />
              <Button type='submit' variant='outline'>
                Search
              </Button>
            </form>
          </CardContent>
        </Card>

        {error ? (
          <Card className='border-destructive/50'>
            <CardContent className='text-destructive py-6 text-sm'>
              {error}
            </CardContent>
          </Card>
        ) : (
          <Card>
            <CardHeader className='flex flex-row items-center justify-between gap-4'>
              <CardTitle className='text-base'>
                Products ({total.toLocaleString()})
              </CardTitle>
              <p className='text-muted-foreground text-xs'>
                Page {result?.page ?? page}, {result?.perPage ?? perPage} per
                page
              </p>
            </CardHeader>
            <CardContent>
              <div className='rounded-md border'>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Title</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Vendor</TableHead>
                      <TableHead>Type</TableHead>
                      <TableHead>Seller</TableHead>
                      <TableHead>Published</TableHead>
                      <TableHead className='text-right'>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {products.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={7}
                          className='py-12 text-center'
                        >
                          <div className='flex flex-col items-center justify-center space-y-3'>
                            <PackageOpen className='text-muted-foreground/50 h-10 w-10' />
                            <p className='text-foreground text-sm font-medium'>
                              No products found
                            </p>
                            <p className='text-muted-foreground max-w-sm text-xs'>
                              {search
                                ? 'No products match your current search query. Try clearing the filter.'
                                : 'Get started by creating your first product in this tenant.'}
                            </p>
                            <Button asChild size='sm' className='mt-2'>
                              <Link href='/dashboard/product/new'>
                                <Plus className='mr-1.5 h-3.5 w-3.5' />
                                Add Product
                              </Link>
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ) : (
                      products.map((product) => (
                        <TableRow key={product.id}>
                          <TableCell>
                            <Link
                              href={`/dashboard/product/${product.id}`}
                              className='text-primary font-medium hover:underline'
                            >
                              {product.title || product.handle}
                            </Link>
                            <div className='text-muted-foreground text-xs'>
                              {product.handle}
                            </div>
                          </TableCell>
                          <TableCell>
                            {getStatusBadge(product.status)}
                          </TableCell>
                          <TableCell>{product.vendor ?? '-'}</TableCell>
                          <TableCell>{product.productType ?? '-'}</TableCell>
                          <TableCell className='text-muted-foreground text-xs'>
                            {product.sellerId ?? '-'}
                          </TableCell>
                          <TableCell>
                            {formatDate(product.publishedAt)}
                          </TableCell>
                          <TableCell className='text-right'>
                            <Button asChild variant='ghost' size='sm'>
                              <Link href={`/dashboard/product/${product.id}`}>
                                View
                                <ExternalLink className='ml-1 h-3.5 w-3.5' />
                              </Link>
                            </Button>
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>

              {products.length > 0 && (
                <div className='mt-4 flex items-center justify-end gap-2'>
                  <Button
                    asChild
                    variant='outline'
                    size='sm'
                    disabled={!hasPrevious}
                  >
                    <Link
                      href={hasPrevious ? pageHref(page - 1) : pageHref(page)}
                      aria-disabled={!hasPrevious}
                    >
                      Previous
                    </Link>
                  </Button>
                  <Button
                    asChild
                    variant='outline'
                    size='sm'
                    disabled={!hasNext}
                  >
                    <Link
                      href={hasNext ? pageHref(page + 1) : pageHref(page)}
                      aria-disabled={!hasNext}
                    >
                      Next
                    </Link>
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        )}
      </div>
    </PageContainer>
  );
}
