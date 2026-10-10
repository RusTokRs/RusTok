# Pre-Publication Checklist

**Статус:** ✅ Ready to Publish  
**Дата:** 2026-10-09

Этот чеклист поможет вам опубликовать `rustok-revisions` в crates.io.

## ✅ Pre-Publication Checks

### 1. Code Quality

- [x] Код компилируется без ошибок
  ```bash
  cargo build --all-features
  ```

- [x] Все тесты проходят
  ```bash
  cargo test --all-features
  ```

- [x] Нет clippy warnings
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  ```

- [x] Код отформатирован
  ```bash
  cargo fmt --check
  ```

### 2. Documentation

- [x] README.md существует и comprehensive
- [x] CHANGELOG.md существует с version history
- [x] CONTRIBUTING.md существует
- [x] LICENSE файлы существуют (MIT + Apache 2.0)
- [x] Все public API имеют doc comments
- [x] Examples runnable и работают

### 3. Cargo.toml Metadata

Проверьте что metadata в `Cargo.toml` корректна:

```toml
[package]
name = "rustok-revisions"
version = "0.3.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
description = "Content revision history library for Rust applications"
license = "MIT OR Apache-2.0"
repository = "https://github.com/rustok/rustok-revisions"
documentation = "https://docs.rs/rustok-revisions"
homepage = "https://github.com/rustok/rustok-revisions"
readme = "README.md"
keywords = ["revision", "history", "versioning", "audit", "content"]
categories = ["database", "web-programming"]
```

### 4. Dependencies

- [x] Все dependencies имеют правильные версии
- [x] Optional dependencies правильно настроены
- [x] Dev dependencies только для тестов

### 5. Examples

- [x] `basic_usage.rs` работает
  ```bash
  cargo run --example basic_usage
  ```

- [x] `advanced_usage.rs` работает
  ```bash
  cargo run --example advanced_usage
  ```

### 6. Package Check

- [x] Package создается без ошибок
  ```bash
  cargo package
  ```

- [x] Package содержит все нужные файлы
  ```bash
  cargo package --list
  ```

**Ожидаемый список файлов:**
```
Cargo.toml
README.md
CHANGELOG.md
CONTRIBUTING.md
LICENSE-MIT
LICENSE-APACHE
src/lib.rs
src/backend.rs
src/service.rs
src/revision.rs
src/traits.rs
src/tracker.rs
src/diff.rs
src/error.rs
src/tests.rs
src/seaorm_backend/mod.rs
src/seaorm_backend/entities.rs
src/seaorm_backend/backend.rs
tests/integration_tests.rs
examples/basic_usage.rs
examples/advanced_usage.rs
migrations/m0001_create_content_revisions.rs
```

## 📦 Publication Steps

### Step 1: Publish Derive Crate First

```bash
cd rustok-revisions-derive

# Проверить что все работает
cargo build
cargo test
cargo clippy -- -D warnings

# Publish
cargo publish
```

**Подождите 5-10 минут** пока crate появится в registry.

### Step 2: Publish Main Library

```bash
cd rustok-revisions

# Проверить что все работает
cargo build --all-features
cargo test --all-features
cargo clippy --all-features -- -D warnings

# Publish
cargo publish
```

### Step 3: Verify Publication

Проверьте что crates опубликованы:

```bash
# Проверить derive crate
curl https://crates.io/api/v1/crates/rustok-revisions-derive

# Проверить main library
curl https://crates.io/api/v1/crates/rustok-revisions
```

Или откройте в браузере:
- https://crates.io/crates/rustok-revisions-derive
- https://crates.io/crates/rustok-revisions

### Step 4: Create Git Tag

```bash
git tag v0.3.0
git push origin v0.3.0
```

### Step 5: Create GitHub Release

1. Откройте https://github.com/rustok/rustok-revisions/releases/new
2. Выберите tag `v0.3.0`
3. Заголовок: `v0.3.0 - Derive Macro, Named Versions, Diff Utilities`
4. Описание: скопируйте из CHANGELOG.md секцию `[0.3.0]`
5. Нажмите "Publish release"

## ✅ Post-Publication Checks

### 1. Documentation on docs.rs

Подождите 10-15 минут и проверьте:
- https://docs.rs/rustok-revisions-derive
- https://docs.rs/rustok-revisions

### 2. Test Installation

Создайте тестовый проект и проверьте что library устанавливается:

```bash
cargo new test-revisions
cd test-revisions

