import { Badge } from '@/shared/ui/shadcn/badge';
import { AppType } from '../model';

export function OAuthAppTypeBadge({ appType }: { appType: AppType }) {
  let variant: 'default' | 'secondary' | 'outline' | 'destructive' = 'default';
  let label: string = appType;

  switch (appType) {
    case 'Embedded':
      variant = 'secondary';
      break;
    case 'FirstParty':
      variant = 'default';
      label = 'First Party';
      break;
    case 'Mobile':
      variant = 'default';
      break;
    case 'Service':
      variant = 'outline';
      break;
    case 'ThirdParty':
      variant = 'outline';
      label = 'Third Party';
      break;
  }

  return (
    <Badge variant={variant} className='whitespace-nowrap'>
      {label}
    </Badge>
  );
}
