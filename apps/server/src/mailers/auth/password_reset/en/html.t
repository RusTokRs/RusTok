<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Password Reset</title>
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
      <p>You requested a password reset for your account.</p>
      <p>Click the button below to set a new password. This link is valid for <strong>15 minutes</strong>.</p>
      <p><a class="btn" href="{{ reset_url }}">Reset Password</a></p>
      <p>Or copy this URL into your browser:<br /><small>{{ reset_url }}</small></p>
      <p>If you did not request this, please ignore this email — your password will not change.</p>
    </div>
    <div class="footer">© RusToK. This is an automated message, please do not reply.</div>
  </div>
</body>
</html>
