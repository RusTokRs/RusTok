# leptos-auth — no longer used

`leptos-auth` is no longer used and is not a Cargo package or runtime. Its former
authentication context, hooks, session, guards and transports now belong to the
Auth module's [`rustok-auth-ui`](../../modules/rustok-auth/ui/README.md) FFA
client boundary. There is no compatibility alias or parallel implementation.

This directory retains only this retirement description and pointers for readers
of older documentation. See the [Auth implementation plan](../../modules/rustok-auth/docs/implementation-plan.md)
for verification still required.
