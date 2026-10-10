# GraphQL API для Revision History

Полная спецификация GraphQL API для работы с историей ревизий контента.

## Schema Overview

```graphql
# Основные типы
type Revision
type RevisionDiff
type FieldChange
type NamedVersion
type RevisionConnection
type RevisionEdge
type PageInfo

# Input types
input CreateNamedVersionInput
input RestoreRevisionInput
input RevisionFilterInput

# Enums
enum ChangeSource
enum SortOrder
```

## Types

### Revision

```graphql
"""
Ревизия контента
"""
type Revision {
  """Уникальный идентификатор ревизии"""
  id: UUID!
  
  """ID тенанта"""
  tenantId: UUID!
  
  """Тип контента (например, 'blog_post')"""
  contentType: String!
  
  """ID контента"""
  contentId: UUID!
  
  """Язык контента"""
  locale: String!
  
  """Номер ревизии (автоинкремент)"""
  revisionNumber: Int!
  
  """ID родительской ревизии"""
  parentRevisionId: UUID
  
  """Delta с изменениями"""
  delta: JSON!
  
  """Пользователь, создавший ревизию"""
  createdBy: User!
  
  """Когда создана ревизия"""
  createdAt: DateTime!
  
  """Источник изменения"""
  changeSource: ChangeSource!
  
  """Описание изменения"""
  changeSummary: String
  
  """Имя версии (если это named version)"""
  versionName: String
  
  """Получить предыдущую ревизию"""
  parent: Revision
  
  """Получить следующую ревизию"""
  next: Revision
  
  """Сравнить с другой ревизией"""
  diffWith(revisionNumber: Int!): RevisionDiff!
  
  """Восстановить контент к этой ревизии"""
  restore: RestorePayload!
}
```

### RevisionDiff

```graphql
"""
Различия между двумя ревизиями
"""
type RevisionDiff {
  """От какой ревизии"""
  fromRevision: Revision!
  
  """К какой ревизии"""
  toRevision: Revision!
  
  """Список изменений по полям"""
  changes: [FieldChange!]!
  
  """Количество измененных полей"""
  changedFieldsCount: Int!
  
  """Есть ли изменения"""
  hasChanges: Boolean!
  
  """Человекочитаемое описание изменений"""
  humanReadable: String!
  
  """JSON представление изменений"""
  json: JSON!
}
```

### FieldChange

```graphql
"""
Изменение одного поля
"""
type FieldChange {
  """Имя поля"""
  field: String!
  
  """Старое значение"""
  oldValue: JSON
  
  """Новое значение"""
  newValue: JSON
  
  """Тип изменения"""
  changeType: ChangeType!
}

enum ChangeType {
  """Поле добавлено"""
  ADDED
  
  """Поле удалено"""
  REMOVED
  
  """Поле изменено"""
  MODIFIED
}
```

### NamedVersion

```graphql
"""
Именованная версия (snapshot)
"""
type NamedVersion {
  """Ревизия"""
  revision: Revision!
  
  """Имя версии"""
  name: String!
  
  """Описание"""
  description: String
  
  """Когда создана"""
  createdAt: DateTime!
  
  """Кто создал"""
  createdBy: User!
}
```

### Pagination Types

```graphql
"""
Пагинация для списка ревизий
"""
type RevisionConnection {
  """Список ревизий"""
  edges: [RevisionEdge!]!
  
  """Информация о пагинации"""
  pageInfo: PageInfo!
  
  """Общее количество"""
  totalCount: Int!
}

type RevisionEdge {
  """Ревизия"""
  node: Revision!
  
  """Курсор для пагинации"""
  cursor: String!
}

type PageInfo {
  """Есть ли следующая страница"""
  hasNextPage: Boolean!
  
  """Есть ли предыдущая страница"""
  hasPreviousPage: Boolean!
  
  """Курсор начала"""
  startCursor: String
  
  """Курсор конца"""
  endCursor: String
}
```

### Input Types

