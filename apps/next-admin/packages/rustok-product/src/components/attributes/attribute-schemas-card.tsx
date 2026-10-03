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
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '@/shared/ui/shadcn/dialog';
import { Plus, Layers, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import type {
  ProductAttributeSchemaSummary,
  CreateProductAttributeSchemaPayload
} from '../../api/types';

interface AttributeSchemasCardProps {
  schemas: ProductAttributeSchemaSummary[];
  onCreateSchema: (
    payload: CreateProductAttributeSchemaPayload
  ) => Promise<void>;
}

export function AttributeSchemasCard({
  schemas,
  onCreateSchema
}: AttributeSchemasCardProps) {
  const [open, setOpen] = React.useState(false);
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [name, setName] = React.useState('');
  const [code, setCode] = React.useState('');
  const [description, setDescription] = React.useState('');

  const handleNameChange = (val: string) => {
    setName(val);
    const generatedCode = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '');
    if (
      !code ||
      code ===
        val
          .slice(0, -1)
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, '_')
    ) {
      setCode(generatedCode);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.error('Schema name is required');
      return;
    }
    if (!code.trim()) {
      toast.error('Schema code is required');
      return;
    }

    setIsSubmitting(true);
    try {
      await onCreateSchema({
        name: name.trim(),
        code: code.trim(),
        description: description.trim() || undefined
      });
      toast.success('Attribute schema created successfully');
      setOpen(false);
      setName('');
      setCode('');
      setDescription('');
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to create attribute schema'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Card>
      <CardHeader className='flex flex-row items-center justify-between pb-3'>
        <div>
          <CardTitle className='text-base'>Attribute Schemas</CardTitle>
          <CardDescription>
            Reusable attribute bundles that can be bound to catalog categories.
          </CardDescription>
        </div>
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild>
            <Button variant='outline' size='sm'>
              <Plus className='mr-1.5 h-3.5 w-3.5' />
              New Schema
            </Button>
          </DialogTrigger>
          <DialogContent className='sm:max-w-[450px]'>
            <form onSubmit={handleSubmit}>
              <DialogHeader>
                <DialogTitle>Create Attribute Schema</DialogTitle>
                <DialogDescription>
                  Define a schema template that groups attributes for catalog
                  categories.
                </DialogDescription>
              </DialogHeader>

              <div className='grid gap-4 py-4'>
                <div className='space-y-1.5'>
                  <Label htmlFor='schema-name'>Name *</Label>
                  <Input
                    id='schema-name'
                    value={name}
                    onChange={(e) => handleNameChange(e.target.value)}
                    placeholder='e.g. Apparel Specifications'
                    required
                    disabled={isSubmitting}
                  />
                </div>

                <div className='space-y-1.5'>
                  <Label htmlFor='schema-code'>Code *</Label>
                  <Input
                    id='schema-code'
                    value={code}
                    onChange={(e) => setCode(e.target.value)}
                    placeholder='e.g. apparel_specs'
                    required
                    disabled={isSubmitting}
                  />
                </div>

                <div className='space-y-1.5'>
                  <Label htmlFor='schema-desc'>Description</Label>
                  <Input
                    id='schema-desc'
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    placeholder='Optional description'
                    disabled={isSubmitting}
                  />
                </div>
              </div>

              <DialogFooter>
                <Button
                  type='button'
                  variant='outline'
                  onClick={() => setOpen(false)}
                  disabled={isSubmitting}
                >
                  Cancel
                </Button>
                <Button type='submit' disabled={isSubmitting}>
                  {isSubmitting ? (
                    <>
                      <Loader2 className='mr-1.5 h-4 w-4 animate-spin' />
                      Creating...
                    </>
                  ) : (
                    'Create Schema'
                  )}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </CardHeader>
      <CardContent>
        {schemas.length === 0 ? (
          <div className='text-muted-foreground flex flex-col items-center justify-center py-6 text-center text-sm'>
            <Layers className='text-muted-foreground/50 mb-1 h-7 w-7' />
            <p>No attribute schemas defined yet.</p>
          </div>
        ) : (
          <div className='grid gap-3 sm:grid-cols-2 lg:grid-cols-3'>
            {schemas.map((s) => (
              <div
                key={s.id}
                className='bg-muted/20 space-y-1 rounded-md border p-3'
              >
                <div className='flex items-center justify-between'>
                  <p className='text-sm font-medium'>{s.name}</p>
                </div>
                <p className='text-muted-foreground font-mono text-xs'>
                  {s.code}
                </p>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
