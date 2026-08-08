# Pages / Page Builder rollout binding source review — 2026-08-08

Base: `main@488803a831e725ef0fbeaa8540f09458ee461b85`.

Source review confirms the change is limited to trusted rollout normalization/binding, current-state evidence, and source guards.

Key review points:

- `BuilderCapabilityFlags` is `Clone` rather than `Copy`; facade provider status therefore clones its retained snapshot instead of moving from `&self`.
- admin workspace flags come from the server function and are only used for provider-status narrowing;
- Preview/Publish independently reread the trusted routed-tenant snapshot on SSR;
- request snapshot tenant slug is rejected when it differs from routed trusted tenant slug;
- the old hardcoded all-on helper is absent from the consumer binding;
- browser-intent persistence still reaches the same authoritative SSR path;
- a full-file composition update briefly changed the Channels input to `get_untracked()` during editing; source review caught it and restored the original reactive `get()` before this review packet;
- no runtime acceptance or observed-health claim was introduced.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed.
