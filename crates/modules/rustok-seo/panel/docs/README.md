# rustok-seo-admin-support documentation

This support crate provides reusable owner-side SEO panels and widgets for
pages, products, blog, and forum. It does not own entity screens, SEO runtime
storage, a central SEO route, or a package-local locale chain.

Entity metadata operations remain shared GraphQL helpers here. SEO
control-plane REST parity remains with SEO admin/Next owners. The active
boundary, coverage, and widget acceptance work is in the
[implementation plan](./implementation-plan.md).
