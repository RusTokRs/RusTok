import * as React from 'react';
import {
  Button as ShadcnButton,
  buttonVariants
} from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { Spinner } from './Spinner';

type IUButtonVariant =
  | 'default'
  | 'primary'
  | 'secondary'
  | 'ghost'
  | 'outline'
  | 'destructive'
  | 'link';
type IUButtonSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'icon';

const variantMap: Record<IUButtonVariant, string> = {
  default: 'default',
  primary: 'default',
  secondary: 'secondary',
  ghost: 'ghost',
  outline: 'outline',
  destructive: 'destructive',
  link: 'link'
};

const sizeMap: Record<IUButtonSize, string> = {
  xs: 'sm',
  sm: 'sm',
  md: 'default',
  lg: 'lg',
  xl: 'lg',
  icon: 'icon'
};

export interface ButtonProps
  extends Omit<React.ComponentProps<'button'>, 'children'> {
  variant?: IUButtonVariant;
  size?: IUButtonSize;
  loading?: boolean;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
  children?: React.ReactNode;
}

export function Button({
  variant = 'primary',
  size = 'md',
  loading = false,
  leftIcon,
  rightIcon,
  disabled,
  children,
  className,
  ...props
}: ButtonProps) {
  return (
    <ShadcnButton
      variant={variantMap[variant] as Parameters<typeof buttonVariants>[0]['variant']}
      size={sizeMap[size] as Parameters<typeof buttonVariants>[0]['size']}
      disabled={disabled || loading}
      className={cn(className)}
      {...props}
    >
      {loading ? (
        <Spinner size='sm' />
      ) : (
        leftIcon
      )}
      {children}
      {!loading && rightIcon}
    </ShadcnButton>
  );
}
