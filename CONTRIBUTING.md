# Contributing to RusToK

Thank you for your interest in contributing to RusToK! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Contributing Guidelines](#contributing-guidelines)
- [Project Structure](#project-structure)
- [Testing](#testing)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)

## Code of Conduct

By participating in this project, you are expected to uphold our Code of Conduct. Please read it before contributing.

## Getting Started

### Prerequisites

- **Rust**: Version 1.80 or higher
- **PostgreSQL**: Version 16 or higher
- **Docker & Docker Compose**: For local development
- **Node.js**: Version 18+ (for admin/storefront)
- **Git**: For version control

### Recommended Tools

- **Rust Analyzer**: VS Code extension
- **Trunk**: For Leptos WASM builds
- **Loco CLI**: `cargo install loco-cli`

## Development Setup

### 1. Clone the Repository

```bash
git clone https://github.com/RustokCMS/RusToK.git
cd RusToK
```

### 2. Quick Start

The fastest way to get everything running:

```bash
# Start all services (server, admin panels, storefronts)
make dev-start

# Or use the direct script
./scripts/dev-start.sh start
```

This will start:
- PostgreSQL database
- RusToK server (http://localhost:5150)
- Next.js Admin (http://localhost:3000)
- Leptos Admin (http://localhost:3001)
- Next.js Storefront (http://localhost:3100)
- Leptos Storefront (http://localhost:3101)

### 3. Manual Setup

If you prefer manual setup:

```bash
# 1. Copy environment configuration
cp .env.dev.example .env.dev

# 2. Start database
docker-compose up -d db

# 3. Install dependencies
cd apps/server && cargo loco db install

# 4. Run migrations
cd apps/server && cargo loco db migrate

# 5. Start server
cargo loco start

# 6. In separate terminals:
cd apps/admin && npm install && npm run dev
cd apps/storefront && npm install && npm run dev
cd apps/next-frontend && npm install && npm run dev
```

### 4. Default Login Credentials

After the first startup, you can log in using:
- **Email**: admin@local
- **Password**: admin12345

## Contributing Guidelines

### Branch Naming

Use descriptive branch names:
- `feature/short-description`
- `fix/issue-description`
- `docs/update-readme`
- `refactor/component-name`

### Commit Messages

Follow conventional commits:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation updates
- `refactor:` - Code refactoring
- `test:` - Adding tests
- `chore:` - Maintenance tasks

Examples:
```
feat: add product listing to storefront
fix: resolve user authentication issue
docs: update API documentation
```

### Code Style

We use Rust's standard formatting:

```bash
# Format code
cargo fmt --all

# Lint code
cargo clippy --workspace -- -D warnings
```

### Documentation

Update documentation when:
- Adding new features
- Changing APIs
- Modifying configuration
- Updating dependencies
- Changing architecture, events, modules, tenancy, routing, or UI contracts (also update [`docs/index.md`](docs/index.md) and [`docs/modules/registry.md`](docs/modules/registry.md))

## Project Structure

### Apps

- **apps/server**: Main backend API (Loco.rs)
- **apps/admin**: Leptos CSR admin panel
- **apps/storefront**: Leptos SSR storefront
- **apps/next-frontend**: Next.js storefront
- **crates/rustok-mcp**: MCP adapter crate (includes stdio server binary `rustok-mcp-server`)

### Crates (Modules)

Core modules in `crates/`:
- **rustok-core**: Platform foundation (auth, events, RBAC)
- **rustok-commerce**: E-commerce functionality
- **rustok-content**: CMS core (nodes, categories)
- **rustok-blog**: Blogging features
- **rustok-index**: CQRS read models
- **rustok-telemetry**: Observability and logging

### Development Workflow

1. **Module-First Development**: Most features are implemented as modules
2. **Event-Driven Architecture**: Modules communicate via events
3. **CQRS-lite**: Write models in modules, read models in rustok-index
4. **Tenant Isolation**: All features respect multi-tenant boundaries

## Testing

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p rustok-core

# With database
DATABASE_URL=postgres://localhost/rustok_test cargo test

# Coverage
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --out html
```

### Testing Guidelines

1. **Unit Tests**: Test individual functions and methods
2. **Integration Tests**: Test module interactions
3. **Property Tests**: Use proptest for complex logic
4. **E2E Tests**: Test complete user flows
5. **Event-flow changes require integration coverage**:
   - Changes to `DomainEvent` producers/consumers or handler routing are not accepted without integration tests for the full chain `event created -> handler executed -> projection/index updated`.
   - Integration test naming must describe the chain explicitly, for example: `test_product_created_event_updates_index_projection`.
   - Every newly introduced `DomainEvent` must include at least:
     - one happy-path integration test;
     - one repeat/idempotency integration test.
   - **Legacy event-flow transition policy**: existing legacy flows may be covered incrementally, but any PR that touches legacy producer/consumer logic, routing, outbox/delivery, or projection/index updates must add or update the corresponding integration tests in the same PR.

See `docs/testing-guidelines.md` for detailed testing strategies.

## Documentation

### Where to Document

- **API Changes**: Update CHANGELOG.md
- **Architecture**: Update relevant docs in `docs/`
- **Module APIs**: Document in module README files
- **User Guides**: Update QUICKSTART.md or create new guides

### Documentation Standards

- Use clear, concise language
- Include code examples
- Add diagrams for complex concepts
- Keep documentation up-to-date with code

### Building Documentation

```bash
# Generate API documentation
cargo doc --workspace --no-deps

# Build book documentation (if applicable)
cd docs && mdbook build
```

## Pull Request Process

### Before Submitting

1. **Run Tests**: Ensure all tests pass
2. **Format Code**: Use `cargo fmt --all`
3. **Lint Code**: Use `cargo clippy --workspace`
4. **Update Documentation**: Include relevant docs updates
5. **Update Changelog**: Add entry to CHANGELOG.md

### PR Checklist

- [ ] Code follows project style guidelines
- [ ] Tests added/updated for new features
- [ ] Event-flow changes include integration tests for `event -> handler -> projection/index`
- [ ] Every new `DomainEvent` has happy-path + idempotency/repeat integration tests
- [ ] Documentation updated
- [ ] Review: «Есть ли новый код в app-слое, который должен жить в библиотеке?»
- [ ] Review: «Есть ли изменения в docs модуля и `docs/index.md` при изменении контракта?»
- [ ] For critical domains (content/commerce/blog/forum/pages/index/rbac/tenant), any intentional bypass of module-first DoD has explicit architecture approval
- [ ] DOCS_MAP.md updated when docs triggers were touched
- [ ] CHANGELOG.md updated
- [ ] Framework deviation checklist completed for every new framework/runtime deviation (benchmark evidence, failure-mode table, rollback strategy, owner sign-off); see `docs/standards/forbidden-actions.md#64-framework-deviation-checklist-обязателен-для-каждого-нового-отклонения`
- [ ] All CI checks pass
- [ ] No breaking changes (or properly documented)

### Review Process

1. **Automated Checks**: CI must pass
2. **Code Review**: At least one maintainer review
3. **Testing**: Manual testing may be required
4. **Documentation**: Review for completeness
5. **Merge**: Squash and merge after approval

## Areas for Contribution

### High Priority

- **Testing**: Integration tests and E2E tests
- **Documentation**: API docs and guides
- **Bug Fixes**: Issues in GitHub tracker
- **Performance**: Optimizations and benchmarks

### Medium Priority

- **Features**: New modules and functionality
- **UI/UX**: Admin panel and storefront improvements
- **DevOps**: CI/CD improvements
- **Security**: Security audits and fixes

### Good First Issues

Look for issues labeled:
- `good first issue`
- `help wanted`
- `documentation`

## Getting Help

- **Documentation**: Check `docs/` directory
- **Issues**: Search GitHub issues
- **Discussions**: Use GitHub Discussions
- **Chat**: Join our community channels

## Release Process

1. **Version Bump**: Update version in Cargo.toml
2. **CHANGELOG**: Finalize changelog
3. **Testing**: Run full test suite
4. **Tag**: Create Git tag
5. **Release**: GitHub release with notes

## Security

### Reporting Security Issues

Please report security vulnerabilities to: security@rustok-cms.com

### Security Guidelines

- Never commit secrets or API keys
- Use environment variables for configuration
- Follow OWASP security best practices
- Regular dependency audits: `cargo audit`

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to RusToK! 🚀

For questions about contributing, please open an issue or discussion.
