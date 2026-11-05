blog-badge = блог
blog-error-archivePost = Не удалось архивировать пост
blog-error-createPreviewToken = Не удалось создать токен предпросмотра
blog-error-deletePost = Не удалось удалить пост
blog-preview-close = Закрыть
blog-preview-expires = Действует до { $at }
blog-preview-note = Показывается только один раз — бэкенд хранит дайджест, а не секрет. Публикация, планирование или архивация поста немедленно отзывают ссылку.
blog-preview-pathLabel = Путь предпросмотра — добавьте домен вашего сторфронта
blog-preview-title = Ссылка предпросмотра черновика
blog-preview-tokenLabel = Токен
blog-error-deleteReturnedFalse = Удаление поста вернуло false. Сначала снимите его с публикации или архивируйте.
blog-error-loadPost = Не удалось загрузить пост
blog-error-loadPosts = Не удалось загрузить посты
blog-error-postNotFound = Пост для редактирования не найден.
blog-error-requiredFields = Чтобы сохранить пост блога, нужны заголовок и содержимое.
blog-error-seoConfiguration = Конфигурация SEO блога недоступна
blog-error-seoTargetInvalid = SEO-цель блога некорректна. Обратитесь к администратору платформы.
blog-error-runtimeBadge = Рантайм
blog-error-sanitizeBadge = Санитизация
blog-error-savePost = Не удалось сохранить пост
blog-error-updateStatus = Не удалось обновить статус поста
blog-error-validationBadge = Валидация
blog-form-body = Содержимое
blog-form-content = Содержимое
blog-form-create = Создать пост
blog-form-createNewInstead = Создать новый вместо этого
blog-form-createTitle = Создать пост
blog-form-editTitle = Редактировать пост
blog-form-editingBanner = Редактируется пост { $id }
blog-form-excerpt = Краткое описание
blog-form-locale = Локаль
blog-form-publishNow = Опубликовать сразу
blog-form-saving = Сохранение...
blog-form-slug = Слаг
blog-form-subtitle = Пакет владеет и списком, и формой. apps/admin только хостит маршрут модуля.
blog-form-tags = Теги
blog-form-tagsPlaceholder = news, launch, release
blog-form-title = Заголовок
blog-form-update = Обновить пост
blog-moderation-approve = Одобрить
blog-moderation-empty = У этого поста нет комментариев для модерации.
blog-moderation-loadError = Не удалось загрузить очередь модерации комментариев
blog-moderation-next = Вперёд
blog-moderation-page = Страница
blog-moderation-previous = Назад
blog-moderation-reply = ответ
blog-moderation-selectPost = Откройте пост, чтобы проверить его комментарии.
blog-moderation-spam = Отметить как спам
blog-moderation-subtitle = Проверяйте неудалённые комментарии выбранного поста через owner boundary модуля Comments.
blog-moderation-title = Модерация комментариев
blog-moderation-total = Всего
blog-moderation-transport = Native и GraphQL
blog-moderation-trash = Переместить в корзину
blog-moderation-unavailable = Модерация комментариев недоступна в этой сокращённой сборке сервера.
blog-posts-subtitle = Загружается из rustok-blog-admin через GraphQL, а не зашивается вручную в apps/admin.
blog-posts-title = Посты
blog-seo-empty = Сначала создайте или откройте пост. SEO остаётся внутри редактора блога, а не в глобальном SEO-хабе.
blog-seo-subtitle = Явные метаданные, social tags и диагностика для выбранного поста блога.
blog-seo-title = SEO поста
blog-subtitle = Канонический module-owned CRUD-поток для постов блога через GraphQL-контракт модуля blog.
blog-table-archive = Архивировать
blog-table-author = Автор
blog-table-createdAt = Создан
blog-table-delete = Удалить
blog-table-draft = черновик
blog-table-edit = Редактировать
blog-table-editing = Редактируется
blog-table-empty = Пока нет постов. Создайте первый через форму пакета модуля.
blog-table-loadMore = Загрузить ещё
blog-table-locale = Локаль
blog-table-noExcerpt = Нет краткого описания
blog-table-previewLink = Ссылка предпросмотра
blog-table-publish = Опубликовать
blog-table-published = Опубликован
blog-table-slug = Слаг
blog-table-sort = Сортировка
blog-table-sortAsc = По возрастанию
blog-table-sortCreatedAt = Сначала новые
blog-table-sortDesc = По убыванию
blog-table-sortPublishedAt = Недавно опубликованные
blog-table-sortUpdatedAt = Недавно изменённые
blog-table-status = Статус
blog-table-title = Заголовок
blog-table-total =
    { $count ->
        [one] { $count } пост
        [few] { $count } поста
        [many] { $count } постов
       *[other] { $count } поста
    }
