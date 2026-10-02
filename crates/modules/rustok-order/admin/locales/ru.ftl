order-action-cancel = Отменить
order-action-deliver = Доставить
order-action-markPaid = Отметить оплаченным
order-action-open = Открыть
order-action-refresh = Обновить
order-action-ship = Отгрузить
order-actionHint-cancelled = Заказ уже отменён; lifecycle-кнопки остаются только для чтения.
order-actionHint-confirmed = Следующий операционный шаг — отметить заказ как оплаченный.
order-actionHint-delivered = Заказ завершён; дальше остаётся только инспекция статуса.
order-actionHint-paid = Заказ оплачен и готов к отгрузке.
order-actionHint-pending = Заказ ждёт следующего lifecycle-события от checkout или оператора.
order-actionHint-shipped = Заказ уже в пути и может быть отмечен как доставленный.
order-badge = order
order-caption-created = создан { $date }
order-caption-customer = клиент { $customer }
order-detail-channel = канал { $channel }
order-detail-created = создан { $date }
order-detail-empty = Откройте заказ, чтобы посмотреть line items, payment state и прогресс доставки.
order-detail-none = Связанных данных нет.
order-detail-subtitle = Смотрите operational snapshot и выполняйте только тот lifecycle-шаг, который соответствует текущему статусу.
order-detail-title = Детали заказа
order-detail-updated = обновлён { $date }
order-error-bootstrapLoading = Bootstrap ещё загружается.
order-error-cancel = Не удалось отменить заказ
order-error-deliver = Не удалось отметить заказ как доставленный
order-error-loadOrder = Не удалось загрузить заказ
order-error-loadOrders = Не удалось загрузить список заказов
order-error-markPaid = Не удалось отметить заказ как оплаченный
order-error-markPaidRequirements = Нужны payment id и payment method.
order-error-orderNotFound = Заказ не найден.
order-error-selectionRequired = Сначала откройте заказ.
order-error-ship = Не удалось отгрузить заказ
order-error-shipRequirements = Нужны tracking number и carrier.
order-field-cancelReason = Причина отмены
order-field-carrier = Перевозчик
order-field-deliveredSignature = Подпись при получении
order-field-paymentId = Payment ID
order-field-paymentMethod = Способ оплаты
order-field-trackingNumber = Трек-номер
order-filter-allStatuses = Все статусы
order-fulfillment-carrier = перевозчик: { $carrier }
order-fulfillment-deliveredNote = примечание о доставке: { $note }
order-fulfillment-status = статус: { $status }
order-fulfillment-tracking = трекинг: { $tracking }
order-header-payment = платёж { $id }
order-header-tracking = трекинг { $tracking }
order-list-empty = По текущим фильтрам заказы не найдены.
order-list-subtitle = Смотрите заказы, созданные checkout, и переходите к operational lifecycle transitions.
order-list-title = Заказы
order-lines-empty = нет позиций
order-lines-itemsCount =
    { $count ->
        [one] { $count } позиция
        [few] { $count } позиции
       *[other] { $count } позиций
    }
# plural-exempt: compact line item metadata
order-lines-lineDetails = { $sku } · кол-во { $quantity } · профиль { $profile }
order-lines-more =
    { $count ->
        [one] + ещё { $count }
        [few] + ещё { $count }
       *[other] + ещё { $count }
    }
order-lines-unitPrice = цена за ед. { $price }
order-loading = Загрузка...
order-payment-authorized = авторизовано: { $amount }
order-payment-captured = списано: { $amount }
order-payment-paymentsCount =
    { $count ->
        [one] платежей: { $count }
        [few] платежей: { $count }
       *[other] платежей: { $count }
    }
order-payment-provider = провайдер: { $provider }
order-payment-status = статус: { $status }
order-section-actions = Lifecycle-действия
order-section-customer = Клиент
order-section-fulfillment = Fulfillment
order-section-lifecycle = Lifecycle
order-section-lines = Позиции заказа
order-section-payment = Платёжная коллекция
order-status-cancelled = Отменён
order-status-confirmed = Подтверждён
order-status-delivered = Доставлен
order-status-paid = Оплачен
order-status-pending = Ожидает
order-status-shipped = Отгружен
order-subtitle = Module-owned операторская поверхность для order lifecycle, видимости payment state и delivery progress.
order-timeline-cancelled = отменён { $date }
order-timeline-confirmed = подтверждён { $date }
order-timeline-created = создан { $date }
order-timeline-delivered = доставлен { $date }
order-timeline-paid = оплачен { $date }
order-timeline-shipped = отгружен { $date }
order-title = Операции с заказами