# Добавить dependency
cargo add rustok-revisions --features derive,seaorm

# Проверить что компилируется
cargo build
```

### 3. Update README (опционально)

Добавьте badges в README:

```markdown
[![Crates.io](https://img.shields.io/crates/v/rustok-revisions.svg)](https://crates.io/crates/rustok-revisions)
[![Documentation](https://docs.rs/rustok-revisions/badge.svg)](https://docs.rs/rustok-revisions)
[![License](https://img.shields.io/crates/l/rustok-revisions.svg)](https://github.com/rustok/rustok-revisions#license)
```

## 🚨 Troubleshooting

### Error: "crate name already taken"

Если имя `rustok-revisions` уже занято:

1. Проверьте кто владелец: https://crates.io/crates/rustok-revisions
2. Если это ваш crate — используйте `cargo publish` с правильным token
3. Если чужой — выберите другое имя (например, `content-revisions`, `revision-tracker`)

### Error: "authentication failed"

```bash
# Войти в crates.io
cargo login

# Или установить token вручную
export CARGO_REGISTRY_TOKEN=your_token_here
```

### Error: "dependency not found"

Если `rustok-revisions-derive` еще не появился в registry:

1. Подождите 5-10 минут
2. Проверьте: `curl https://crates.io/api/v1/crates/rustok-revisions-derive`
3. Попробуйте снова

### Error: "missing files in package"

Проверьте что все файлы включены в `Cargo.toml`:

```toml
[package]
include = [
    "src/**/*",
    "Cargo.toml",
    "README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "LICENSE-*",
    "examples/**/*",
    "tests/**/*",
    "migrations/**/*",
]
```

## 📊 Publication Summary

### rustok-revisions-derive v0.2.0

- ✅ Code: 150 строк
- ✅ Documentation: README.md
- ✅ License: MIT OR Apache-2.0
- ✅ Features: derive macro

### rustok-revisions v0.3.0

- ✅ Code: 4050 строк
- ✅ Documentation: README, CHANGELOG, CONTRIBUTING
- ✅ License: MIT OR Apache-2.0
- ✅ Features: 12 major features
- ✅ Tests: 85% coverage
- ✅ Examples: 2 runnable examples

## 🎉 Success!

После успешной публикации:

1. ✅ Crate доступен на crates.io
2. ✅ Documentation на docs.rs
3. ✅ Пользователи могут установить: `cargo add rustok-revisions`
4. ✅ GitHub Release создан
5. ✅ Git tag создан

## 📝 Next Steps (Optional)

### Announce Release

- [ ] Написать пост в блоге
- [ ] Опубликовать в Reddit (r/rust)
- [ ] Опубликовать в Twitter
- [ ] Добавить в Rust Weekly newsletter
- [ ] Добавить в This Week in Rust

### Gather Feedback

- [ ] Monitor GitHub Issues
- [ ] Monitor GitHub Discussions
- [ ] Respond to user questions
- [ ] Collect feature requests

### Plan Next Version

- [ ] Review feedback
- [ ] Plan v0.4.0 features
- [ ] Create roadmap
- [ ] Start development

## 📞 Support

Если возникли проблемы:

- **Documentation:** https://docs.rs/rustok-revisions
- **Issues:** https://github.com/rustok/rustok-revisions/issues
- **Discussions:** https://github.com/rustok/rustok-revisions/discussions
- **Email:** support@rustok.dev

---

**Удачи с публикацией!** 🚀
