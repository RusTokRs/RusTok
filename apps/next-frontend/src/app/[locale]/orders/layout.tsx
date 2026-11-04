import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function OrdersLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="order">{children}</ModuleGuard>;
}
