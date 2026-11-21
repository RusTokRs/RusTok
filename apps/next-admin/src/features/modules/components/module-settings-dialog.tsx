'use client';

import {
  Fragment,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode
} from 'react';
import {
  formatSettingsJson,
  parseSettingsObject
} from './module-settings-json';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent/client';

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
import { updateModuleSettings, type GqlOpts } from '@/shared/api/modules';

interface ModuleSettingsFieldsContext {
  settingsText: string;
  onSettingsTextChange: (value: string) => void;
  onFieldValidityChange: (field: string, valid: boolean) => void;
  disabled: boolean;
}

interface PendingSettingsWrite {
  slug: string;
  settings: string;
  snapshot: string;
  revision: number;
  key: string;
  opts: GqlOpts;
  invalidated: boolean;
}

function matchesSettingsContext(
  operation: PendingSettingsWrite,
  slug: string,
  snapshot: string,
  revision: number,
  opts: GqlOpts
) {
  return (
    operation.slug === slug &&
    operation.snapshot === snapshot &&
    operation.revision === revision &&
    operation.opts.token === opts.token &&
    operation.opts.tenantSlug === opts.tenantSlug &&
    operation.opts.tenantId === opts.tenantId &&
    operation.opts.locale === opts.locale &&
    operation.opts.graphqlUrl === opts.graphqlUrl &&
    operation.opts.graphql === opts.graphql
  );
}

interface ModuleSettingsDialogProps {
  moduleSlug: string;
  initialSettings: string;
  expectedRevision: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (
    moduleSlug: string,
    newSettings: string,
    newRevision: number
  ) => void;
  apiOpts?: GqlOpts;
  settingsFields?: (context: ModuleSettingsFieldsContext) => ReactNode;
}

