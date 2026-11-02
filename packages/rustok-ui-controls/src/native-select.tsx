import type { ComponentProps } from "react";
import { clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** Native selection semantics (including a selectable empty value), aligned with Leptos Select.
 * This is not a replacement for Admin's Radix popup select contract. */
export function NativeSelect({
  className,
  children,
  ...props
}: ComponentProps<"select">) {
  return (
    <select
      data-slot="native-select"
      className={twMerge(
        clsx(
          "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm text-foreground shadow-xs transition-colors focus:outline-none focus:ring-1 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive",
          className,
        ),
      )}
      {...props}
    >
      {children}
    </select>
  );
}
