import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function CheckoutLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="cart">{children}</ModuleGuard>;
}
