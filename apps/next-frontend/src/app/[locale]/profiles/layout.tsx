import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function ProfilesLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="profiles">{children}</ModuleGuard>;
}
