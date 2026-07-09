'use client';

import { useEffect, useState } from 'react';
import { toast } from 'sonner';

import {
  CreateOAuthAppInput,
  CreateOAuthAppResult,
  ManualAppType,
  OAuthApp,
  UpdateOAuthAppInput
} from '@/entities/oauth-app';
import {
  createOAuthApp,
  revokeOAuthApp,
  rotateOAuthAppSecret,
  updateOAuthApp
} from '@/shared/api/oauth-apps';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/shared/ui/shadcn/dialog';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Textarea } from '@/shared/ui/shadcn/textarea';

const DEFAULTS_BY_TYPE: Record<
  ManualAppType,
  { redirectUris: string[]; scopes: string[]; grantTypes: string[] }
> = {
  ThirdParty: {
    redirectUris: ['http://localhost:3000/auth/callback'],
    scopes: [],
    grantTypes: ['authorization_code', 'refresh_token']
  },
  Mobile: {
    redirectUris: ['myapp://auth/callback'],
    scopes: [],
    grantTypes: ['authorization_code', 'refresh_token']
  },
  Service: {
    redirectUris: [],
    scopes: [],
    grantTypes: ['client_credentials']
  }
};

function normalizeMultiline(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function linesToText(lines: string[]): string {
  return lines.join('\n');
}

function parseCreateInput(
  name: string,
  slug: string,
  description: string,
  iconUrl: string,
  appType: ManualAppType,
  redirectUrisText: string,
  scopesText: string,
  grantTypesText: string
): CreateOAuthAppInput {
  const redirectUris = normalizeMultiline(redirectUrisText);
  const scopes = normalizeMultiline(scopesText);
  const grantTypes = normalizeMultiline(grantTypesText);

  return {
    name: name.trim(),
    slug: slug.trim(),
    description: description.trim() || undefined,
    iconUrl: iconUrl.trim() || undefined,
    appType,
    redirectUris: redirectUris.length > 0 ? redirectUris : undefined,
    scopes,
    grantTypes
  };
}

function parseUpdateInput(
  name: string,
  description: string,
  iconUrl: string,
  redirectUrisText: string,
  scopesText: string,
  grantTypesText: string
): UpdateOAuthAppInput {
  return {
    name: name.trim(),
    description: description.trim() || undefined,
    iconUrl: iconUrl.trim() || undefined,
    redirectUris: normalizeMultiline(redirectUrisText),
    scopes: normalizeMultiline(scopesText),
    grantTypes: normalizeMultiline(grantTypesText)
  };
}

function SecretDialog({
  open,
  title,
  description,
  secret,
  onClose
}: {
  open: boolean;
  title: string;
  description: string;
  secret: string;
  onClose: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className='text-green-600'>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <div className='bg-muted rounded border p-4 font-mono text-sm break-all'>
          {secret}
        </div>
        <p className='text-destructive text-sm font-medium'>
          Store this secret safely. It is shown only once.
        </p>
        <DialogFooter>
          <Button onClick={onClose} className='w-full'>
            I have saved it safely
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function AppFields({
  name,
  onNameChange,
  slug,
  onSlugChange,
  disableSlug,
  description,
  onDescriptionChange,
  iconUrl,
  onIconUrlChange,
  appType,
  onAppTypeChange,
  disableType,
  redirectUrisText,
  onRedirectUrisChange,
  scopesText,
  onScopesChange,
  grantTypesText,
  onGrantTypesChange
}: {
  name: string;
  onNameChange: (value: string) => void;
  slug?: string;
  onSlugChange?: (value: string) => void;
  disableSlug?: boolean;
  description: string;
  onDescriptionChange: (value: string) => void;
  iconUrl: string;
  onIconUrlChange: (value: string) => void;
  appType?: ManualAppType;
  onAppTypeChange?: (value: ManualAppType) => void;
  disableType?: boolean;
  redirectUrisText: string;
  onRedirectUrisChange: (value: string) => void;
  scopesText: string;
  onScopesChange: (value: string) => void;
  grantTypesText: string;
  onGrantTypesChange: (value: string) => void;
}) {
  return (
    <div className='grid gap-4 py-4'>
      <div className='grid gap-2'>
        <Label htmlFor='oauth-app-name'>App Name</Label>
        <Input
          id='oauth-app-name'
          value={name}
          onChange={(event) => onNameChange(event.target.value)}
          placeholder='My Integration'
          required
        />
      </div>

      {onSlugChange ? (
        <div className='grid gap-2'>
          <Label htmlFor='oauth-app-slug'>Slug / Bundle ID</Label>
          <Input
            id='oauth-app-slug'
            value={slug ?? ''}
            onChange={(event) => onSlugChange(event.target.value)}
            placeholder='com.example.app'
            required
            disabled={disableSlug}
          />
        </div>
      ) : null}

      {onAppTypeChange ? (
        <div className='grid gap-2'>
          <Label htmlFor='oauth-app-type'>App Type</Label>
          <Select
            value={appType}
            onValueChange={(value) => onAppTypeChange(value as ManualAppType)}
            disabled={disableType}
          >
            <SelectTrigger id='oauth-app-type'>
              <SelectValue placeholder='Select app type' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='ThirdParty'>Third Party</SelectItem>
              <SelectItem value='Mobile'>Mobile</SelectItem>
              <SelectItem value='Service'>Service</SelectItem>
            </SelectContent>
          </Select>
        </div>
      ) : null}

      <div className='grid gap-2'>
        <Label htmlFor='oauth-app-description'>Description</Label>
        <Textarea
          id='oauth-app-description'
          value={description}
          onChange={(event) => onDescriptionChange(event.target.value)}
          placeholder='Optional description'
        />
      </div>

      <div className='grid gap-2'>
        <Label htmlFor='oauth-app-icon-url'>Icon URL</Label>
        <Input
          id='oauth-app-icon-url'
          value={iconUrl}
          onChange={(event) => onIconUrlChange(event.target.value)}
          placeholder='https://example.com/icon.png'
        />
      </div>

      <div className='grid gap-2'>
        <Label htmlFor='oauth-app-redirect-uris'>Redirect URIs</Label>
        <Textarea
          id='oauth-app-redirect-uris'
          value={redirectUrisText}
          onChange={(event) => onRedirectUrisChange(event.target.value)}
          placeholder='One URI per line'
          rows={4}
        />
      </div>

      <div className='grid gap-2'>
        <Label htmlFor='oauth-app-scopes'>Scopes</Label>
        <Textarea
          id='oauth-app-scopes'
          value={scopesText}
          onChange={(event) => onScopesChange(event.target.value)}
          placeholder='One scope per line'
          rows={4}
        />
      </div>

      <div className='grid gap-2'>
        <Label htmlFor='oauth-app-grant-types'>Grant Types</Label>
        <Textarea
          id='oauth-app-grant-types'
          value={grantTypesText}
          onChange={(event) => onGrantTypesChange(event.target.value)}
          placeholder={'authorization_code\nrefresh_token'}
          rows={3}
        />
      </div>
    </div>
  );
}

export function CreateAppDialog({
  token,
  tenantSlug,
  open,
  onOpenChange,
  onSuccess
}: {
  token?: string | null;
  tenantSlug?: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess: (app: OAuthApp) => void;
}) {
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [description, setDescription] = useState('');
  const [iconUrl, setIconUrl] = useState('');
  const [appType, setAppType] = useState<ManualAppType>('ThirdParty');
  const [redirectUrisText, setRedirectUrisText] = useState(
    linesToText(DEFAULTS_BY_TYPE.ThirdParty.redirectUris)
  );
  const [scopesText, setScopesText] = useState('');
  const [grantTypesText, setGrantTypesText] = useState(
    linesToText(DEFAULTS_BY_TYPE.ThirdParty.grantTypes)
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<CreateOAuthAppResult | null>(null);

  useEffect(() => {
    if (!open) {
      setError(null);
      setLoading(false);
    }
  }, [open]);

  const applyDefaults = (nextType: ManualAppType) => {
    setAppType(nextType);
    setRedirectUrisText(linesToText(DEFAULTS_BY_TYPE[nextType].redirectUris));
    setGrantTypesText(linesToText(DEFAULTS_BY_TYPE[nextType].grantTypes));
  };

  const resetForm = () => {
    setName('');
    setSlug('');
    setDescription('');
    setIconUrl('');
    setError(null);
    applyDefaults('ThirdParty');
    setScopesText('');
    setResult(null);
  };

  const handleClose = () => {
    onOpenChange(false);
    resetForm();
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!token) {
      setError('Sign in again to manage app connections.');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const created = await createOAuthApp(
        token,
        tenantSlug,
        parseCreateInput(
          name,
          slug,
          description,
          iconUrl,
          appType,
          redirectUrisText,
          scopesText,
          grantTypesText
        )
      );

      setResult(created);
      onSuccess(created.app);
    } catch (nextError) {
      const message =
        nextError instanceof Error
          ? nextError.message
          : 'Failed to create OAuth app';
      setError(message);
      toast.error(message);
    } finally {
      setLoading(false);
    }
  };

  if (result) {
    return (
      <SecretDialog
        open={open}
        title='App Created Successfully'
        description='Your new OAuth application has been created. Here is the client secret.'
        secret={result.clientSecret}
        onClose={handleClose}
      />
    );
  }

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && handleClose()}>
      <DialogContent className='sm:max-w-[640px]'>
        <DialogHeader>
          <DialogTitle>Create New App Connection</DialogTitle>
          <DialogDescription>
            Register a manual third-party, mobile, or service application.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <AppFields
            name={name}
            onNameChange={setName}
            slug={slug}
            onSlugChange={setSlug}
            description={description}
            onDescriptionChange={setDescription}
            iconUrl={iconUrl}
            onIconUrlChange={setIconUrl}
            appType={appType}
            onAppTypeChange={applyDefaults}
            redirectUrisText={redirectUrisText}
            onRedirectUrisChange={setRedirectUrisText}
            scopesText={scopesText}
            onScopesChange={setScopesText}
            grantTypesText={grantTypesText}
            onGrantTypesChange={setGrantTypesText}
          />
          {error ? <p className='text-destructive text-sm'>{error}</p> : null}
          <DialogFooter>
            <Button type='button' variant='outline' onClick={handleClose}>
              Cancel
            </Button>
            <Button type='submit' disabled={loading}>
              {loading ? 'Creating...' : 'Create App'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function EditAppDialog({
  token,
  tenantSlug,
  app,
  open,
  onOpenChange,
  onSuccess
}: {
  token?: string | null;
  tenantSlug?: string | null;
  app: OAuthApp | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess: (app: OAuthApp) => void;
}) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [iconUrl, setIconUrl] = useState('');
  const [redirectUrisText, setRedirectUrisText] = useState('');
  const [scopesText, setScopesText] = useState('');
  const [grantTypesText, setGrantTypesText] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!app) {
      return;
    }

    setName(app.name);
    setDescription(app.description ?? '');
    setIconUrl(app.iconUrl ?? '');
    setRedirectUrisText(linesToText(app.redirectUris));
    setScopesText(linesToText(app.scopes));
    setGrantTypesText(linesToText(app.grantTypes));
    setError(null);
  }, [app]);

  if (!app) {
    return null;
  }

  const handleClose = () => {
    onOpenChange(false);
    setError(null);
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!token) {
      setError('Sign in again to manage app connections.');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const updated = await updateOAuthApp(
        token,
        tenantSlug,
        app.id,
        parseUpdateInput(
          name,
          description,
          iconUrl,
          redirectUrisText,
          scopesText,
          grantTypesText
        )
      );

      onSuccess(updated);
      toast.success('OAuth app updated');
    } catch (nextError) {
      const message =
        nextError instanceof Error
          ? nextError.message
          : 'Failed to update OAuth app';
      setError(message);
      toast.error(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && handleClose()}>
      <DialogContent className='sm:max-w-[640px]'>
        <DialogHeader>
          <DialogTitle>Edit App Connection</DialogTitle>
          <DialogDescription>
            Update redirects, scopes, and grants for this manual application.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <AppFields
            name={name}
            onNameChange={setName}
            description={description}
            onDescriptionChange={setDescription}
            iconUrl={iconUrl}
            onIconUrlChange={setIconUrl}
            redirectUrisText={redirectUrisText}
            onRedirectUrisChange={setRedirectUrisText}
            scopesText={scopesText}
            onScopesChange={setScopesText}
            grantTypesText={grantTypesText}
            onGrantTypesChange={setGrantTypesText}
          />
          {error ? <p className='text-destructive text-sm'>{error}</p> : null}
          <DialogFooter>
            <Button type='button' variant='outline' onClick={handleClose}>
              Cancel
            </Button>
            <Button type='submit' disabled={loading}>
              {loading ? 'Saving...' : 'Save Changes'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function RotateSecretDialog({
  token,
  tenantSlug,
  app,
  open,
  onOpenChange,
  onSuccess
}: {
  token?: string | null;
  tenantSlug?: string | null;
  app: OAuthApp | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess: (app: OAuthApp) => void;
}) {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<CreateOAuthAppResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!app) {
    return null;
  }

  const handleRotate = async () => {
    if (!token) {
      setError('Sign in again to manage app connections.');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const rotated = await rotateOAuthAppSecret(token, tenantSlug, app.id);
      setResult(rotated);
      onSuccess(rotated.app);
    } catch (nextError) {
      const message =
        nextError instanceof Error
          ? nextError.message
          : 'Failed to rotate client secret';
      setError(message);
      toast.error(message);
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    onOpenChange(false);
    setResult(null);
    setError(null);
  };

  if (result) {
    return (
      <SecretDialog
        open={open}
        title='Secret Rotated'
        description='The old secret has been invalidated. Here is the new client secret.'
        secret={result.clientSecret}
        onClose={handleClose}
      />
    );
  }

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && handleClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Rotate Client Secret</DialogTitle>
          <DialogDescription>
            Rotate the client secret for{' '}
            <span className='font-semibold'>{app.name}</span>. The old secret
            stops working immediately.
          </DialogDescription>
        </DialogHeader>
        {error ? <p className='text-destructive text-sm'>{error}</p> : null}
        <DialogFooter>
          <Button type='button' variant='outline' onClick={handleClose}>
            Cancel
          </Button>
          <Button
            variant='destructive'
            onClick={handleRotate}
            disabled={loading}
          >
            {loading ? 'Rotating...' : 'Rotate Secret'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function RevokeAppDialog({
  token,
  tenantSlug,
  app,
  open,
  onOpenChange,
  onSuccess
}: {
  token?: string | null;
  tenantSlug?: string | null;
  app: OAuthApp | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess: (appId: string) => void;
}) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!app) {
    return null;
  }

  const handleClose = () => {
    onOpenChange(false);
    setError(null);
  };

  const handleRevoke = async () => {
    if (!token) {
      setError('Sign in again to manage app connections.');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await revokeOAuthApp(token, tenantSlug, app.id);
      onSuccess(app.id);
      toast.success('OAuth app revoked');
    } catch (nextError) {
      const message =
        nextError instanceof Error
          ? nextError.message
          : 'Failed to revoke OAuth app';
      setError(message);
      toast.error(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && handleClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className='text-destructive'>
            Revoke Application
          </DialogTitle>
          <DialogDescription>
            Revoke <span className='font-semibold'>{app.name}</span> and
            invalidate all active tokens for this app.
          </DialogDescription>
        </DialogHeader>
        {error ? <p className='text-destructive text-sm'>{error}</p> : null}
        <DialogFooter>
          <Button type='button' variant='outline' onClick={handleClose}>
            Cancel
          </Button>
          <Button
            variant='destructive'
            onClick={handleRevoke}
            disabled={loading}
          >
            {loading ? 'Revoking...' : 'Revoke Application'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
