forum-badge = forum
forum-categories-label = Категории
forum-categories-noDescription = Описания пока нет.
forum-categories-title = Карта сообщества
forum-categories-total =
    { $count ->
        [one] { $count } раздел опубликован из модуля forum.
        [few] { $count } раздела опубликовано из модуля forum.
        [many] { $count } разделов опубликовано из модуля forum.
       *[other] { $count } раздела опубликовано из модуля forum.
    }
forum-composer-bodyPlaceholder = Напишите сообщение здесь. Пустая строка начинает новый абзац.
forum-composer-cancel = Отмена
forum-composer-categoryPlaceholder = Выберите категорию
forum-composer-close = Закрыть
forum-composer-draft = Черновик
forum-composer-draftActive = Черновик активен
forum-composer-expand = Развернуть
forum-composer-minimize = Свернуть
forum-composer-newTopicTitle = Создать новую тему
forum-composer-previewEmpty = Пока нечего показывать.
forum-composer-previewHint = Живой предпросмотр
forum-composer-quotesLabel = Цитаты
forum-composer-replyTitle = Ответ
forum-composer-submitFailed = Форум отклонил эту запись
forum-composer-submitReply = Опубликовать ответ
forum-composer-submitTopic = Создать тему
forum-composer-submitting = Публикация…
forum-composer-tabPreview = Предпросмотр
forum-composer-tabWrite = Текст
forum-composer-topicTitlePlaceholder = Заголовок темы…
forum-error-loadStorefront = Не удалось загрузить storefront-данные форума
forum-error-updateReadState = Не удалось обновить состояние прочтения форума
forum-feed-emptyBody = Опубликуйте тему из forum admin package, чтобы наполнить эту storefront-ленту.
forum-feed-emptyTitle = Тем пока нет
forum-feed-label = Лента тем
forum-feed-threads =
    { $count ->
        [one] { $count } тред
        [few] { $count } треда
        [many] { $count } тредов
       *[other] { $count } треда
    }
forum-feed-title = Последние обсуждения
forum-member-replies = ответов
forum-member-solutions = решений
forum-member-topics = тем
forum-richContent-summary =
    { $count ->
        [one] Контент хранится в формате `{ $format }`. Длина сырого содержимого: { $count } символ.
        [few] Контент хранится в формате `{ $format }`. Длина сырого содержимого: { $count } символа.
        [many] Контент хранится в формате `{ $format }`. Длина сырого содержимого: { $count } символов.
       *[other] Контент хранится в формате `{ $format }`. Длина сырого содержимого: { $count } символа.
    }
forum-subtitle = NodeBB-подобная storefront-поверхность, которая читает категории, topic feed и ответы треда через публичный GraphQL contract модуля forum.
forum-thread-markRead = Отметить тему прочитанной
forum-thread-markingRead = Отмечаем прочитанной…
forum-thread-noReplies = Ответов пока нет.
forum-thread-openBody = Выберите тему в ленте, чтобы прочитать стартовый пост и последние ответы.
forum-thread-openTitle = Откройте тред
forum-thread-quote = Цитировать
forum-thread-quoteTitle = Цитировать этот ответ
forum-thread-repliesTitle = Ответы
forum-thread-repliesTotal = { $count } всего
forum-thread-reply = Ответить
forum-thread-replyClosed = Ответы в этой теме закрыты.
forum-thread-slug = slug: { $slug }
forum-title = Треды сообщества из пакета модуля
forum-topic-locked = Закрыто
forum-topic-statusOpen = Открыта
forum-topic-statusClosed = Закрыта
forum-topic-statusArchived = В архиве
forum-topic-statusPending = На модерации
forum-topic-statusApproved = Одобрено
forum-topic-statusRejected = Отклонено
forum-topic-statusFlagged = Жалобы
forum-topic-statusHidden = Скрыто
forum-topic-statusDeleted = Удалено
forum-topic-pinned = Закреплено
forum-topic-replies = Ответы
forum-topic-slug = slug треда: { $slug }
forum-topic-unreadCount = { $count } непрочитанных
forum-topic-updatedUnread = Обновлено
forum-view-categories = Категории
forum-view-discussions = Обсуждения