blog-table-unpublish = Снять с публикации
blog-title = Публикации блога
richText-apply_link = Применить ссылку
richText-blockquote = Цитата
richText-bold = Полужирный
richText-bullet_list = Маркированный список
richText-clear_formatting = Очистить форматирование
richText-code = Код
richText-code_block = Блок кода
richText-editor = Редактор форматированного текста
richText-error-frameUnavailable = Редактор форматированного текста недоступен.
richText-error-invalidPayload = Редактор вернул некорректный richtext-документ.
richText-error-serialize = Не удалось подготовить richtext-документ.
richText-heading = Заголовок
richText-horizontal_rule = Горизонтальная линия
richText-italic = Курсив
richText-link = Ссылка
richText-link_url = URL ссылки
richText-ordered_list = Нумерованный список
richText-redo = Повторить
richText-remove_link = Удалить ссылку
richText-strike = Зачёркнутый
richText-undo = Отменить
blog-error-restorePost = Не удалось восстановить публикацию
blog-table-restore = Восстановить

blog-form-category = Категория
blog-form-uncategorized = Без категории
blog-form-channels = Каналы (через запятую; пусто = все каналы)
blog-form-featuredImage = URL обложки
blog-form-seoTitle = SEO-заголовок (значение по умолчанию)
blog-form-seoDescription = SEO-описание (значение по умолчанию)
blog-error-categories = Не удалось загрузить категории
blog-form-scheduledAt = Публикация по расписанию (RFC 3339 с часовым поясом)
blog-table-pin = Закрепить
blog-table-unpin = Открепить
blog-error-updatePin = Не удалось изменить закрепление статьи

blog-categories-title = Категории блога
blog-categories-subtitle = Управление категориями таксономии через контракт владельца блога.
blog-categories-searchPlaceholder = Поиск категорий...
blog-categories-new = Новая категория
blog-categories-nameHeader = Название
blog-categories-slugHeader = Слаг
blog-categories-localeHeader = Язык
blog-categories-orderHeader = Порядок
blog-categories-actionsHeader = Действия
blog-categories-edit = Изменить
blog-categories-delete = Удалить
blog-categories-confirmDelete = Подтвердить удаление
blog-categories-cancel = Отмена
blog-categories-createNewInstead = Создать новую
blog-categories-reset = Сбросить
blog-categories-create = Создать категорию
blog-categories-update = Обновить категорию
blog-categories-editing = Редактирование категории
blog-categories-empty = Категории ещё не созданы.
blog-categories-error-load = Не удалось загрузить категории
blog-categories-error-save = Не удалось сохранить категорию
blog-categories-error-delete = Не удалось удалить категорию
blog-categories-error-invalidName = Введите название категории длиной от 1 до 120 символов.
blog-categories-error-invalidPosition = Порядок категории должен быть неотрицательным числом.
blog-categories-form-title = Категория
blog-categories-form-subtitle = Создание категории или правка выбранной.
blog-categories-form-name = Название
blog-categories-form-slug = Слаг (необязательно)
blog-categories-form-description = Описание
blog-categories-form-parent = Родительская категория
blog-categories-form-root = Корень (без родителя)
blog-categories-form-position = Порядок

blog-grid-title = Заголовок
blog-grid-slug = Слаг
blog-grid-status = Статус
blog-grid-locale = Язык
blog-grid-author = Автор
blog-grid-created = Создан
blog-grid-published = Опубликован
blog-grid-actions = Действия
blog-grid-filter = Фильтр...
blog-grid-search = Поиск по статьям блога...

