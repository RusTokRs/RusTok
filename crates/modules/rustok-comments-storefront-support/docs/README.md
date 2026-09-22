# Comments Storefront Support

This support crate owns the reusable Leptos public comment composer. It keeps editor state, client validation, authentication visibility, busy state, and accessible submission feedback independent from any commentable target.

Consumer modules provide an exact owner-bound `Action<RichTextDocument, Result<(), String>>`. They do not pass `target_type` or arbitrary target identity through this component. Blog is the first consumer and binds the action to one published post in the current tenant and channel.

The live roadmap and FFA/FBA status remain in the [Comments implementation plan](../../rustok-comments/docs/implementation-plan.md). See the [crate README](../README.md) for entry points.

