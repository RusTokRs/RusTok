const API_BASE_URL = process.env.RUSTOK_API_URL ?? 'http://127.0.0.1:5150';
const GRAPHQL_URL = `${API_BASE_URL}/api/graphql`;

const FORWARDED_HEADERS = [
  'authorization',
  'content-type',
  'x-tenant-slug',
  'x-tenant-id',
  'accept-language'
];

export async function POST(request: Request) {
  const headers = new Headers();

  for (const name of FORWARDED_HEADERS) {
    const value = request.headers.get(name);
    if (value) headers.set(name, value);
  }

  if (!headers.has('content-type')) {
    headers.set('content-type', 'application/json');
  }

  const response = await fetch(GRAPHQL_URL, {
    method: 'POST',
    headers,
    body: await request.text(),
    cache: 'no-store'
  });

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers: {
      'content-type': response.headers.get('content-type') ?? 'application/json'
    }
  });
}
