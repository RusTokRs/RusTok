# Changelog

All notable changes to RusToK are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- _No unreleased additions yet._

### Changed
- _No unreleased changes yet._

### Fixed
- _No unreleased fixes yet._

### Deprecated
- _No unreleased deprecations yet._

### Removed
- _No unreleased removals yet._

### Security
- _No unreleased security updates yet._

---

## Release note policy

- Keep this file release-oriented: each released version gets one section
  `## [X.Y.Z] - YYYY-MM-DD` with standard subsections.
- Avoid sprint diaries and implementation logs in this file.
- For deep implementation context, link to canonical docs in `docs/` and `DECISIONS/`.
- Do not reference missing files; every link must resolve inside the repository.

## Version section template

```md
## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...

### Deprecated
- ...

### Removed
- ...

### Security
- ...
```

## Hotspot contract (DOC-12 / H5)

- Hotspot: `H5` (Release and compatibility communication).
- Doc contracts updated: `CHANGELOG.md`.
- Owner scope: platform docs owner.
- Residual drift risk:
  - root onboarding docs (`README.md`, `README.ru.md`) могут обновиться раньше,
    чем release note section в этом файле;
  - без обязательного release cutover checklist риск stale compatibility notes
    остаётся до полного закрытия DOC-12 (B14).
