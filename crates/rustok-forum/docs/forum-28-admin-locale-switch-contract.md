# FORUM-28 admin locale-switch contract

This slice closes the Forum admin **dirty locale-switch policy** left open by the canonical `FORUM-28` rich-text/admin work. It does not move translation, category-hierarchy, tag-identity, route, or lifecycle ownership into another module.

## Active locale versus candidate locale

The category and topic editors now keep two separate values:

- the **active locale** owns current reads, writes, category-tree labels, topic/reply reads, and rich-text direction/spellcheck;
- the **candidate locale** is only the text typed into the locale control.

Typing a candidate must never mutate the active locale. `Load locale` is the explicit context-switch action. Save and reply submission fail closed while a different candidate is pending.

## Existing owner rows

For an open category or topic, the switch first reloads that owner detail in the current active locale and compares the editable form with the fresh persisted snapshot. Any dirty localized or owner-editable state blocks the switch; the candidate is reset to the active locale and nothing is discarded.

Only a clean form may load the same owner identity in the requested target locale. The owner API remains the source of requested/effective locale and fallback behavior.

## Missing target translation

A target read may resolve through fallback. Fallback content is read evidence, not an existing target translation. Therefore the target-form adapter must not prefill fallback localized copy as if it belonged to the requested locale:

- category `name`, `slug`, and `description` are blanked while structural icon/color/position/moderation state is preserved;
- topic `title`, `slug`, and rich-text `body` are blanked while the Forum-owned category attachment and existing tag attachment view remain available.

The normal required-field validation then requires deliberate target-language content before a missing translation can be persisted.

## Topic reply draft

A topic locale switch is blocked while the reply composer contains an unsaved reply. A reply draft is locale-owned content; silently carrying it from the previous topic locale into the target locale would be the same class of cross-locale corruption as carrying an unsaved topic body.

## Category tree alignment

On the category admin page, the category tree follows the category active locale. On the topic admin page, the same owner tree resource follows the **topic active locale** so the topic category sidebar and selector never remain in a stale category-editor locale after a topic locale switch.

## Ownership boundary

Forum continues to own category hierarchy, topic/category attachment semantics, topic translations and reply content. Shared rich-text direction/spellcheck remains driven by the active content locale. This slice changes only the Forum admin transition policy and does not alter Taxonomy Results 1–4 or create any generic hierarchy in Taxonomy.
