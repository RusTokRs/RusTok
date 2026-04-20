# Ownership SEO UI по content-модулям

- Date: 2026-04-19
- Status: Accepted

## Context

`rustok-seo` уже собран как единый tenant-aware runtime для metadata precedence, canonical routing,
redirects, sitemap/robots и storefront `SeoPageContext`. При этом в текущем состоянии
`rustok-seo-admin` содержит central metadata editor для `page`, `product` и `blog_post`.

Такой central editor конфликтует с базовым module-owned UI contract платформы:

- экраном сущности должен владеть её owner-модуль;
- host только монтирует surface;
- cross-cutting capability не должна становиться владельцем чужого domain UI;
- новые модули должны иметь возможность интегрировать SEO в свой собственный UI, а не идти
  через отдельный universal hub.

Это особенно важно для `pages`, `product`, `blog`, `forum` и любых будущих content-domain модулей,
где SEO controls, diagnostics и completion scoring должны жить рядом с основным editor UI сущности.

## Decision

Фиксируем следующий ownership contract:

1. `rustok-seo` остаётся единственным backend/runtime owner для SEO capability:
   metadata storage, precedence, canonical routing, redirects, sitemap/robots, diagnostics,
   storefront/headless read contracts.
2. Entity-specific SEO authoring UI принадлежит owner-модулям:
   `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`
   и любым будущим content-модулям.
3. `rustok-seo-admin` сохраняет только SEO-owned infrastructure UI:
   redirects, robots policy, sitemap generation/status, global defaults, shared diagnostics overview
   и похожие cross-cutting controls.
4. `rustok-seo` или support-слой рядом с ним может поставлять shared SEO widgets/helpers для
   owner-модулей, но эти widgets не делают `rustok-seo-admin` владельцем domain screen.
5. Текущий central metadata editor в `rustok-seo-admin` считается transitional surface и должен
   быть удалён после cutover entity SEO authoring в owner-модули.

## Consequences

- Module-owned UI contract становится согласованным с SEO capability model:
  где доменная сущность, там и её SEO editor.
- `rustok-seo-admin` сужается до настоящего cross-cutting control plane вместо universal editor.
- Для `pages`, `product`, `blog`, `forum` потребуется owner-side integration поверх shared SEO
  contracts и reusable widgets.
- План `rustok-seo` должен фиксировать migration path: shared widgets/support layer, cutover
  metadata editor в owner-модули, затем cleanup transitional central editor.
