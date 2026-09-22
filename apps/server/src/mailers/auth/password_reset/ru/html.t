<!DOCTYPE html>
<html lang="ru">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Сброс пароля</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f4f4f5; margin: 0; padding: 0; }
    .wrapper { max-width: 560px; margin: 40px auto; background: #ffffff; border-radius: 8px; box-shadow: 0 1px 4px rgba(0,0,0,.12); overflow: hidden; }
    .header  { background: #18181b; padding: 24px 32px; }
    .header h1 { color: #ffffff; margin: 0; font-size: 20px; font-weight: 600; }
    .body    { padding: 32px; color: #18181b; }
    .body p  { margin: 0 0 16px; line-height: 1.6; }
    .btn     { display: inline-block; padding: 12px 24px; background: #18181b; color: #ffffff; text-decoration: none; border-radius: 6px; font-weight: 600; font-size: 15px; }
    .footer  { padding: 16px 32px; font-size: 12px; color: #71717a; border-top: 1px solid #e4e4e7; }
  </style>
</head>
<body>
  <div class="wrapper">
    <div class="header"><h1>RusToK</h1></div>
    <div class="body">
      <p>Вы запросили сброс пароля для вашего аккаунта.</p>
      <p>Нажмите кнопку ниже, чтобы задать новый пароль. Ссылка действительна <strong>15 минут</strong>.</p>
      <p><a class="btn" href="{{ reset_url }}">Сбросить пароль</a></p>
      <p>Или скопируйте эту ссылку в браузер:<br /><small>{{ reset_url }}</small></p>
      <p>Если вы не запрашивали сброс пароля, проигнорируйте это письмо — ваш пароль останется прежним.</p>
    </div>
    <div class="footer">© RusToK. Это автоматическое сообщение, не отвечайте на него.</div>
  </div>
</body>
</html>
