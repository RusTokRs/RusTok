welcome = Welcome to RusToK!
unread = { $count ->
    [0] No unread messages
    [one] One unread message
   *[other] { $count } unread messages
}
price = { NUMBER($amount, minimumFractionDigits: 2) }
updated = Updated { DATETIME($date, dateStyle: "medium") }
login-input = Email
    .placeholder = name@example.com
