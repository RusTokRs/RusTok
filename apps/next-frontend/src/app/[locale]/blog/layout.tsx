import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function BlogLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="blog">{children}</ModuleGuard>;
}
