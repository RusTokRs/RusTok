# Admin UI для Revision History

Дизайн и спецификация пользовательского интерфейса для управления историей ревизий контента в админ-панели.

## Обзор

Admin UI предоставляет интуитивный интерфейс для:
- Просмотра истории изменений контента
- Сравнения версий
- Восстановления к предыдущим версиям
- Управления именованными версиями

## Основные экраны

### 1. Revision History Panel

**Расположение:** Боковая панель на странице редактирования контента

**Компоненты:**
- Список ревизий с пагинацией
- Фильтры (по дате, пользователю, типу изменения)
- Поиск по описанию изменений
- Кнопки действий (просмотр, сравнение, восстановление)

**Mockup:**

```
┌─────────────────────────────────────────┐
│ История изменений                       │
├─────────────────────────────────────────┤
│ 🔍 Поиск...                             │
│                                         │
│ Фильтры:                                │
│ [Все ▼] [Последние 7 дней ▼] [Все ▼]   │
│                                         │
├─────────────────────────────────────────┤
│ ● v5 - Текущая версия                   │
│   Сегодня, 14:30 • Иван Петров         │
│   Обновлен заголовок и контент         │
│   [Просмотр] [Сравнить]                │
│                                         │
│ ● v4 - Опубликована                     │
│   Вчера, 16:45 • Анна Сидорова         │
│   Пост опубликован                     │
│   [Просмотр] [Сравнить] [Восстановить] │
│                                         │
│ ● v3                                    │
│   Вчера, 15:20 • Иван Петров           │
│   Добавлен раздел "Заключение"         │
│   [Просмотр] [Сравнить] [Восстановить] │
│                                         │
│ ● v2                                    │
│   2 дня назад, 10:15 • Иван Петров     │
│   Исправлены опечатки                  │
│   [Просмотр] [Сравнить] [Восстановить] │
│                                         │
│ ● v1 - Черновик создан                  │
│   3 дня назад, 09:00 • Иван Петров     │
│   Пост создан                          │
│   [Просмотр] [Восстановить]            │
│                                         │
├─────────────────────────────────────────┤
│ [← Предыдущие] [Следующие →]           │
│                                         │
│ Показано 5 из 23 ревизий               │
└─────────────────────────────────────────┘
```

**React Component:**

```tsx
function RevisionHistoryPanel({ contentId, contentType }: Props) {
  const [revisions, setRevisions] = useState<Revision[]>([]);
  const [loading, setLoading] = useState(false);
  const [cursor, setCursor] = useState<string | null>(null);
  const [selectedRevision, setSelectedRevision] = useState<Revision | null>(null);

  const loadRevisions = async (cursor?: string) => {
    setLoading(true);
    const result = await graphql.query(GET_REVISIONS, {
      contentId,
      contentType,
      locale: 'ru',
      first: 10,
      after: cursor,
    });
    setRevisions(result.data.revisions.edges.map(e => e.node));
    setLoading(false);
  };

  useEffect(() => {
    loadRevisions();
  }, [contentId]);

  return (
    <div className="revision-history-panel">
      <h3>История изменений</h3>
      
      <SearchBar placeholder="Поиск по изменениям..." />
      
      <FilterBar>
        <Select label="Тип" options={['Все', 'Публикации', 'Обновления']} />
        <Select label="Период" options={['Все', 'Последние 7 дней', 'Последний месяц']} />
        <Select label="Автор" options={['Все', 'Иван Петров', 'Анна Сидорова']} />
      </FilterBar>

      <RevisionList>
        {revisions.map(revision => (
          <RevisionItem
            key={revision.id}
            revision={revision}
            isCurrent={revision.revisionNumber === currentRevision}
            onView={() => handleView(revision)}
            onCompare={() => handleCompare(revision)}
            onRestore={() => handleRestore(revision)}
          />
        ))}
      </RevisionList>

      <Pagination
        hasNext={pageInfo.hasNextPage}
        hasPrevious={pageInfo.hasPreviousPage}
        onNext={() => loadRevisions(pageInfo.endCursor)}
        onPrevious={() => loadRevisions(pageInfo.startCursor)}
      />
    </div>
  );
}
```

### 2. Revision Detail View

