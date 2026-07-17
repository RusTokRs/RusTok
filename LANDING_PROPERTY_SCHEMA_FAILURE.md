# Landing property schema validation failure

## Integration

```text
Traceback (most recent call last):
  File "/tmp/integrate_landing_properties.py", line 213, in <module>
    text = replace_in_block(text, '"fly.contact_form"', ']\n    ]\n    .into_iter()',
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/tmp/integrate_landing_properties.py", line 168, in replace_in_block
    end_index = text.index(end, start_index)
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
ValueError: substring not found
```

## Validation

```text
```
