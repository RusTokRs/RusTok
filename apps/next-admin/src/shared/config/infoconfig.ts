import type { InfobarContent } from '@/shared/ui/shadcn/infobar';

export const workspacesInfoContent: InfobarContent = {
  title: 'Tenant workspace context',
  sections: [
    {
      title: 'Runtime ownership',
      description:
        'RusTok resolves the active workspace through the authenticated admin session and tenant headers. Workspace switching is a platform concern, not a starter organization-management screen.',
      links: []
    }
  ]
};

export const teamInfoContent: InfobarContent = {
  title: 'Team and access control',
  sections: [
    {
      title: 'Operator access',
      description:
        'User, role and permission screens must use RusTok RBAC contracts and tenant-scoped backend reads. External identity-provider examples should not appear as admin UI copy.',
      links: []
    }
  ]
};

export const billingInfoContent: InfobarContent = {
  title: 'Billing surface disabled',
  sections: [
    {
      title: 'Not part of current admin contract',
      description:
        'Billing is not exposed as a RusTok admin route in the current debug stack. When billing becomes a module or platform surface, it must register through a package-owned entrypoint.',
      links: []
    }
  ]
};

export const productInfoContent: InfobarContent = {
  title: 'Product catalog module',
  sections: [
    {
      title: 'Module-owned UI',
      description:
        'Product catalog navigation and data access are owned by the product admin package. The Next host mounts that surface and keeps write-side behavior behind RusTok module contracts.',
      links: []
    }
  ]
};
