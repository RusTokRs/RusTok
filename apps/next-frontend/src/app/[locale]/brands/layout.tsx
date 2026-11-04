import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function BrandsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="brand">{children}</ModuleGuard>;
}
