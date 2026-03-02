export type TableSortDirection = 'asc' | 'desc';

export type TableSortRule = {
  field: string;
  direction: TableSortDirection;
};

export type TableFilterRule = {
  field: string;
  value: string;
};
