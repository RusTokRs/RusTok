# Code Review Report — rustok-revisions

**Дата:** 2026-10-09  
**Статус:** ❌ Найдены критические ошибки

## Критические проблемы

### 1. **apply_retention_policy не работает** 🔴

**Файл:** `rustok-revisions/src/service.rs:116-147`

**Проблема:**
```rust
pub async fn apply_retention_policy(
    &self,
    tenant_id: Uuid,
    content_id: Uuid,
    locale: &str,
    policy: &RetentionPolicy,
) -> Result<usize, RevisionError> {
    match policy {
        RetentionPolicy::KeepAll => Ok(0),
        
        RetentionPolicy::KeepLast(n) => {
            let current = self
                .backend
                .get_next_revision_number(tenant_id, "", content_id, locale)  // ❌ Пустой content_type!
                .await?;
            
            // ...
            Ok(0) // TODO: Implement properly  // ❌ Всегда возвращает 0
        }
        
        RetentionPolicy::KeepDays(days) => {
            // ...
            Ok(0) // TODO: Implement properly  // ❌ Всегда возвращает 0
        }
    }
}
```

**Почему это критично:**
- Метод вызывается в `create_revision_with_tracker` (строка 108)
- Всегда возвращает `Ok(0)` — retention policy не применяется
- Передает пустой `content_type` в backend, что может вызвать ошибки

**Решение:**
Удалить этот метод или исправить. Использовать только `apply_retention_policy_for_type`.

---

### 2. **Тесты не скомпилируются** 🔴

**Файл:** `rustok-revisions/src/backend.rs:233-267`

**Проблема:**
```rust
let revision = Revision {
    id: Uuid::new_v4(),
    tenant_id,
    content_type: "blog_post".to_string(),
    // ...
    change_summary: None,
    // ❌ Отсутствует поле version_name!
};
```

**Почему это критично:**
- Я добавил поле `version_name: Option<String>` в структуру `Revision`
- Но не обновил тесты в backend.rs
- Код не скомпилируется

**Решение:**
Добавить `version_name: None` во все тестовые Revision structs.

---

### 3. **Неправильный assertion в тесте** 🟡

**Файл:** `rustok-revisions/src/service.rs:588`

**Проблема:**
```rust
assert_eq!(revision.delta["title"], "New Title");
```

**Почему это неправильно:**
- В v0.2.0 delta формат изменился на `{"title": {"old": ..., "new": ...}}`
- Assertion проверяет старое значение вместо нового формата
- Тест упадет

**Правильно:**
```rust
assert_eq!(revision.delta["title"]["new"], "New Title");
assert_eq!(revision.delta["title"]["old"], "Old Title");
```

---

### 4. **create_revision_with_tracker вызывает apply_retention_policy без content_type** 🔴

**Файл:** `rustok-revisions/src/service.rs:95-113`

**Проблема:**
```rust
pub async fn create_revision_with_tracker<T: Revisionable>(
    &self,
    // ...
    tracker: &RevisionTracker<T>,
    event: RevisionEvent,
) -> Result<Option<Revision>, RevisionError> {
    // ...
    
    if revision.is_some() {
        self.apply_retention_policy(tenant_id, content_id, locale, &tracker.retention)
            .await?;  // ❌ Вызывает метод без content_type
    }
    
    Ok(revision)
}
```

**Почему это критично:**
- Должен вызывать `apply_retention_policy_for_type::<T>`
- Иначе retention policy не работает

**Решение:**
```rust
if revision.is_some() {
    self.apply_retention_policy_for_type::<T>(
        tenant_id,
        content_id,
        locale,
        &tracker.retention,
    ).await?;
}
```

---

## Средние проблемы

### 5. **get_content_at_revision может вернуть неправильный результат** 🟡

**Файл:** `rustok-revisions/src/service.rs:213-247`

**Проблема:**
```rust
let current_revision = self
    .backend
    .get_next_revision_number(tenant_id, T::content_type(), content_id, locale)
    .await?
    - 1;

if target_revision > current_revision {
    return Err(...);
}
```

**Почему это проблема:**
- Если `target_revision == current_revision`, метод вернет текущий content без изменений
- Это не ошибка, но может быть неожиданно

**Решение:**
Добавить проверку и вернуть early:
```rust
if target_revision == current_revision {
    return Ok(current_content.clone());
}
```

---

### 6. **Integration crate зависит от rustok-core, который может не существовать** 🟡

**Файл:** `crates/integration/rustok-content-revisions/Cargo.toml:11`

**Проблема:**
```toml
rustok-core = { path = "../../libs/rustok-core" }
```

**Почему это проблема:**
- Я не проверил существует ли `rustok-core`
- Если нет — compilation error

**Решение:**
Проверить структуру RusTok и исправить путь или убрать зависимость.

---

### 7. **Derive macro может не работать корректно** 🟡

**Файл:** `rustok-revisions-derive/src/lib.rs`

**Проблемы:**
1. Парсинг атрибутов может быть ненадежным (строковый парсинг вместо proper AST)
2. Нет валидации что все tracked fields существуют
3. Нет обработки enum fields

**Решение:**
- Использовать `syn` для proper AST parsing
- Добавить валидацию
- Добавить больше тестов

---

## Мелкие проблемы

### 8. **Отсутствует документация для публичных API** 🟢

