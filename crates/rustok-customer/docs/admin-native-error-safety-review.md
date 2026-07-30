# Customer admin native error-safety source review

Reviewed against `main` commit `36739625d6c244b8d96112591981550138aac667`.

The branch is not behind `main`. The intervening repository change is Index-owned and does not overlap Customer paths.

Source review confirms:

- all five mounted customer-admin native endpoints remain present;
- permissions, pagination, UUID validation, locale fallback, profile audience selection, DTOs, and customer/profile bridge behavior remain unchanged;
- direct framework and typed customer error conversion through `.map_err(ServerFnError::new)` is absent from the adapter;
- technical profile/storage causes are private diagnostics;
- validation, not-found, duplicate-email, and duplicate-user-link outcomes retain bounded public meaning;
- no customer email, name, phone, search text, profile payload, or request body was added to diagnostics.

Tests, verifiers, Cargo, formatting, workflows, CI, and runtime traces were not executed. The source review does not promote FFA or FBA status.
