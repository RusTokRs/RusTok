#!/usr/bin/env bash
# Runs every step listed in steps.txt, collects compiler diagnostics and failing test output, and
# posts a condensed report as a comment on the commit that triggered the run.
set -uo pipefail

sha="$(git rev-parse HEAD)"
report="$(mktemp)"
overall=0

git config user.name "arena-devloop"
git config user.email "arena-devloop@users.noreply.github.com"

fmt_packages=(-p fly -p fly-ui -p fly-web -p fly-browser -p fly-leptos -p rustok-page-builder
  -p rustok-page-builder-admin -p rustok-pages -p rustok-pages-admin -p rustok-pages-storefront)
if cargo fmt "${fmt_packages[@]}" >/tmp/fmt.log 2>&1; then
  if ! git diff --quiet; then
    git commit -am "style: cargo fmt (arena dev loop)" >/dev/null
    git push origin "HEAD:${GITHUB_REF_NAME}" >/tmp/push.log 2>&1 \
      && echo "fmt: pushed formatting commit $(git rev-parse --short HEAD)" >>"$report" \
      || { echo "fmt: push failed"; tail -5 /tmp/push.log; } >>"$report"
  else
    echo "fmt: clean" >>"$report"
  fi
else
  echo "fmt: FAILED" >>"$report"
  tail -40 /tmp/fmt.log >>"$report"
  overall=1
fi

while IFS= read -r step || [[ -n "$step" ]]; do
  [[ -z "${step// }" || "$step" == \#* ]] && continue
  log="$(mktemp)"
  start=$(date +%s)
  bash -c "$step" >"$log" 2>&1
  code=$?
  took=$(( $(date +%s) - start ))
  if [[ $code -eq 0 ]]; then
    echo "OK   (${took}s) $step" >>"$report"
  else
    overall=1
    {
      echo ""
      echo "FAIL (${took}s, exit $code) $step"
      echo '```'
      # Compiler errors first (short format or full), then failing tests, then the tail.
      grep -E '^(error|warning: unused)|^\s+--> |^[^ ]+\.rs:[0-9]+:[0-9]+: error' "$log" | head -80
      grep -E -A25 '^---- .* stdout ----' "$log" | head -120
      grep -E '^test result:|panicked at|FAILED|failures:' "$log" | head -30
      echo "--- tail ---"
      tail -40 "$log"
      echo '```'
    } >>"$report"
  fi
done < .github/arena-devloop/steps.txt

body="$(printf '### Arena dev loop: %s\n\n```\n%s\n```\n' \
  "$([[ $overall -eq 0 ]] && echo PASS || echo FAIL)" "$(head -c 60000 "$report")")"
gh api "repos/${GITHUB_REPOSITORY}/commits/${sha}/comments" -f body="$body" >/dev/null \
  || echo "could not post commit comment"
cat "$report"
exit $overall
