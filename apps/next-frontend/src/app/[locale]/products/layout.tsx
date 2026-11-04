import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function ProductsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="product">{children}</ModuleGuard>;
}
