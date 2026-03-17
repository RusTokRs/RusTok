'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { FormInput, FormTextarea } from '@/shared/ui/forms';
import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import type { GqlOpts } from '../api/posts';
import { addPageBlock, reorderPageBlocks, updatePageBlock, type PageBlock } from '../api/pages';

export function PageBuilder({ pageId, initialBlocks = [], gqlOpts = {} }: { pageId: string; initialBlocks?: PageBlock[]; gqlOpts?: GqlOpts }) {
  const [blocks, setBlocks] = useState<PageBlock[]>(initialBlocks);
  const [draggedId, setDraggedId] = useState<string | null>(null);

  const form = useForm<{ blockType: string; payload: string }>({
    defaultValues: { blockType: 'text', payload: '{"text":"New block"}' }
  });


  async function addBlock(values: { blockType: string; payload: string }) {
    try {
      const data = JSON.parse(values.payload);
      const created = await addPageBlock(pageId, { blockType: values.blockType, position: blocks.length, data }, gqlOpts);
      setBlocks((prev) => [...prev, created].sort((a, b) => a.position - b.position));
      toast.success('Block added');
    } catch {
      toast.error('Invalid block payload JSON');
    }
  }

  async function saveBlock(block: PageBlock, payload: string) {
    try {
      await updatePageBlock(block.id, { data: JSON.parse(payload) }, gqlOpts);
      toast.success('Block updated');
    } catch {
      toast.error('Failed to update block');
    }
  }

  async function handleDrop(targetId: string) {
    if (!draggedId || draggedId === targetId) return;
    const next = [...blocks];
    const from = next.findIndex((block) => block.id === draggedId);
    const to = next.findIndex((block) => block.id === targetId);
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    const positioned = next.map((block, position) => ({ ...block, position }));
    setBlocks(positioned);
    setDraggedId(null);
    await reorderPageBlocks(pageId, positioned.map((block) => block.id), gqlOpts);
    toast.success('Blocks reordered');
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Page builder</CardTitle>
      </CardHeader>
      <CardContent className='space-y-6'>
        <form className='space-y-4' onSubmit={form.handleSubmit(addBlock)}>
          <FormInput control={form.control} name='blockType' label='Block type' />
          <FormTextarea control={form.control} name='payload' label='Block JSON payload' config={{ rows: 4 }} />
          <Button type='submit'>Add block</Button>
        </form>

        <div className='space-y-3'>
          {blocks.map((block) => (
            <BlockRow
              key={block.id}
              block={block}
              onDragStart={() => setDraggedId(block.id)}
              onDrop={() => void handleDrop(block.id)}
              onSave={saveBlock}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function BlockRow({
  block,
  onDragStart,
  onDrop,
  onSave
}: {
  block: PageBlock;
  onDragStart: () => void;
  onDrop: () => void;
  onSave: (block: PageBlock, payload: string) => Promise<void>;
}) {
  const [payload, setPayload] = useState(JSON.stringify(block.data, null, 2));

  return (
    <div
      className='rounded-lg border p-3'
      draggable
      onDragStart={onDragStart}
      onDragOver={(event) => event.preventDefault()}
      onDrop={onDrop}
    >
      <p className='mb-2 text-sm font-medium'>
        #{block.position + 1} {block.blockType}
      </p>
      <textarea
        className='min-h-32 w-full rounded-md border p-2 text-sm'
        value={payload}
        onChange={(event) => setPayload(event.target.value)}
      />
      <Button className='mt-2' size='sm' type='button' onClick={() => void onSave(block, payload)}>
        Save block
      </Button>
    </div>
  );
}