**Назначение:** Просмотр деталей конкретной ревизии

**Компоненты:**
- Метаданные ревизии (кто, когда, что)
- Delta с изменениями
- Кнопки действий

**Mockup:**

```
┌─────────────────────────────────────────┐
│ Ревизия #5                              │
├─────────────────────────────────────────┤
│ 📅 Сегодня, 14:30                       │
│ 👤 Иван Петров (ivan@example.com)      │
│ 🏷️ API                                 │
│ 📝 Обновлен заголовок и контент        │
│                                         │
├─────────────────────────────────────────┤
│ Изменения:                              │
│                                         │
│ title:                                  │
│ - Старый заголовок                      │
│ + Новый заголовок                       │
│                                         │
│ content:                                │
│ - Старый контент поста...              │
│ + Новый контент поста с обновлениями...│
│                                         │
│ status:                                 │
│ - draft                                 │
│ + published                             │
│                                         │
├─────────────────────────────────────────┤
│ [← Назад к списку]                      │
│ [Сравнить с v4] [Восстановить эту]     │
│ [Создать snapshot]                      │
└─────────────────────────────────────────┘
```

**React Component:**

```tsx
function RevisionDetailView({ revision }: { revision: Revision }) {
  return (
    <div className="revision-detail">
      <Header>
        <h2>Ревизия #{revision.revisionNumber}</h2>
        {revision.versionName && (
          <Badge color="blue">{revision.versionName}</Badge>
        )}
      </Header>

      <Metadata>
        <MetaItem icon="calendar">
          {formatDate(revision.createdAt)}
        </MetaItem>
        <MetaItem icon="user">
          {revision.createdBy.name} ({revision.createdBy.email})
        </MetaItem>
        <MetaItem icon="source">
          {revision.changeSource}
        </MetaItem>
        {revision.changeSummary && (
          <MetaItem icon="note">
            {revision.changeSummary}
          </MetaItem>
        )}
      </Metadata>

      <Section title="Изменения">
        <DeltaViewer delta={revision.delta} />
      </Section>

      <Actions>
        <Button onClick={handleBack}>
          ← Назад к списку
        </Button>
        <Button onClick={handleCompare}>
          Сравнить с предыдущей
        </Button>
        <Button onClick={handleRestore} variant="warning">
          Восстановить эту версию
        </Button>
        <Button onClick={handleSnapshot}>
          Создать snapshot
        </Button>
      </Actions>
    </div>
  );
}
```

### 3. Diff Viewer

**Назначение:** Визуальное сравнение двух версий

**Компоненты:**
- Side-by-side сравнение
- Подсветка изменений
- Навигация по изменениям

**Mockup:**

```
┌─────────────────────────────────────────┐
│ Сравнение версий                        │
├─────────────────────────────────────────┤
│ [v3 ▼] → [v5 ▼]                        │
│                                         │
│ Изменено 3 поля                         │
│ [← Предыдущее] [Следующее →]           │
│                                         │
├──────────────────┬──────────────────────┤
│ Версия 3         │ Версия 5             │
├──────────────────┼──────────────────────┤
│ title:           │ title:               │
│ Старый заголовок │ Новый заголовок  ✓  │
│                  │                      │
├──────────────────┼──────────────────────┤
│ content:         │ content:             │
│ Старый контент   │ Новый контент    ✓  │
│ поста...         │ поста с              │
│                  │ обновлениями...      │
│                  │                      │
├──────────────────┼──────────────────────┤
│ status:          │ status:              │
│ draft            │ published        ✓  │
│                  │                      │
├──────────────────┴──────────────────────┤
│                                         │
│ [Закрыть] [Восстановить v3]            │
└─────────────────────────────────────────┘
```

**React Component:**

