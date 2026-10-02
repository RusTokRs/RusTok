comments-badge = комментарии
comments-comment-approve = Одобрить
comments-comment-pending = На проверке
comments-comment-spam = Спам
comments-comment-trash = В корзину
comments-detail-authorLine = автор { $author } · { $created_at }
comments-detail-localeLine = локаль { $requested } -> { $effective }
comments-detail-statusLine =
    { $count ->
        [one] { $count } комментарий, статус { $status }
        [few] { $count } комментария, статус { $status }
        [many] { $count } комментариев, статус { $status }
       *[other] { $count } комментария, статус { $status }
    }
comments-detail-thread = Тред
comments-detail-title = Детали треда
comments-error-loadThreads = Не удалось загрузить треды
comments-error-selectThread = Сначала выберите тред
comments-error-updateComment = Не удалось обновить комментарий
comments-error-updateThread = Не удалось обновить тред
comments-filters-allCommentStatuses = Все статусы комментариев
comments-filters-allThreadStatuses = Все статусы тредов
comments-filters-localePlaceholder = Локаль
comments-filters-targetTypePlaceholder = Тип цели
comments-pagination-next = Далее
comments-pagination-page = Страница { $page }
comments-pagination-prev = Назад
comments-subtitle = Module-owned поверхность модерации для обычных нефорумных комментариев. Этот UI остаётся native-first и не вводит отдельный GraphQL или REST transport.
comments-thread-closed = Закрыть
comments-thread-open = Открыть
comments-threads-count =
    { $count ->
        [one] { $count } комментарий
        [few] { $count } комментария
        [many] { $count } комментариев
       *[other] { $count } комментария
    }
comments-threads-title = Треды
comments-threads-total =
    { $count ->
        [one] найден { $count } тред
        [few] найдено { $count } треда
        [many] найдено { $count } тредов
       *[other] найдено { $count } треда
    }
comments-title = Модерация комментариев
