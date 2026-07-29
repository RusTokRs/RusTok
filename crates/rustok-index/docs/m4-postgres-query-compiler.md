# M4 controlled PostgreSQL query compiler

This slice compiles the database-independent `ExecutableQueryPlan` into one
controlled PostgreSQL statement plus an ordered typed bind list. It does not
execute SQL or publish an `IndexQueryPort`.

## Accepted subset

`ExecutableQueryPlan::compile_postgres` currently accepts:

- root-field projection;
- projection through explicit one-cardinality links;
- a fresh bounded cursor page with no continuation token;
- tenant, module, entity, schema-version, locale, and live-row scope;
- deterministic root/entity identity columns for later result assembly.

Every runtime value is a bind parameter:

- tenant UUID;
- module/entity names and schema versions;
- locale key;
- link name and exact target schema identity;
- projected field names;
- page limit.

Only fixed table/column names and planner-owned aliases are emitted into SQL.
Relation aliases must match `t` followed by decimal digits. The compiler also
rechecks the complete path-to-alias map before SQL construction. Link aliases are
compiler-owned `l1`, `l2`, and so on.

## Join contract

A one-cardinality link projection uses two `LEFT JOIN` operations:

1. source entity to `index_links`, including exact source version and link name;
2. link target identity to a live `index_entities` target.

The target module, entity, and schema version are separately bound from the
planned schema contract. Missing links therefore preserve the root entity while
returning null linked identity/field columns.

## Fail-closed pending semantics

The compiler returns typed pending errors instead of guessing semantics for:

- filters;
- explicit ordering;
- exact count;
- cursor continuation;
- offset pagination;
- many-cardinality link projection/aggregation.

Those features remain the next bounded M4 slice. Cursor bytes are not decoded by
this compiler, and no caller-controlled string is interpolated into SQL.

## Non-claims

This source slice does not:

- connect to PostgreSQL;
- execute or prepare a statement;
- decode result rows;
- authorize a caller;
- verify persisted schema readiness;
- implement filter, sort, count, keyset, offset, or many-link semantics;
- read source-module tables;
- change migrations or partition lifecycle state.

The repository owner runs compilation, unit tests, static verifiers, and later
PostgreSQL/reference-engine equivalence evidence.
