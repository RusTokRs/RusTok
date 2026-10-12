welcome = Добро пожаловать в RusToK!
unread = { $count ->
    [0] Нет непрочитанных сообщений
    [one] { $count } непрочитанное сообщение
    [few] { $count } непрочитанных сообщения
   *[other] { $count } непрочитанных сообщений
}
price = { NUMBER($amount, minimumFractionDigits: 2) }
updated = Обновлено { DATETIME($date, dateStyle: "medium") }
login-input = Почта
    .placeholder = name@example.com
