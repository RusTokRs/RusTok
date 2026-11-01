# utoipa-swagger-ui-vendored — Crate API

Vendored Swagger UI asset bundle for RusToK API docs surfaces.

## Public surface

- `SWAGGER_UI_VENDORED` — the pinned Swagger UI asset set as path/contents pairs.

## Invariants

- The bundle is vendored and pinned: serving the UI never requires a build step or a runtime download.
- Asset contents are generated from the vendored upstream release and must not be hand-edited.
