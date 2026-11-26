# Fly theme-token reference and inline-rendering contract

Status: Accepted
Date: 2026-11-25
Scope: `crates/ui/fly`
Extends: `DECISIONS/2026-10-14-fly-token-authoring-and-single-backlog.md`
Supersedes (partial): the token-rename rule in the earlier decision that left existing `var()` references unchanged. The earlier decision's single-backlog and panel-ownership decisions remain in force.

## Context

Fly design tokens are project-owned CSS custom properties. The editor exposes token usage
inspection and a rename command, while the renderer supports both generated style hooks and an
inline-only mode. These paths must agree on what constitutes a token reference and where token
values become available. A broad text replacement can corrupt authored content, and suppressing
all generated CSS can leave otherwise valid inline styles referencing undefined custom properties.

## Decision

1. A token name is stored without the CSS `--` prefix. Its CSS custom property is
   `--{name}`.
2. `ThemeCommand::Rename` and `find_token_usages` operate on actual CSS `var()` function
   arguments. They match the complete custom-property name, include nested fallback expressions,
   and ignore quoted strings, comments, and unrelated identifiers or custom-property text.
3. A malformed or escaped `var()` expression is not partially rewritten. Existing validation
   remains fail-closed for malformed CSS values.
4. With `RenderPolicy.emit_style_hooks=true`, theme custom properties are emitted in the
   generated root stylesheet and component style hooks are emitted as before.
5. With `RenderPolicy.emit_style_hooks=false`, generated project CSS and style identity hooks
   remain disabled. Validated theme custom properties are instead emitted on the rendered root
   element as inline custom properties, inheriting into descendants. Authored component styles,
   including embed component styles, use the inline style path.
6. Invalid theme-token definitions do not produce root custom properties. Inline declarations
   referencing unavailable tokens are filtered by the existing style admission path.

## Consequences

- Renaming a token preserves references that are semantically CSS variable references without
  rewriting literal text that merely contains the same characters.
- Nested fallbacks remain renameable and discoverable by usage inspection.
- Inline-only rendering remains self-contained for validated theme tokens without reintroducing
  generated project CSS.
- Embed components obey the same inline-style policy as ordinary components while retaining
  their provider-controlled iframe attributes and aspect-ratio declaration.

## Verification

Unit coverage belongs beside the owning contracts: theme rename and usage tests in `theme.rs`,
CSS variable parsing and inline rendering tests in `render.rs`. Host-mounted iframe and browser
verification remain owned by the Page Builder integration.