```tsx
function DiffViewer({ 
  contentId, 
  fromRevision, 
  toRevision 
}: Props) {
  const { data, loading } = useQuery(GET_DIFF, {
    variables: {
      contentId,
      contentType: 'blog_post',
      locale: 'ru',
      fromRevision,
      toRevision,
    },
  });

  if (loading) return <Spinner />;

  const diff = data.revisionDiff;

  return (
    <div className="diff-viewer">
      <Header>
        <h2>Сравнение версий</h2>
        <VersionSelector>
          <Select
            value={fromRevision}
            onChange={setFromRevision}
            options={revisions.map(r => ({
              value: r.revisionNumber,
              label: `v${r.revisionNumber}`,
            }))}
          />
          <span>→</span>
          <Select
            value={toRevision}
            onChange={setToRevision}
            options={revisions.map(r => ({
              value: r.revisionNumber,
              label: `v${r.revisionNumber}`,
            }))}
          />
        </VersionSelector>
      </Header>

      <Stats>
        Изменено {diff.changedFieldsCount} полей
      </Stats>

      <Navigation>
        <Button onClick={goToPreviousChange}>
          ← Предыдущее
        </Button>
        <Button onClick={goToNextChange}>
          Следующее →
        </Button>
      </Navigation>

      <DiffTable>
        <thead>
          <tr>
            <th>Версия {fromRevision}</th>
            <th>Версия {toRevision}</th>
          </tr>
        </thead>
        <tbody>
          {diff.changes.map(change => (
            <tr key={change.field}>
              <td>
                <strong>{change.field}:</strong>
                <DiffLine type="removed">
                  {JSON.stringify(change.oldValue, null, 2)}
                </DiffLine>
              </td>
              <td>
                <strong>{change.field}:</strong>
                <DiffLine type="added">
                  {JSON.stringify(change.newValue, null, 2)}
                </DiffLine>
              </td>
            </tr>
          ))}
        </tbody>
      </DiffTable>

      <Actions>
        <Button onClick={handleClose}>Закрыть</Button>
        <Button 
          onClick={() => handleRestore(fromRevision)}
          variant="warning"
        >
          Восстановить v{fromRevision}
        </Button>
      </Actions>
    </div>
  );
}
```

### 4. Named Versions Manager

**Назначение:** Управление именованными версиями (snapshots)

**Mockup:**

```
┌─────────────────────────────────────────┐
│ Именованные версии                      │
├─────────────────────────────────────────┤
│ [+ Создать snapshot]                    │
│                                         │
├─────────────────────────────────────────┤
│ 📌 v1.0-published                       │
│    Первая опубликованная версия        │
│    15 октября 2024, 16:45              │
│    Иван Петров                         │
│    [Просмотр] [Восстановить] [Удалить] │
│                                         │
│ 📌 before-bulk-update                   │
│    Перед массовым обновлением          │
│    10 октября 2024, 14:20              │
│    Анна Сидорова                       │
│    [Просмотр] [Восстановить] [Удалить] │
│                                         │
│ 📌 v0.9-draft                           │
│    Финальный черновик                  │
│    5 октября 2024, 11:30               │
│    Иван Петров                         │
│    [Просмотр] [Восстановить] [Удалить] │
│                                         │
└─────────────────────────────────────────┘
```

**React Component:**

```tsx
function NamedVersionsManager({ contentId, contentType }: Props) {
  const [versions, setVersions] = useState<NamedVersion[]>([]);
  const [showCreateModal, setShowCreateModal] = useState(false);

  const { data } = useQuery(GET_NAMED_VERSIONS, {
    variables: { contentId, contentType, locale: 'ru' },
  });

  const handleCreate = async (name: string, description: string) => {
    await graphql.mutate(CREATE_NAMED_VERSION, {
      input: {
        contentId,
        contentType,
        locale: 'ru',
        name,
        description,
      },
    });
    setShowCreateModal(false);
    refreshVersions();
  };

  const handleDelete = async (revisionId: string) => {
    if (confirm('Удалить эту версию?')) {
      await graphql.mutate(DELETE_NAMED_VERSION, {
        variables: { revisionId },
      });
      refreshVersions();
    }
  };

  return (
    <div className="named-versions-manager">
      <Header>
        <h2>Именованные версии</h2>
        <Button onClick={() => setShowCreateModal(true)}>
          + Создать snapshot
        </Button>
      </Header>

      <VersionsList>
        {versions.map(version => (
          <VersionCard key={version.revision.id}>
            <VersionHeader>
              <Icon name="pin" />
              <h3>{version.name}</h3>
            </VersionHeader>
            
            {version.description && (
              <p>{version.description}</p>
            )}
            
            <Metadata>
              <span>{formatDate(version.createdAt)}</span>
              <span>{version.createdBy.name}</span>
            </Metadata>

            <Actions>
              <Button size="small" onClick={() => handleView(version)}>
                Просмотр
              </Button>
              <Button 
                size="small" 
                variant="warning"
                onClick={() => handleRestore(version.revision.revisionNumber)}
              >
                Восстановить
              </Button>
              <Button 
                size="small" 
                variant="danger"
                onClick={() => handleDelete(version.revision.id)}
              >
                Удалить
              </Button>
            </Actions>
          </VersionCard>
        ))}
      </VersionsList>

      {showCreateModal && (
        <CreateSnapshotModal
          onCreate={handleCreate}
          onClose={() => setShowCreateModal(false)}
        />
      )}
    </div>
  );
}
```

