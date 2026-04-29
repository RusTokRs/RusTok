import { auth } from '@/auth';
import { getProduct } from '../../../../../packages/rustok-product/src';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
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
import { notFound } from 'next/navigation';

export const metadata = {
  title: 'RusTok Admin: Product'
};

type PageProps = { params: Promise<{ productId: string }> };

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString() : '—';
}

function formatMoney(amount: number): string {
  return (amount / 100).toLocaleString(undefined, {
    style: 'currency',
    currency: 'USD'
  });
}

export default async function ProductDetailPage({ params }: PageProps) {
  const { productId } = await params;
  const session = await auth();

  if (productId === 'new') {
    return (
      <PageContainer
        pageTitle='Create product'
        pageDescription='Product write-side is owned by the RusTok product module.'
      >
        <Card>
          <CardContent className='space-y-3 py-6'>
            <p className='text-muted-foreground text-sm'>
              The old demo form has been removed. Product creation must use the
              module-owned product write contract, not the old demo form fields.
            </p>
            <Button asChild variant='outline'>
              <Link href='/dashboard/product'>Back to catalog</Link>
            </Button>
          </CardContent>
        </Card>
      </PageContainer>
    );
  }

  const opts = {
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  let product: Awaited<ReturnType<typeof getProduct>> | null = null;
  let error: string | null = null;

  try {
    product = await getProduct(opts, productId);
  } catch (err) {
    error = err instanceof Error ? err.message : 'Failed to load product.';
  }

  if (!error && !product) {
    notFound();
  }

  const primaryTranslation = product?.translations[0] ?? null;

  return (
    <PageContainer
      pageTitle={primaryTranslation?.title ?? 'Product'}
      pageDescription='RusTok product detail backed by GraphQL.'
      pageHeaderAction={
        <Button asChild variant='outline'>
          <Link href='/dashboard/product'>Back to catalog</Link>
        </Button>
      }
    >
      {error ? (
        <Card className='border-destructive/50'>
          <CardContent className='text-destructive py-6 text-sm'>
            {error}
          </CardContent>
        </Card>
      ) : product ? (
        <div className='space-y-4'>
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Product state</CardTitle>
            </CardHeader>
            <CardContent className='grid gap-4 md:grid-cols-3'>
              <div>
                <p className='text-muted-foreground text-xs'>Status</p>
                <Badge variant='outline'>{product.status}</Badge>
              </div>
              <div>
                <p className='text-muted-foreground text-xs'>Handle</p>
                <p className='text-sm'>{primaryTranslation?.handle ?? '—'}</p>
              </div>
              <div>
                <p className='text-muted-foreground text-xs'>Seller</p>
                <p className='text-sm'>{product.sellerId ?? '—'}</p>
              </div>
              <div>
                <p className='text-muted-foreground text-xs'>Vendor</p>
                <p className='text-sm'>{product.vendor ?? '—'}</p>
              </div>
              <div>
                <p className='text-muted-foreground text-xs'>Type</p>
                <p className='text-sm'>{product.productType ?? '—'}</p>
              </div>
              <div>
                <p className='text-muted-foreground text-xs'>Published</p>
                <p className='text-sm'>{formatDate(product.publishedAt)}</p>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Localized content</CardTitle>
            </CardHeader>
            <CardContent className='space-y-3'>
              {product.translations.length === 0 ? (
                <p className='text-muted-foreground text-sm'>
                  No translations are stored for this product.
                </p>
              ) : (
                product.translations.map((translation) => (
                  <div
                    key={translation.locale}
                    className='rounded-md border p-3'
                  >
                    <div className='mb-2 flex items-center justify-between gap-2'>
                      <p className='font-medium'>{translation.title}</p>
                      <Badge variant='secondary'>{translation.locale}</Badge>
                    </div>
                    <p className='text-muted-foreground text-sm'>
                      {translation.description ?? 'No description.'}
                    </p>
                  </div>
                ))
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Variants</CardTitle>
            </CardHeader>
            <CardContent>
              <div className='rounded-md border'>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Variant</TableHead>
                      <TableHead>SKU</TableHead>
                      <TableHead>Inventory</TableHead>
                      <TableHead>Policy</TableHead>
                      <TableHead>Prices</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {product.variants.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={5}
                          className='text-muted-foreground text-center text-sm'
                        >
                          No variants found.
                        </TableCell>
                      </TableRow>
                    ) : (
                      product.variants.map((variant) => (
                        <TableRow key={variant.id}>
                          <TableCell>{variant.title ?? variant.id}</TableCell>
                          <TableCell>{variant.sku ?? '—'}</TableCell>
                          <TableCell>
                            {variant.inventoryQuantity}{' '}
                            {variant.inStock ? 'in stock' : 'out of stock'}
                          </TableCell>
                          <TableCell>{variant.inventoryPolicy}</TableCell>
                          <TableCell>
                            {variant.prices.length === 0
                              ? '—'
                              : variant.prices
                                  .map(
                                    (price) =>
                                      `${price.currencyCode} ${formatMoney(price.amount)}`
                                  )
                                  .join(', ')}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>
            </CardContent>
          </Card>
        </div>
      ) : null}
    </PageContainer>
  );
}
