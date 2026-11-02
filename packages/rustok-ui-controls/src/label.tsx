import type { ComponentProps } from "react";
import { clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function Label({ className, ...props }: ComponentProps<"label">) {
  return (
    <label
      data-slot="label"
      className={twMerge(
        clsx(
          "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
          className,
        ),
      )}
      {...props}
    />
  );
}