### 5. Restore Confirmation Modal

**Назначение:** Подтверждение восстановления к предыдущей версии

**Mockup:**

```
┌─────────────────────────────────────────┐
│ Восстановить версию?                    │
├─────────────────────────────────────────┤
│                                         │
│ Вы собираетесь восстановить контент к  │
│ версии #3 от 12 октября 2024.          │
│                                         │
│ ⚠️ Текущая версия будет заменена.      │
│                                         │
│ Будет создана новая ревизия с          │
│ содержимым версии #3.                  │
│                                         │
│ Описание изменения (опционально):      │
│ ┌───────────────────────────────────┐  │
│ │ Восстановление после случайного  │  │
│ │ удаления контента                │  │
│ └───────────────────────────────────┘  │
│                                         │
│ ☑ Создать snapshot текущей версии     │
│   перед восстановлением                │
│                                         │
├─────────────────────────────────────────┤
│ [Отмена]              [Восстановить]   │
└─────────────────────────────────────────┘
```

**React Component:**

```tsx
function RestoreConfirmationModal({
  revision,
  onConfirm,
  onCancel,
}: Props) {
  const [summary, setSummary] = useState('');
  const [createSnapshot, setCreateSnapshot] = useState(true);

  const handleConfirm = async () => {
    if (createSnapshot) {
      await createNamedVersion({
        name: `before-restore-v${revision.revisionNumber}`,
        description: 'Snapshot перед восстановлением',
      });
    }

    await onConfirm({
      revisionNumber: revision.revisionNumber,
      summary,
    });
  };

  return (
    <Modal onClose={onCancel}>
      <ModalHeader>
        <h2>Восстановить версию?</h2>
      </ModalHeader>

      <ModalBody>
        <Alert type="warning">
          <p>
            Вы собираетесь восстановить контент к версии{' '}
            <strong>#{revision.revisionNumber}</strong> от{' '}
            {formatDate(revision.createdAt)}.
          </p>
        </Alert>

        <WarningBox>
          ⚠️ Текущая версия будет заменена.
        </WarningBox>

        <p>
          Будет создана новая ревизия с содержимым версии{' '}
          #{revision.revisionNumber}.
        </p>

        <FormGroup>
          <Label>Описание изменения (опционально):</Label>
          <Textarea
            value={summary}
            onChange={setSummary}
            placeholder="Почему вы восстанавливаете эту версию?"
          />
        </FormGroup>

        <Checkbox
          checked={createSnapshot}
          onChange={setCreateSnapshot}
          label="Создать snapshot текущей версии перед восстановлением"
        />
      </ModalBody>

      <ModalFooter>
        <Button onClick={onCancel}>Отмена</Button>
        <Button 
          onClick={handleConfirm}
          variant="warning"
        >
          Восстановить
        </Button>
      </ModalFooter>
    </Modal>
  );
}
```

## Стилизация

### Цветовая схема

```css
:root {
  --revision-current: #10b981;
  --revision-added: #3b82f6;
  --revision-removed: #ef4444;
  --revision-modified: #f59e0b;
  
  --diff-added-bg: #d1fae5;
  --diff-removed-bg: #fee2e2;
  --diff-modified-bg: #fef3c7;
}
```

### Компоненты

