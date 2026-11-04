import type { ReactNode } from "react";
import { ModuleGuard } from "@/modules/module-guard";

export default function ProductRelationsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="product_relations">{children}</ModuleGuard>;
}
