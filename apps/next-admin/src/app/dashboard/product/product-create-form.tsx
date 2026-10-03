'use client';

import * as React from 'react';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { Button } from '@/shared/ui/shadcn/button';
import { Switch } from '@/shared/ui/shadcn/switch';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';
import { createProductAction } from './actions';
import { Loader2, ArrowLeft, AlertCircle } from 'lucide-react';

export function ProductCreateForm() {
  const router = useRouter();
  const [isPending, startTransition] = React.useTransition();
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null);
  const [productType, setProductType] = React.useState('simple');
  const [publishImmediately, setPublishImmediately] = React.useState(true);

  async function handleSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setErrorMessage(null);

    const form = e.currentTarget;
    const formData = new FormData(form);
    formData.set('productType', productType);
    formData.set('publish', publishImmediately ? 'true' : 'false');

    startTransition(async () => {
      try {
        await createProductAction(formData);
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : 'Failed to create product.';
        // Next.js redirect throws a special internal error; don't intercept it
        if (message.includes('NEXT_REDIRECT')) {
          return;
        }
        setErrorMessage(message);
      }
    });
  }

  return (
    <form onSubmit={handleSubmit} className='space-y-6 max-w-4xl'>
      {errorMessage && (
        <Alert variant='destructive'>
          <AlertCircle className='h-4 w-4' />
          <AlertTitle>Error creating product</AlertTitle>
          <AlertDescription>{errorMessage}</AlertDescription>
        </Alert>
      )}

      <div className='grid gap-6 md:grid-cols-3'>
        {/* Main Content (2 columns) */}
        <div className='space-y-6 md:col-span-2'>
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>General Information</CardTitle>
              <CardDescription>
                Define the core title, handle, and description of your product.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='space-y-2'>
                <Label htmlFor='title'>
                  Title <span className='text-destructive'>*</span>
                </Label>
                <Input
                  id='title'
                  name='title'
                  placeholder='e.g. Ergonomic Mechanical Keyboard'
                  required
                  disabled={isPending}
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='handle'>URL Handle</Label>
                <Input
                  id='handle'
                  name='handle'
                  placeholder='e.g. ergonomic-mechanical-keyboard (auto-generated if empty)'
                  disabled={isPending}
                />
                <p className='text-muted-foreground text-xs'>
                  Leave blank to automatically derive from the product title.
                </p>
              </div>

              <div className='space-y-2'>
                <Label htmlFor='description'>Description</Label>
                <Textarea
                  id='description'
                  name='description'
                  placeholder='Detailed description, features, specs...'
                  rows={4}
                  disabled={isPending}
                />
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Pricing & Inventory</CardTitle>
              <CardDescription>
                Set the default base price and stock tracking for the primary variant.
              </CardDescription>
            </CardHeader>
            <CardContent className='grid gap-4 sm:grid-cols-3'>
              <div className='space-y-2'>
                <Label htmlFor='price'>Price ($ USD)</Label>
                <Input
                  id='price'
                  name='price'
                  type='number'
                  step='0.01'
                  min='0'
                  placeholder='29.99'
                  disabled={isPending}
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='sku'>SKU</Label>
                <Input
                  id='sku'
                  name='sku'
                  placeholder='KB-ERG-01'
                  disabled={isPending}
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='inventoryQuantity'>Initial Quantity</Label>
                <Input
                  id='inventoryQuantity'
                  name='inventoryQuantity'
                  type='number'
                  min='0'
                  defaultValue='10'
                  disabled={isPending}
                />
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Sidebar settings (1 column) */}
        <div className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Publish Status</CardTitle>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='flex items-center justify-between space-x-2'>
                <Label htmlFor='publish-toggle' className='flex flex-col space-y-1 cursor-pointer'>
                  <span className='font-medium text-sm'>Publish Immediately</span>
                  <span className='text-muted-foreground text-xs font-normal'>
                    Make product visible in storefront and catalog.
                  </span>
                </Label>
                <Switch
                  id='publish-toggle'
                  checked={publishImmediately}
                  onCheckedChange={setPublishImmediately}
                  disabled={isPending}
                />
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Organization</CardTitle>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='space-y-2'>
                <Label htmlFor='productType'>Product Type</Label>
                <Select
                  value={productType}
                  onValueChange={setProductType}
                  disabled={isPending}
                >
                  <SelectTrigger id='productType' className='w-full'>
                    <SelectValue placeholder='Select product type' />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='simple'>Simple Product</SelectItem>
                    <SelectItem value='variable'>Variable Product</SelectItem>
                    <SelectItem value='bundle'>Bundle</SelectItem>
                    <SelectItem value='digital'>Digital Download</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className='space-y-2'>
                <Label htmlFor='vendor'>Vendor / Brand</Label>
                <Input
                  id='vendor'
                  name='vendor'
                  placeholder='e.g. RusTok Devices'
                  disabled={isPending}
                />
              </div>

              <div className='space-y-2'>
                <Label htmlFor='tags'>Tags</Label>
                <Input
                  id='tags'
                  name='tags'
                  placeholder='e.g. keyboard, hardware, featured'
                  disabled={isPending}
                />
                <p className='text-muted-foreground text-xs'>
                  Separate multiple tags with commas.
                </p>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>

      <div className='flex items-center justify-end gap-3 pt-4 border-t'>
        <Button asChild variant='outline' disabled={isPending}>
          <Link href='/dashboard/product'>
            <ArrowLeft className='mr-1.5 h-4 w-4' />
            Cancel
          </Link>
        </Button>
        <Button type='submit' disabled={isPending}>
          {isPending && <Loader2 className='mr-2 h-4 w-4 animate-spin' />}
          Create Product
        </Button>
      </div>
    </form>
  );
}
