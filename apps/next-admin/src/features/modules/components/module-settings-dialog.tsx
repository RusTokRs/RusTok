'use client';

import { useEffect, useState } from 'react';
import { toast } from 'sonner';

import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/shared/ui/shadcn/dialog';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { Switch } from '@/shared/ui/shadcn/switch';
import { updateModuleSettings, type GqlOpts } from '@/shared/api/modules';

interface ModuleSettingsDialogProps {
  moduleSlug: string;
  initialSettings?: string;
  expectedRevision: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (
    moduleSlug: string,
    newSettings: string,
    newRevision: number
  ) => void;
  apiOpts?: GqlOpts;
}

export function ModuleSettingsDialog({
  moduleSlug,
  initialSettings = '{}',
  expectedRevision,
  open,
  onOpenChange,
  onSaved,
  apiOpts = {}
}: ModuleSettingsDialogProps) {
  const [settingsText, setSettingsText] = useState(initialSettings);
  const [useReactions, setUseReactions] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [jsonError, setJsonError] = useState<string | null>(null);

  useEffect(() => {
    try {
      // Format with 2 spaces for readable editing
      const parsed = JSON.parse(initialSettings || '{}');
      setSettingsText(JSON.stringify(parsed, null, 2));
      setUseReactions(moduleSlug === 'forum' && parsed?.useReactions === true);
      setJsonError(null);
    } catch {
      setSettingsText(initialSettings || '{}');
      setUseReactions(false);
    }
  }, [initialSettings, moduleSlug, open]);

  const handleFormatJson = () => {
    try {
      const parsed = JSON.parse(settingsText);
      setSettingsText(JSON.stringify(parsed, null, 2));
      setUseReactions(moduleSlug === 'forum' && parsed?.useReactions === true);
      setJsonError(null);
      toast.success('JSON formatted');
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Invalid JSON';
      setJsonError(msg);
      toast.error('Invalid JSON structure');
    }
  };

  const handleSave = async () => {
    let normalized = settingsText.trim();
    try {
      const parsed = JSON.parse(normalized);
      normalized = JSON.stringify(parsed);
      setJsonError(null);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Invalid JSON';
      setJsonError(msg);
      toast.error('Cannot save: Invalid JSON structure');
      return;
    }

    setIsSaving(true);
    const idempotencyKey = crypto.randomUUID();

    try {
      const result = await updateModuleSettings(
        moduleSlug,
        normalized,
        expectedRevision,
        idempotencyKey,
        apiOpts
      );
      toast.success(
        `Settings updated for ${moduleSlug} (rev ${result.revision})`
      );
      onSaved(moduleSlug, result.settings, result.revision);
      onOpenChange(false);
    } catch (err) {
      const msg =
        err instanceof Error ? err.message : 'Failed to update settings';
      toast.error(msg);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-lg'>
        <DialogHeader>
          <div className='mr-6 flex items-center justify-between gap-2'>
            <DialogTitle className='text-base'>
              Configure Settings: {moduleSlug}
            </DialogTitle>
            <Badge variant='outline' className='font-mono text-xs'>
              Rev #{expectedRevision}
            </Badge>
          </div>
          <DialogDescription className='text-xs'>
            Modify tenant-scoped module configuration. Settings are validated
            and saved atomically with CAS revision checks.
          </DialogDescription>
        </DialogHeader>

        <div className='space-y-3 py-2'>
          {moduleSlug === 'forum' && (
            <div className='flex items-center justify-between rounded-lg border bg-muted/20 p-3'>
              <div className='space-y-1 pr-4'>
                <label className='text-sm font-medium'>
                  Use Reactions instead of internal voting
                </label>
                <p className='text-muted-foreground text-xs'>
                  Forum-specific setting. The shared Reactions module can stay enabled for other modules.
                </p>
              </div>
              <Switch
                checked={useReactions}
                disabled={isSaving}
                onCheckedChange={(checked) => {
                  try {
                    const parsed = JSON.parse(settingsText || '{}');
                    const next = {
                      ...(parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {}),
                      useReactions: checked
                    };
                    setSettingsText(JSON.stringify(next, null, 2));
                    setUseReactions(checked);
                    setJsonError(null);
                  } catch {
                    setJsonError('Invalid JSON structure');
                  }
                }}
              />
            </div>
          )}

          <div className='flex items-center justify-between'>
            <label className='text-muted-foreground text-xs font-medium'>
              Configuration JSON
            </label>
            <Button
              type='button'
              variant='ghost'
              size='sm'
              onClick={handleFormatJson}
              className='h-6 px-2 text-[11px]'
            >
              Format JSON
            </Button>
          </div>

          <Textarea
            value={settingsText}
            onChange={(e) => {
              const value = e.target.value;
              setSettingsText(value);
              if (moduleSlug === 'forum') {
                try {
                  const parsed = JSON.parse(value);
                  setUseReactions(parsed?.useReactions === true);
                } catch {
                  // Keep the last valid switch state until the JSON is fixed.
                }
              }
              setJsonError(null);
            }}
            rows={10}
            className='font-mono text-xs'
            disabled={isSaving}
          />

          {jsonError && (
            <p className='text-destructive text-xs font-medium'>{jsonError}</p>
          )}
        </div>

        <DialogFooter className='gap-2 sm:gap-0'>
          <Button
            type='button'
            variant='outline'
            size='sm'
            onClick={() => onOpenChange(false)}
            disabled={isSaving}
          >
            Cancel
          </Button>
          <Button
            type='button'
            variant='default'
            size='sm'
            onClick={handleSave}
            disabled={isSaving}
          >
            {isSaving ? 'Saving...' : 'Save Settings'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
