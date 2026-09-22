export interface User {
  id: string;
  email: string;
  name: string | null;
  role: string;
  status: string;
  createdAt?: string;
  tenantName?: string | null;
}

export interface UsersResponse {
  users: {
    edges: Array<{ node: User }>;
    pageInfo: { totalCount: number };
  };
}
