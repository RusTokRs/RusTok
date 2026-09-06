import type { NavItem } from "@/types";

export const iggyConnectorNavItems: NavItem[] = [
  {
    title: "Infrastructure",
    url: "#",
    icon: "dashboard",
    isActive: false,
    items: [
      {
        title: "Iggy Connector",
        url: "/dashboard/iggy-connector",
      },
    ],
    access: { role: "admin" },
  },
];
