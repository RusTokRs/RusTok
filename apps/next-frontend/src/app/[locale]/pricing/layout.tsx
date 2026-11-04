import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function PricingLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="pricing">{children}</ModuleGuard>;
}
