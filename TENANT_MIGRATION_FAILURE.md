# Tenant context loading migration v3 failure

```text
/tmp/unify_tenant_context_loading_v3.py:546: SyntaxWarning: invalid escape sequence '\.'
  '''forbidMatch(integration, /tenant\.resolution\s*=\s*"/, "integration tests must use typed tenant modes");
/tmp/unify_tenant_context_loading_v3.py:548: SyntaxWarning: invalid escape sequence '\.'
  '''forbidMatch(integration, /tenant\.resolution\s*=\s*"/, "integration tests must use typed tenant modes");
/tmp/unify_tenant_context_loading_v3.py:546: SyntaxWarning: invalid escape sequence '\.'
  '''forbidMatch(integration, /tenant\.resolution\s*=\s*"/, "integration tests must use typed tenant modes");
/tmp/unify_tenant_context_loading_v3.py:548: SyntaxWarning: invalid escape sequence '\.'
  '''forbidMatch(integration, /tenant\.resolution\s*=\s*"/, "integration tests must use typed tenant modes");
Traceback (most recent call last):
  File "/tmp/unify_tenant_context_loading_v3.py", line 37, in <module>
    resolution = replace_once(resolution, old, new, label)
                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/tmp/unify_tenant_context_loading_v3.py", line 20, in replace_once
    raise RuntimeError(f"{label}: expected 1 match, got {count}")
RuntimeError: route scope visibility: expected 1 match, got 0
```