export function ModuleSettingsDialog({
  moduleSlug,
  initialSettings,
  expectedRevision,
  open,
  onOpenChange,
  onSaved,
  apiOpts = {},
  settingsFields
}: ModuleSettingsDialogProps) {
  const t = useTranslations('modules');
  const [settingsText, setSettingsText] = useState(initialSettings);
  const [isSaving, setIsSaving] = useState(false);
  // React state is not a synchronous mutex: two events can arrive before the
  // disabled button renders. This guard is in-memory, not durable recovery.
  const savingRef = useRef(false);
  // Retain the exact operation while this dialog exists. A lost response is
  // not permission to issue a new operation against the same revision.
  const pendingWrite = useRef<PendingSettingsWrite | null>(null);
  const currentContext = useRef({
    moduleSlug,
    initialSettings,
    expectedRevision,
    apiOpts,
    open
  });
  useLayoutEffect(() => {
    currentContext.current = {
      moduleSlug,
      initialSettings,
      expectedRevision,
      apiOpts,
      open
    };
    const operation = pendingWrite.current;
    if (
      operation &&
      (!open ||
        !matchesSettingsContext(
          operation,
          moduleSlug,
          initialSettings,
          expectedRevision,
          apiOpts
        ))
    )
      operation.invalidated = true;
  });
  const mountedRef = useRef(false);
  useLayoutEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [writeUnconfirmed, setWriteUnconfirmed] = useState(false);
  const [invalidFields, setInvalidFields] = useState<Set<string>>(
    () => new Set()
  );
  const hasInvalidFields = invalidFields.size > 0;
  const contextChanged =
    !!pendingWrite.current &&
    (pendingWrite.current.invalidated ||
      !matchesSettingsContext(
        pendingWrite.current,
        moduleSlug,
        initialSettings,
        expectedRevision,
        apiOpts
      ));
  const lockedDraft = writeUnconfirmed || contextChanged;

  useEffect(() => {
    setInvalidFields(new Set());
    if (pendingWrite.current) return;
    setWriteUnconfirmed(false);
    try {
      // Format with 2 spaces for readable editing
      setSettingsText(formatSettingsJson(initialSettings, true));
      setJsonError(null);
    } catch {
      // An empty or malformed owner snapshot is not an empty settings object.
      setSettingsText(initialSettings);
    }
  }, [initialSettings, moduleSlug, open]);

  const handleFormatJson = () => {
    try {
      setSettingsText(formatSettingsJson(settingsText, true));
      setJsonError(null);
      toast.success('JSON formatted');
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Invalid JSON';
      setJsonError(msg);
      toast.error('Invalid JSON structure');
    }
  };

  const handleSave = async () => {
    if (savingRef.current || hasInvalidFields || contextChanged) return;
    let operation = pendingWrite.current;
    if (!operation) {
      let normalized = settingsText.trim();
      try {
        parseSettingsObject(normalized);
        normalized = formatSettingsJson(normalized, false);
        setJsonError(null);
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Invalid JSON';
        setJsonError(msg);
        toast.error('Cannot save: Invalid JSON structure');
        return;
      }
      operation = {
        slug: moduleSlug,
        settings: normalized,
        snapshot: initialSettings,
        revision: expectedRevision,
        key: crypto.randomUUID(),
        opts: { ...apiOpts },
        invalidated: false
      };
      pendingWrite.current = operation;
    }

    savingRef.current = true;
    setIsSaving(true);

    try {
      const result = await updateModuleSettings(
        operation.slug,
        operation.settings,
        operation.revision,
        operation.key,
        operation.opts
      );
      const context = currentContext.current;
      if (
        !mountedRef.current ||
        operation.invalidated ||
        !context.open ||
        !matchesSettingsContext(
          operation,
          context.moduleSlug,
          context.initialSettings,
          context.expectedRevision,
          context.apiOpts
        )
      )
        return;
      toast.success(
        `Settings updated for ${operation.slug} (rev ${result.revision})`
      );
      pendingWrite.current = null;
      onSaved(operation.slug, result.settings, result.revision);
      onOpenChange(false);
    } catch (err) {
      const context = currentContext.current;
      if (
        !mountedRef.current ||
        operation.invalidated ||
        !context.open ||
        !matchesSettingsContext(
          operation,
          context.moduleSlug,
          context.initialSettings,
          context.expectedRevision,
          context.apiOpts
        )
      )
        return;
      const msg =
        err instanceof Error ? err.message : 'Failed to update settings';
      // Transport failure is not proof that the owner did not commit.
      setWriteUnconfirmed(true);
      toast.error(msg);
    } finally {
      savingRef.current = false;
      if (mountedRef.current) setIsSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!savingRef.current) onOpenChange(nextOpen);
      }}
    >
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
          <Fragment key={JSON.stringify([moduleSlug, initialSettings, open])}>
            {settingsFields?.({
              settingsText,
              onSettingsTextChange: setSettingsText,
              onFieldValidityChange: (field, valid) => {
                setInvalidFields((current) => {
                  const next = new Set(current);
                  if (valid) next.delete(field);
                  else next.add(field);
                  return next;
                });
              },
              disabled: isSaving || lockedDraft
            })}
          </Fragment>

          <div className='flex items-center justify-between'>
            <label className='text-muted-foreground text-xs font-medium'>
              Configuration JSON
            </label>
            <Button
              type='button'
              variant='ghost'
              size='sm'
              onClick={handleFormatJson}
              disabled={isSaving || lockedDraft || hasInvalidFields}
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
              setJsonError(null);
            }}
            rows={10}
            className='font-mono text-xs'
            disabled={isSaving || lockedDraft || hasInvalidFields}
          />

          {jsonError && (
            <p className='text-destructive text-xs font-medium'>{jsonError}</p>
          )}
          {lockedDraft && (
            <p role='alert' className='text-destructive text-xs font-medium'>
              {t('settings.writeUnconfirmed')}
            </p>
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
            disabled={isSaving || contextChanged || hasInvalidFields}
          >
            {isSaving ? 'Saving...' : 'Save Settings'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
