# Tenant context loading repair failure

```text
Traceback (most recent call last):
  File "/tmp/repair_tenant_context_loading.py", line 26, in <module>
    resolution = replace_once(
                 ^^^^^^^^^^^^^
  File "/tmp/repair_tenant_context_loading.py", line 14, in replace_once
    raise RuntimeError(f"{label}: expected 1 match, got {count}")
RuntimeError: reuse canonical slug constructor: expected 1 match, got 2
```
