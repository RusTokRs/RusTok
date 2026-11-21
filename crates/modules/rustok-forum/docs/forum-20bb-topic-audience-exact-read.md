# forum-topic-audience-exact-read

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

Tests, Cargo, formatting, and CI were not run by the implementation agent; this note records source review only.

- ForumTopicAudienceReadService
- FORUM-20BB
- effective locale and route channel come from that exact context
- No existing `TopicService` method or transport call site changes
- FORUM-20BC
- canonical implementation plan
- CRATE_API.md
- did not run tests