```graphql
"""
Input для создания именованной версии
"""
input CreateNamedVersionInput {
  """ID контента"""
  contentId: UUID!
  
  """Тип контента"""
  contentType: String!
  
  """Язык"""
  locale: String!
  
  """Имя версии"""
  name: String!
  
  """Описание"""
  description: String
}

"""
Input для восстановления ревизии
"""
input RestoreRevisionInput {
  """ID контента"""
  contentId: UUID!
  
  """Тип контента"""
  contentType: String!
  
  """Язык"""
  locale: String!
  
  """Номер ревизии для восстановления"""
  revisionNumber: Int!
  
  """Описание восстановления"""
  summary: String
}

"""
Фильтры для списка ревизий
"""
input RevisionFilterInput {
  """Фильтр по источнику изменения"""
  changeSource: ChangeSource
  
  """Фильтр по пользователю"""
  createdBy: UUID
  
  """Фильтр по дате (от)"""
  createdAfter: DateTime
  
  """Фильтр по дате (до)"""
  createdBefore: DateTime
  
  """Только именованные версии"""
  namedVersionsOnly: Boolean
  
  """Минимальный номер ревизии"""
  minRevisionNumber: Int
  
  """Максимальный номер ревизии"""
  maxRevisionNumber: Int
}
```

### Enums

```graphql
"""
Источник изменения
"""
enum ChangeSource {
  """Через админ UI"""
  ADMIN_UI
  
  """Через API"""
  API
  
  """Через импорт"""
  IMPORT
  
  """Через восстановление"""
  RESTORE
  
  """Другой источник"""
  OTHER
}

"""
Порядок сортировки
"""
enum SortOrder {
  ASC
  DESC
}
```

## Queries

### Получить ревизию по ID

```graphql
"""
Получить ревизию по ID
"""
revision(id: UUID!): Revision
```

**Пример:**
```graphql
query {
  revision(id: "550e8400-e29b-41d4-a716-446655440000") {
    id
    revisionNumber
    createdAt
    createdBy { name }
    delta
  }
}
```

### Получить список ревизий для контента

```graphql
"""
Получить список ревизий для контента
"""
revisions(
  contentId: UUID!
  contentType: String!
  locale: String!
  first: Int
  after: String
  last: Int
  before: String
  filter: RevisionFilterInput
  sortBy: RevisionSortField = REVISION_NUMBER
  sortOrder: SortOrder = DESC
): RevisionConnection!
```

**Пример:**
```graphql
query {
  revisions(
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
    first: 10
  ) {
    edges {
      node {
        revisionNumber
        createdAt
        createdBy { name }
        changeSource
        changeSummary
        versionName
      }
      cursor
    }
    pageInfo {
      hasNextPage
      endCursor
    }
    totalCount
  }
}
```

### Сравнить две ревизии

```graphql
"""
Сравнить две ревизии
"""
revisionDiff(
  contentId: UUID!
  contentType: String!
  locale: String!
  fromRevision: Int!
  toRevision: Int!
): RevisionDiff!
```

**Пример:**
```graphql
query {
  revisionDiff(
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
    fromRevision: 1
    toRevision: 5
  ) {
    fromRevision { revisionNumber }
    toRevision { revisionNumber }
    changes {
      field
      oldValue
      newValue
      changeType
    }
    changedFieldsCount
    humanReadable
  }
}
```

### Получить именованные версии

```graphql
"""
Получить список именованных версий
"""
namedVersions(
  contentId: UUID!
  contentType: String!
  locale: String!
): [NamedVersion!]!
```

**Пример:**
```graphql
query {
  namedVersions(
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
  ) {
    name
    description
    createdAt
    createdBy { name }
    revision {
      revisionNumber
    }
  }
}
```

### Получить контент на момент ревизии

```graphql
"""
Получить контент на момент определенной ревизии
"""
contentAtRevision(
  contentId: UUID!
  contentType: String!
  locale: String!
  revisionNumber: Int!
): JSON!
```

**Пример:**
```graphql
query {
  contentAtRevision(
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
    revisionNumber: 5
  )
}
```

## Mutations

### Создать именованную версию

```graphql
"""
Создать именованную версию (snapshot)
"""
createNamedVersion(input: CreateNamedVersionInput!): NamedVersion!
```

**Пример:**
```graphql
mutation {
  createNamedVersion(input: {
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
    name: "v1.0-published"
    description: "Первая опубликованная версия"
  }) {
    name
    description
    createdAt
    revision {
      revisionNumber
    }
  }
}
```

### Восстановить к ревизии

```graphql
"""
Восстановить контент к определенной ревизии
"""
restoreRevision(input: RestoreRevisionInput!): RestorePayload!
```

