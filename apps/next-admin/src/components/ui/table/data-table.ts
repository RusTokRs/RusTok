// Compatibility path: the table widgets live in `@/widgets/data-table`. This file only re-exports
// so older imports keep working; it must never grow logic of its own.
export * from '../../../widgets/data-table/data-table';
export * from '../../../widgets/data-table/data-table-toolbar';
export * from '../../../widgets/data-table/data-table-shell';
export * from '../../../widgets/data-table/data-table-static';