blog-categories-loadError = Не удалось загрузить категории
blog-categories-deleted = Категория удалена
blog-categories-deleteError = Не удалось удалить категорию
blog-categories-search = Поиск категорий...
blog-categories-name = Название
blog-categories-slug = Слаг
blog-categories-position = Порядок
blog-table-actions = Действия
blog-categories-searchEmpty = Категории не найдены.
blog-categories-add = Добавить категорию
blog-categories-loading = Загрузка категорий...
blog-categories-deleteTitle = Удаление категории
blog-moderation-updated = Статус комментария обновлён
blog-moderation-loading = Загрузка комментариев...
blog-pagination-previous = Назад
blog-pagination-next = Далее
blog-moderation-pages = Страницы модерации
blog-table-filterTitle = Фильтр по заголовку
blog-table-searchTitle = Поиск по заголовку...
blog-table-filterSlug = Фильтр по слагу
blog-table-searchSlug = Поиск по слагу...
blog-table-filterLocale = Фильтр по языку
blog-table-searchLocale = Поиск по языку...
blog-table-filterAuthor = Фильтр по автору
blog-table-searchAuthor = Идентификатор автора...
blog-table-pages = Страницы статей
blog-table-deleted = Статья удалена
blog-table-deleteError = Не удалось удалить статью
blog-table-unpublished = Статья снята с публикации
blog-table-restored = Статья возвращена в черновики
blog-table-statusError = Не удалось изменить статус
blog-table-archived = Статья архивирована
blog-table-archiveError = Не удалось архивировать статью
blog-table-pinned = Статья закреплена
blog-table-unpinned = Статья откреплена
blog-table-pinError = Не удалось изменить закрепление
blog-table-openMenu = Открыть меню
blog-table-sortField = Сортировать по
blog-table-sortOrder = Порядок сортировки
blog-table-filterStatus = Фильтр по статусу
blog-table-allStatuses = Все статусы
blog-preview-copyLink = Копировать ссылку
blog-preview-open = Открыть предпросмотр
blog-preview-copyToken = Копировать токен
blog-preview-expiresLabel = Действует до
blog-preview-done = Готово
blog-preview-post = Черновик:
blog-preview-lifetime = Срок действия ссылки
blog-sort-created_at = Создан
blog-sort-published_at = Опубликован
blog-sort-title = Заголовок
blog-sort-asc = По возрастанию
blog-sort-desc = По убыванию
blog-status-draft = Черновик
blog-status-published = Опубликован
blog-status-archived = В архиве
blog-sort-updated_at = Последние изменения
blog-moderation-approved = Одобрить
blog-moderation-saving = Сохранение...
blog-categories-deleteConfirm = Удалить категорию «{ $name }»? Это действие нельзя отменить.
blog-editor-saved = Запись сохранена.

blog-editor-savedReloadFailed = Запись сохранена. Обновите её перед редактированием: текущие данные загрузить не удалось.

blog-editor-fallbackNotice = Перевод на выбранный язык отсутствует. Исходный текст ниже доступен только для чтения; введите заголовок и текст на выбранном языке, чтобы создать перевод.

blog-editor-categoryFallbackNotice = Перевод на выбранный язык отсутствует. Исходный текст ниже доступен только для чтения; введите название на выбранном языке, чтобы создать перевод.

blog-editor-retry = Повторить

blog-editor-currentCategory = Текущая категория

blog-editor-requiredCopy = Перед созданием перевода введите заголовок и текст на выбранном языке.

blog-editor-localeChanged = Язык содержимого изменился. Перед сохранением загрузите выбранный язык.

blog-editor-invalidSchedule = Введите дату и время в формате RFC 3339 с часовым поясом.

blog-editor-invalidImage = Введите HTTP/HTTPS URL или путь изображения от корня сайта.

blog-editor-revisionMissing = Версия записи отсутствует. Обновите запись перед сохранением.

blog-editor-categoriesUnavailable = Не удалось загрузить категории. Текущая категория сохранена; повторите загрузку, чтобы изменить её.

blog-editor-scheduleHelp = Используйте RFC 3339 с часовым поясом, например 2026-12-01T10:00:00+03:00.

blog-categories-created = Категория создана.

blog-categories-updated = Категория обновлена.

blog-editor-imagePreview = Предпросмотр главного изображения

blog-editor-slugRequired = Канонический адрес обязателен