**Пример:**
```graphql
mutation {
  restoreRevision(input: {
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
    revisionNumber: 5
    summary: "Восстановление после случайного удаления"
  }) {
    success
    restoredContent
    newRevision {
      revisionNumber
      createdAt
    }
  }
}

type RestorePayload {
  success: Boolean!
  restoredContent: JSON!
  newRevision: Revision!
  message: String
}
```

### Удалить именованную версию

```graphql
"""
Удалить именованную версию
"""
deleteNamedVersion(revisionId: UUID!): Boolean!
```

**Пример:**
```graphql
mutation {
  deleteNamedVersion(revisionId: "550e8400-e29b-41d4-a716-446655440000")
}
```

### Очистить старые ревизии

```graphql
"""
Принудительно применить retention policy
"""
cleanupOldRevisions(
  contentId: UUID!
  contentType: String!
  locale: String!
  keepLast: Int
  keepDays: Int
): CleanupPayload!
```

**Пример:**
```graphql
mutation {
  cleanupOldRevisions(
    contentId: "550e8400-e29b-41d4-a716-446655440000"
    contentType: "blog_post"
    locale: "en"
    keepLast: 50
  ) {
    deletedCount
    remainingCount
  }
}

type CleanupPayload {
  deletedCount: Int!
  remainingCount: Int!
}
```

## Subscriptions (опционально)

```graphql
"""
Подписка на новые ревизии
"""
subscription {
  revisionCreated(
    contentId: UUID
    contentType: String
  ): Revision!
}
```

**Пример:**
```graphql
subscription {
  revisionCreated(contentType: "blog_post") {
    revisionNumber
    createdAt
    createdBy { name }
    changeSummary
  }
}
```

## Примеры использования

### 1. Получить историю поста с пагинацией

```graphql
query GetPostHistory($postId: UUID!, $cursor: String) {
  revisions(
    contentId: $postId
    contentType: "blog_post"
    locale: "en"
    first: 20
    after: $cursor
  ) {
    edges {
      node {
        id
        revisionNumber
        createdAt
        createdBy {
          id
          name
          avatar
        }
        changeSource
        changeSummary
        versionName
        delta
      }
      cursor
    }
    pageInfo {
      hasNextPage
      endCursor
    }
    totalCount
  }
}
```

### 2. Сравнить текущую версию с предыдущей

```graphql
query CompareWithPrevious($postId: UUID!) {
  post(id: $postId) {
    title
    revisions(first: 2) {
      edges {
        node {
          revisionNumber
          diffWith(revisionNumber: 1) {
            changes {
              field
              oldValue
              newValue
            }
            humanReadable
          }
        }
      }
    }
  }
}
```

### 3. Timeline изменений

```graphql
query GetTimeline($postId: UUID!) {
  revisions(
    contentId: $postId
    contentType: "blog_post"
    locale: "en"
    first: 100
    sortBy: CREATED_AT
    sortOrder: ASC
  ) {
    edges {
      node {
        revisionNumber
        createdAt
        createdBy { name }
        changeSource
        changeSummary
        versionName
      }
    }
  }
}
```

### 4. Найти все named versions

```graphql
query GetNamedVersions($postId: UUID!) {
  namedVersions(
    contentId: $postId
    contentType: "blog_post"
    locale: "en"
  ) {
    name
    description
    createdAt
    createdBy { name }
    revision {
      revisionNumber
      delta
    }
  }
}
```

### 5. Восстановить и создать snapshot

```graphql
mutation RestoreAndSnapshot($postId: UUID!, $revisionNumber: Int!) {
  # Сначала создаем snapshot текущей версии
  snapshot: createNamedVersion(input: {
    contentId: $postId
    contentType: "blog_post"
    locale: "en"
    name: "before-restore-${revisionNumber}"
    description: "Snapshot перед восстановлением"
  }) {
    name
    revision { revisionNumber }
  }
  
  # Затем восстанавливаем
  restore: restoreRevision(input: {
    contentId: $postId
    contentType: "blog_post"
    locale: "en"
    revisionNumber: $revisionNumber
    summary: "Восстановление к ревизии ${revisionNumber}"
  }) {
    success
    newRevision { revisionNumber }
  }
}
```

## Admin UI Integration

### Revision History Panel

