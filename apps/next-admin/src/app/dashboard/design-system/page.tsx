'use client';

import * as React from 'react';
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Input } from '@/shared/ui/shadcn/input';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { Switch } from '@/shared/ui/shadcn/switch';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter
} from '@/shared/ui/shadcn/card';
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose
} from '@/shared/ui/shadcn/dialog';
import { Alert, AlertTitle, AlertDescription } from '@/shared/ui/shadcn/alert';
import { Skeleton } from '@/shared/ui/shadcn/skeleton';
import { Separator } from '@/shared/ui/shadcn/separator';
import {
  Check,
  Copy,
  Layers,
  Sparkles,
  AlertCircle,
  Search,
  Trash2,
  RefreshCw,
  Save
} from 'lucide-react';

type VariantType =
  'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link';
type SizeType = 'default' | 'sm' | 'lg' | 'icon';

export default function DesignSystemPage() {
  // Playground state
  const [btnVariant, setBtnVariant] = React.useState<VariantType>('default');
  const [btnSize, setBtnSize] = React.useState<SizeType>('default');
  const [btnDisabled, setBtnDisabled] = React.useState(false);
  const [btnLoading, setBtnLoading] = React.useState(false);
  const [btnText, setBtnText] = React.useState('Save Changes');
  const [copiedSnippet, setCopiedSnippet] = React.useState<string | null>(null);

  // Recipe states
  const [searchQuery, setSearchQuery] = React.useState('');
  const [selectedTag, setSelectedTag] = React.useState<string>('all');
  const [isSaving, setIsSaving] = React.useState(false);
  const [saveSuccess, setSaveSuccess] = React.useState(false);

  // Copy helper
  const handleCopy = (code: string, label: string) => {
    navigator.clipboard.writeText(code);
    setCopiedSnippet(label);
    setTimeout(() => setCopiedSnippet(null), 2000);
  };

  // Snippets
  const nextSnippet = `<Button variant="${btnVariant}" size="${btnSize}"${btnDisabled ? ' disabled' : ''}>
  ${btnLoading ? '<Spinner className="mr-2 h-4 w-4 animate-spin" />' : ''}${btnText}
</Button>`;

  const leptosSnippet = `<Button
    variant=ButtonVariant::${btnVariant.charAt(0).toUpperCase() + btnVariant.slice(1)}
    size=Size::${btnSize === 'default' ? 'Md' : btnSize.charAt(0).toUpperCase() + btnSize.slice(1)}
    disabled=${btnDisabled}
>
    "${btnText}"
</Button>`;

  const handleSimulateSave = () => {
    setIsSaving(true);
    setTimeout(() => {
      setIsSaving(false);
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2500);
    }, 1200);
  };

  return (
    <div className='flex flex-1 flex-col gap-8 p-6 md:p-10'>
      {/* Header */}
      <div className='flex flex-col gap-2 border-b pb-6 sm:flex-row sm:items-center sm:justify-between'>
        <div>
          <div className='flex items-center gap-2'>
            <Layers className='text-primary h-6 w-6' />
            <h1 className='text-3xl font-bold tracking-tight'>UI Workbench</h1>
            <Badge variant='outline' className='ml-2 font-mono text-xs'>
              FFA Design System
            </Badge>
          </div>
          <p className='text-muted-foreground mt-1 text-sm'>
            Autonomous component gallery for RusToK. Cross-framework API parity
            across Next.js (React), Leptos (Rust/WASM), and Flutter.
          </p>
        </div>
      </div>

      {/* Interactive Controls & Code Generator */}
      <section className='space-y-4'>
        <div className='flex items-center gap-2'>
          <Sparkles className='text-primary h-5 w-5' />
          <h2 className='text-xl font-semibold tracking-tight'>
            Interactive Button Inspector
          </h2>
        </div>

        <div className='grid gap-6 lg:grid-cols-12'>
          {/* Controls Panel */}
          <Card className='lg:col-span-5'>
            <CardHeader>
              <CardTitle className='text-base'>Props Controls</CardTitle>
              <CardDescription>
                Adjust props in real-time to preview styles and copy code.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-4 text-sm'>
              <div className='space-y-1.5'>
                <label className='text-muted-foreground text-xs font-medium'>
                  Variant
                </label>
                <div className='flex flex-wrap gap-1.5'>
                  {(
                    [
                      'default',
                      'destructive',
                      'outline',
                      'secondary',
                      'ghost',
                      'link'
                    ] as VariantType[]
                  ).map((v) => (
                    <Button
                      key={v}
                      type='button'
                      variant={btnVariant === v ? 'default' : 'outline'}
                      size='sm'
                      onClick={() => setBtnVariant(v)}
                      className='capitalize'
                    >
                      {v}
                    </Button>
                  ))}
                </div>
              </div>

              <div className='space-y-1.5'>
                <label className='text-muted-foreground text-xs font-medium'>
                  Size
                </label>
                <div className='flex gap-1.5'>
                  {(['sm', 'default', 'lg', 'icon'] as SizeType[]).map((s) => (
                    <Button
                      key={s}
                      type='button'
                      variant={btnSize === s ? 'default' : 'outline'}
                      size='sm'
                      onClick={() => setBtnSize(s)}
                      className='capitalize'
                    >
                      {s === 'default' ? 'md (default)' : s}
                    </Button>
                  ))}
                </div>
              </div>

              <div className='space-y-1.5'>
                <label className='text-muted-foreground text-xs font-medium'>
                  Button Label
                </label>
                <Input
                  value={btnText}
                  onChange={(e) => setBtnText(e.target.value)}
                  placeholder='Button label'
                />
              </div>

              <div className='flex items-center justify-between border-t pt-3'>
                <span className='text-xs font-medium'>Disabled</span>
                <Switch
                  checked={btnDisabled}
                  onCheckedChange={setBtnDisabled}
                />
              </div>

              <div className='flex items-center justify-between'>
                <span className='text-xs font-medium'>Simulate Loading</span>
                <Switch checked={btnLoading} onCheckedChange={setBtnLoading} />
              </div>
            </CardContent>
          </Card>

          {/* Live Preview & Code Export */}
          <Card className='flex flex-col lg:col-span-7'>
            <CardHeader>
              <CardTitle className='text-base'>Live Preview</CardTitle>
              <CardDescription>
                Rendered with native Tailwind CSS v4 design tokens.
              </CardDescription>
            </CardHeader>
            <CardContent className='bg-muted/20 flex min-h-[160px] flex-1 items-center justify-center rounded-lg border border-dashed p-6'>
              <Button
                variant={btnVariant}
                size={btnSize}
                disabled={btnDisabled || btnLoading}
              >
                {btnLoading && (
                  <RefreshCw className='mr-2 h-4 w-4 animate-spin' />
                )}
                {btnSize === 'icon' ? (
                  <Sparkles className='h-4 w-4' />
                ) : (
                  btnText
                )}
              </Button>
            </CardContent>
            <CardFooter className='bg-muted/10 flex-col items-stretch gap-3 border-t p-4'>
              <div className='flex items-center justify-between'>
                <span className='text-muted-foreground text-xs font-semibold uppercase'>
                  Export Code Snippet
                </span>
                <div className='flex gap-2'>
                  <Button
                    variant='ghost'
                    size='sm'
                    className='h-7 text-xs'
                    onClick={() => handleCopy(nextSnippet, 'next')}
                  >
                    {copiedSnippet === 'next' ? (
                      <Check className='mr-1 h-3.5 w-3.5 text-green-500' />
                    ) : (
                      <Copy className='mr-1 h-3.5 w-3.5' />
                    )}
                    Next.js (TSX)
                  </Button>
                  <Button
                    variant='ghost'
                    size='sm'
                    className='h-7 text-xs'
                    onClick={() => handleCopy(leptosSnippet, 'leptos')}
                  >
                    {copiedSnippet === 'leptos' ? (
                      <Check className='mr-1 h-3.5 w-3.5 text-green-500' />
                    ) : (
                      <Copy className='mr-1 h-3.5 w-3.5' />
                    )}
                    Leptos (Rust)
                  </Button>
                </div>
              </div>
              <pre className='overflow-x-auto rounded-md bg-slate-950 p-3 text-xs text-slate-100 dark:bg-black'>
                <code>{nextSnippet}</code>
              </pre>
            </CardFooter>
          </Card>
        </div>
      </section>

      <Separator />

      {/* Component Matrix Showcase */}
      <section className='space-y-6'>
        <h2 className='text-xl font-semibold tracking-tight'>
          Component Baseline Matrix
        </h2>

        {/* Buttons Gallery */}
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>Button Variants & Sizes</CardTitle>
            <CardDescription>
              All standard variants configured with identical tokens to
              Leptos/Flutter.
            </CardDescription>
          </CardHeader>
          <CardContent className='space-y-6'>
            <div className='flex flex-wrap items-center gap-3'>
              <Button variant='default'>Default</Button>
              <Button variant='secondary'>Secondary</Button>
              <Button variant='destructive'>Destructive</Button>
              <Button variant='outline'>Outline</Button>
              <Button variant='ghost'>Ghost</Button>
              <Button variant='link'>Link</Button>
            </div>
            <div className='flex flex-wrap items-center gap-3'>
              <Button size='sm'>Small (sm)</Button>
              <Button size='default'>Medium (default)</Button>
              <Button size='lg'>Large (lg)</Button>
              <Button size='icon' aria-label='Icon'>
                <Sparkles className='h-4 w-4' />
              </Button>
              <Button disabled>Disabled</Button>
            </div>
          </CardContent>
        </Card>

        {/* Badges Gallery */}
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>Badge Variants</CardTitle>
            <CardDescription>Status indicators and labels.</CardDescription>
          </CardHeader>
          <CardContent className='flex flex-wrap gap-3'>
            <Badge variant='default'>Default Badge</Badge>
            <Badge variant='secondary'>Secondary Badge</Badge>
            <Badge variant='destructive'>Destructive Badge</Badge>
            <Badge variant='outline'>Outline Badge</Badge>
          </CardContent>
        </Card>

        {/* Form Controls */}
        <Card>
          <CardHeader>
            <CardTitle className='text-base'>Form Controls</CardTitle>
            <CardDescription>
              Inputs, textareas, checkboxes, and switches.
            </CardDescription>
          </CardHeader>
          <CardContent className='grid gap-6 sm:grid-cols-2 lg:grid-cols-3'>
            <div className='space-y-2'>
              <label className='text-muted-foreground text-xs font-medium'>
                Standard Input
              </label>
              <Input placeholder='Enter text here...' />
            </div>

            <div className='space-y-2'>
              <label className='text-muted-foreground text-xs font-medium'>
                Disabled Input
              </label>
              <Input disabled value='Locked value' />
            </div>

            <div className='space-y-2'>
              <label className='text-muted-foreground text-xs font-medium'>
                Invalid Input State
              </label>
              <Input
                aria-invalid='true'
                defaultValue='Invalid input'
                className='border-destructive focus-visible:ring-destructive'
              />
            </div>

            <div className='space-y-2 sm:col-span-2 lg:col-span-3'>
              <label className='text-muted-foreground text-xs font-medium'>
                Textarea
              </label>
              <Textarea
                rows={3}
                placeholder='Detailed description or markdown content...'
              />
            </div>

            <div className='flex items-center space-x-2'>
              <Checkbox id='ds-terms' defaultChecked />
              <label
                htmlFor='ds-terms'
                className='text-sm leading-none font-medium peer-disabled:cursor-not-allowed peer-disabled:opacity-70'
              >
                Accept terms and conditions
              </label>
            </div>

            <div className='flex items-center space-x-2'>
              <Switch id='ds-notifications' defaultChecked />
              <label
                htmlFor='ds-notifications'
                className='text-sm leading-none font-medium'
              >
                Enable push notifications
              </label>
            </div>
          </CardContent>
        </Card>

        {/* Feedback & Overlays */}
        <div className='grid gap-6 md:grid-cols-2'>
          <Alert>
            <AlertCircle className='h-4 w-4' />
            <AlertTitle>Default Alert</AlertTitle>
            <AlertDescription>
              This is a standard system notice communicating important status.
            </AlertDescription>
          </Alert>

          <Alert variant='destructive'>
            <AlertCircle className='h-4 w-4' />
            <AlertTitle>Destructive Alert</AlertTitle>
            <AlertDescription>
              Critical action warning. Verify all dependent modules before
              proceeding.
            </AlertDescription>
          </Alert>
        </div>
      </section>

      <Separator />

      {/* Composite Recipes Section */}
      <section className='space-y-6'>
        <div className='flex items-center gap-2'>
          <Layers className='text-primary h-5 w-5' />
          <h2 className='text-xl font-semibold tracking-tight'>
            Composite Pattern Recipes (FFA)
          </h2>
        </div>

        <div className='grid gap-6 lg:grid-cols-3'>
          {/* Recipe 1: Confirm Delete Dialog */}
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Confirm Action Dialog</CardTitle>
              <CardDescription>
                Pattern for irreversible actions (e.g. deleting resources).
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-4'>
              <p className='text-muted-foreground text-sm'>
                Standard dialog structure with destructive confirmation and
                cancel.
              </p>
              <Dialog>
                <DialogTrigger asChild>
                  <Button variant='destructive' size='sm'>
                    <Trash2 className='mr-2 h-4 w-4' />
                    Delete Module
                  </Button>
                </DialogTrigger>
                <DialogContent>
                  <DialogHeader>
                    <DialogTitle>Are you absolutely sure?</DialogTitle>
                    <DialogDescription>
                      This action cannot be undone. This will permanently delete
                      the module and remove all associated cache entries.
                    </DialogDescription>
                  </DialogHeader>
                  <DialogFooter className='gap-2 sm:gap-0'>
                    <DialogClose asChild>
                      <Button variant='outline'>Cancel</Button>
                    </DialogClose>
                    <DialogClose asChild>
                      <Button variant='destructive'>Confirm Delete</Button>
                    </DialogClose>
                  </DialogFooter>
                </DialogContent>
              </Dialog>
            </CardContent>
          </Card>

          {/* Recipe 2: Search & Filter Toolbar */}
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>
                Resource Filter Toolbar
              </CardTitle>
              <CardDescription>
                Compact search and status filter bar for data tables.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-3'>
              <div className='relative'>
                <Search className='text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4' />
                <Input
                  placeholder='Filter resources...'
                  className='pl-8'
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                />
              </div>
              <div className='flex gap-1.5'>
                {['all', 'active', 'draft'].map((tag) => (
                  <Badge
                    key={tag}
                    variant={selectedTag === tag ? 'default' : 'outline'}
                    className='cursor-pointer capitalize'
                    onClick={() => setSelectedTag(tag)}
                  >
                    {tag}
                  </Badge>
                ))}
              </div>
            </CardContent>
          </Card>

          {/* Recipe 3: Save & Action Bar */}
          <Card>
            <CardHeader>
              <CardTitle className='text-base'>Save Action Toolbar</CardTitle>
              <CardDescription>
                Form action bar with loading feedback and status notifications.
              </CardDescription>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='bg-muted/40 flex items-center justify-between rounded-lg border p-3'>
                <div className='flex items-center gap-2'>
                  <span className='h-2 w-2 rounded-full bg-amber-500' />
                  <span className='text-xs font-medium'>Unsaved changes</span>
                </div>
                <div className='flex gap-2'>
                  <Button
                    variant='outline'
                    size='sm'
                    disabled={isSaving}
                    onClick={() => {}}
                  >
                    Reset
                  </Button>
                  <Button
                    size='sm'
                    disabled={isSaving}
                    onClick={handleSimulateSave}
                  >
                    {isSaving ? (
                      <RefreshCw className='mr-1.5 h-3.5 w-3.5 animate-spin' />
                    ) : (
                      <Save className='mr-1.5 h-3.5 w-3.5' />
                    )}
                    {isSaving ? 'Saving...' : 'Save'}
                  </Button>
                </div>
              </div>
              {saveSuccess && (
                <div className='flex items-center gap-2 rounded-md bg-green-500/10 p-2 text-xs text-green-700 dark:text-green-400'>
                  <Check className='h-3.5 w-3.5' />
                  <span>Changes saved successfully!</span>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </section>
    </div>
  );
}