```css
.revision-item {
  padding: 1rem;
  border-left: 3px solid var(--revision-modified);
  margin-bottom: 0.5rem;
  background: white;
  border-radius: 0.5rem;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.revision-item.current {
  border-left-color: var(--revision-current);
  background: #f0fdf4;
}

.diff-line.added {
  background: var(--diff-added-bg);
  padding: 0.25rem 0.5rem;
  border-radius: 0.25rem;
}

.diff-line.removed {
  background: var(--diff-removed-bg);
  padding: 0.25rem 0.5rem;
  border-radius: 0.25rem;
  text-decoration: line-through;
}
```

## Accessibility

### Keyboard Navigation

```tsx
// Навигация с клавиатуры
useKeyboardNavigation({
  onUp: () => selectPreviousRevision(),
  onDown: () => selectNextRevision(),
  onEnter: () => viewSelectedRevision(),
  onEscape: () => closePanel(),
});
```

### ARIA Labels

```tsx
<button
  aria-label={`Восстановить к версии ${revision.revisionNumber}`}
  onClick={() => handleRestore(revision)}
>
  Восстановить
</button>

<div
  role="list"
  aria-label="Список ревизий"
>
  {revisions.map(revision => (
    <div
      key={revision.id}
      role="listitem"
      aria-label={`Ревизия ${revision.revisionNumber}`}
    >
      {/* ... */}
    </div>
  ))}
</div>
```

### Screen Reader Support

```tsx
<VisuallyHidden>
  Ревизия {revision.revisionNumber}, создана {formatDate(revision.createdAt)} 
  пользователем {revision.createdBy.name}. 
  {revision.changeSummary && `Описание: ${revision.changeSummary}`}
</VisuallyHidden>
```

## Responsive Design

### Mobile View

```css
@media (max-width: 768px) {
  .revision-history-panel {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    max-height: 50vh;
    overflow-y: auto;
  }

  .diff-viewer {
    flex-direction: column;
  }

  .diff-table th,
  .diff-table td {
    display: block;
    width: 100%;
  }
}
```

## Performance Optimizations

### Virtual Scrolling

```tsx
import { FixedSizeList } from 'react-window';

function RevisionList({ revisions }: Props) {
  return (
    <FixedSizeList
      height={600}
      itemCount={revisions.length}
      itemSize={100}
      width="100%"
    >
      {({ index, style }) => (
        <div style={style}>
          <RevisionItem revision={revisions[index]} />
        </div>
      )}
    </FixedSizeList>
  );
}
```

### Lazy Loading

```tsx
const DiffViewer = lazy(() => import('./DiffViewer'));
const NamedVersionsManager = lazy(() => import('./NamedVersionsManager'));

function RevisionHistory() {
  return (
    <Suspense fallback={<Spinner />}>
      <DiffViewer />
    </Suspense>
  );
}
```

## Testing

### Unit Tests

```tsx
describe('RevisionHistoryPanel', () => {
  it('отображает список ревизий', () => {
    render(<RevisionHistoryPanel contentId="123" contentType="blog_post" />);
    expect(screen.getByText('Ревизия #5')).toBeInTheDocument();
  });

  it('фильтрует ревизии по дате', () => {
    render(<RevisionHistoryPanel contentId="123" contentType="blog_post" />);
    fireEvent.click(screen.getByText('Последние 7 дней'));
    expect(screen.queryByText('3 дня назад')).not.toBeInTheDocument();
  });
});
```

### Integration Tests

```tsx
describe('Восстановление ревизии', () => {
  it('восстанавливает к предыдущей версии', async () => {
    render(<RevisionHistoryPanel contentId="123" contentType="blog_post" />);
    
    fireEvent.click(screen.getByText('Восстановить'));
    fireEvent.click(screen.getByText('Подтвердить'));
    
    await waitFor(() => {
      expect(screen.getByText('Версия восстановлена')).toBeInTheDocument();
    });
  });
});
```

## Заключение

Admin UI для revision history предоставляет:

✅ **Интуитивный интерфейс** — легкое управление версиями  
✅ **Визуальное сравнение** — side-by-side diff viewer  
✅ **Быстрое восстановление** — one-click restore  
✅ **Named versions** — управление snapshots  
✅ **Responsive design** — работает на всех устройствах  
✅ **Accessibility** — поддержка screen readers и keyboard navigation  
✅ **Performance** — virtual scrolling и lazy loading  

UI готов к реализации!