**Проблема:**
Многие публичные методы не имеют doc comments.

**Решение:**
Добавить документацию.

---

### 9. **Нет error handling для concurrent access** 🟢

**Проблема:**
В InMemoryBackend используется `Mutex`, но нет retry logic для lock poisoning.

**Решение:**
Добавить proper error handling или использовать `RwLock`.

---

### 10. **Миграция может конфликтовать с существующими таблицами** 🟢

**Проблема:**
Миграция использует `if_not_exists()`, но не проверяет совместимость схемы.

**Решение:**
Добавить проверку существующей схемы.

---

## Архитектурные проблемы

### 11. **Library находится вне RusTok workspace** 🟡

**Проблема:**
```
/home/user/rustok-revisions/          # Library
/home/user/RusTok/                     # Platform
```

**Почему это проблема:**
- Library должна быть в `crates/libs/rustok-revisions/`
- Или оставаться standalone, но тогда integration crate не должен использовать `path` dependency

**Решение:**
Определиться с архитектурой:
- Option A: Переместить library в RusTok
- Option B: Сделать library external dependency (publish to crates.io)

---

### 12. **Integration crate дублирует функционал library** 🟡

**Проблема:**
`ContentRevisionService` в integration crate почти полностью дублирует `RevisionService` из library.

**Почему это проблема:**
- Code duplication
- Harder to maintain
- Confusion about which to use

**Решение:**
Integration crate должен быть thin wrapper, не дублировать логику:
```rust
pub struct ContentRevisionService {
    inner: RevisionService,
    config: ContentRevisionConfig,
}

impl ContentRevisionService {
    pub async fn track_update<T: Revisionable>(...) {
        // Add platform-specific logic
        // Delegate to inner service
        self.inner.create_revision(...).await
    }
}
```

---

## Рекомендации по исправлению

### Приоритет 1: Критические ошибки

1. ✅ **Исправить apply_retention_policy**
   - Удалить метод или исправить
   - Использовать только `apply_retention_policy_for_type`

2. ✅ **Добавить version_name в тесты**
   - Обновить все тестовые Revision structs

3. ✅ **Исправить assertion в тесте**
   - Обновить на новый delta формат

4. ✅ **Исправить create_revision_with_tracker**
   - Вызывать `apply_retention_policy_for_type`

### Приоритет 2: Средние проблемы

5. ✅ **Добавить early return в get_content_at_revision**
6. ✅ **Проверить зависимость rustok-core**
7. ✅ **Улучшить derive macro**

### Приоритет 3: Мелкие проблемы

8. ✅ **Добавить документацию**
9. ✅ **Улучшить error handling**
10. ✅ **Проверить миграции**

### Приоритет 4: Архитектура

11. ✅ **Определиться с расположением library**
12. ✅ **Убрать duplication в integration crate**

---

## Тестирование

### Текущее состояние

- ✅ Unit tests для backend
- ✅ Unit tests для service (но некоторые сломаются)
- ❌ Integration tests отсутствуют
- ❌ End-to-end tests отсутствуют

### Что нужно добавить

1. **Integration tests для SeaORM backend**
   ```rust
   #[tokio::test]
   async fn test_seaorm_backend_with_real_db() {
       let db = Database::connect("postgres://localhost/test").await?;
       // Run migrations
       // Test CRUD operations
   }
   ```

2. **End-to-end tests для integration crate**
   ```rust
   #[tokio::test]
   async fn test_blog_revision_tracking() {
       let service = ContentRevisionService::new(db, config);
       // Test full workflow
   }
   ```

3. **Performance tests**
   - Test with 1000+ revisions
   - Test retention policy performance

---

## Security Review

### Найденные проблемы

1. ✅ **SQL Injection** — SeaORM защищает от этого
2. ✅ **Tenant isolation** — Проверяется во всех методах
3. ⚠️ **No input validation** — Нет валидации version_name length
4. ⚠️ **No rate limiting** — Можно создать infinite revisions

### Рекомендации

1. Добавить валидацию:
   ```rust
   pub async fn create_named_version(
       &self,
       // ...
       version_name: &str,
   ) -> Result<Revision, RevisionError> {
       if version_name.len() > 100 {
           return Err(RevisionError::InvalidInput("Version name too long".to_string()));
       }
       // ...
   }
   ```

2. Добавить rate limiting в integration crate

---

## Performance Review

### Найденные проблемы

1. ⚠️ **get_content_at_revision** — O(n) где n = количество revisions
2. ⚠️ **list_revisions** — Загружает все revisions в память
3. ✅ **Индексы** — Правильно настроены в миграции

### Рекомендации

1. Добавить pagination:
   ```rust
   pub async fn list_revisions_paginated(
       &self,
       // ...
       limit: usize,
       offset: usize,
   ) -> Result<Vec<Revision>, RevisionError>;
   ```

2. Кэшировать часто запрашиваемые revisions

---

## Conclusion

**Найдено критических ошибок:** 4  
**Найдено средних проблем:** 3  
**Найдено мелких проблем:** 3  
**Найдено архитектурных проблем:** 2

**Статус:** ❌ Код требует исправлений перед использованием в production

**Следующие шаги:**
1. Исправить критические ошибки
2. Добавить missing tests
3. Протестировать с реальным database
4. Исправить архитектурные проблемы

**Оценка готовности:** 60% (после исправления критических ошибок — 85%)