```jsx
function RevisionHistoryPanel({ contentId, contentType }) {
  const [cursor, setCursor] = useState(null);
  
  const { data, loading } = useQuery(GET_REVISIONS, {
    variables: {
      contentId,
      contentType,
      locale: 'en',
      first: 20,
      after: cursor,
    },
  });

  if (loading) return <Spinner />;

  return (
    <div className="revision-history">
      <h3>История изменений ({data.revisions.totalCount})</h3>
      
      {data.revisions.edges.map(({ node }) => (
        <RevisionItem
          key={node.id}
          revision={node}
          onRestore={() => handleRestore(node.revisionNumber)}
          onCompare={() => handleCompare(node.revisionNumber)}
        />
      ))}
      
      {data.revisions.pageInfo.hasNextPage && (
        <button onClick={() => setCursor(data.revisions.pageInfo.endCursor)}>
          Загрузить еще
        </button>
      )}
    </div>
  );
}
```

### Diff Viewer

```jsx
function DiffViewer({ contentId, contentType, fromRevision, toRevision }) {
  const { data } = useQuery(GET_DIFF, {
    variables: {
      contentId,
      contentType,
      locale: 'en',
      fromRevision,
      toRevision,
    },
  });

  return (
    <div className="diff-viewer">
      <h3>Изменения: v{fromRevision} → v{toRevision}</h3>
      
      {data.revisionDiff.changes.map(change => (
        <div key={change.field} className="field-change">
          <strong>{change.field}:</strong>
          <div className="old-value">{JSON.stringify(change.oldValue)}</div>
          <div className="new-value">{JSON.stringify(change.newValue)}</div>
        </div>
      ))}
      
      <pre>{data.revisionDiff.humanReadable}</pre>
    </div>
  );
}
```

## Best Practices

### 1. Пагинация

Всегда используйте пагинацию для больших списков:

```graphql
# Хорошо
revisions(first: 20, after: $cursor)

# Плохо
revisions(first: 1000)
```

### 2. Фильтры

Используйте фильтры для уменьшения количества данных:

```graphql
revisions(
  filter: {
    createdAfter: "2024-01-01T00:00:00Z"
    namedVersionsOnly: true
  }
)
```

### 3. Named Versions

Создавайте named versions для важных моментов:

```graphql
# Перед публикацией
createNamedVersion(name: "before-publication")

# Перед массовым обновлением
createNamedVersion(name: "before-bulk-update")

# Перед восстановлением
createNamedVersion(name: "before-restore")
```

### 4. Error Handling

Всегда обрабатывайте ошибки:

```graphql
mutation {
  restoreRevision(input: { ... }) {
    success
    message
    newRevision { ... }
  }
}
```

## Performance Considerations

### Индексы

Убедитесь что созданы индексы:

```sql
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale);

CREATE INDEX idx_revisions_created_at 
  ON content_revisions(created_at);

CREATE INDEX idx_revisions_version_name 
  ON content_revisions(version_name);
```

### Кэширование

Кэшируйте частые запросы:

```javascript
const cache = new Map();

async function getRevisions(contentId) {
  const key = `revisions:${contentId}`;
  if (cache.has(key)) return cache.get(key);
  
  const result = await graphql.query(GET_REVISIONS, { contentId });
  cache.set(key, result);
  
  return result;
}
```

### Batch Loading

Используйте DataLoader для batch загрузки:

```javascript
const revisionLoader = new DataLoader(async (contentIds) => {
  const results = await Promise.all(
    contentIds.map(id => getRevisions(id))
  );
  return results;
});
```

## Security

### Authorization

Проверяйте права доступа:

```rust
async fn revisions(
    &self,
    ctx: &Context<'_>,
    content_id: Uuid,
) -> Result<Vec<Revision>> {
    let user = ctx.data::<User>()?;
    
    // Проверить что пользователь имеет доступ к контенту
    if !user.can_view_content(content_id) {
        return Err(Error::Unauthorized);
    }
    
    // ...
}
```

### Rate Limiting

Ограничьте количество запросов:

```rust
#[rate_limit(limit = 100, duration = 60)]
async fn revisions(...) -> Result<Vec<Revision>> {
    // ...
}
```

## Заключение

Этот GraphQL API предоставляет полный набор инструментов для работы с историей ревизий:

✅ Просмотр истории  
✅ Сравнение версий  
✅ Восстановление  
✅ Named versions  
✅ Пагинация и фильтрация  
✅ Real-time updates (subscriptions)  

API готов к использованию в production!
