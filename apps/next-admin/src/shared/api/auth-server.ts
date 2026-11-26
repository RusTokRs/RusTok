import 'server-only';
import { cookies } from 'next/headers';
import {
  RUSTOK_ACCESS_COOKIE_NAME, RUSTOK_ACCESS_COOKIE_PATH,
  RUSTOK_ACCESS_REFRESH_SKEW_MS, RUSTOK_SESSION_COOKIE_NAME,
  RUSTOK_SESSION_COOKIE_PATH, RUSTOK_SESSION_REQUEST_HEADER,
  parseRustokSessionCookie
} from './auth-session';

const API_BASE_URL = (process.env.RUSTOK_API_URL ?? process.env.NEXT_PUBLIC_API_URL ?? 'http://127.0.0.1:5150').replace(/\/+$/, '');
const TIMEOUT_MS = 10_000;

export interface RustokSessionUser {
  id: string; email: string; name: string | null; role: string; status: string;
}
export interface RustokSessionCredentials {
  accessToken: string; expiresIn: number; refreshToken: string;
  refreshMaxAge: number; user: RustokSessionUser;
}
export class RustokSessionError extends Error {
  constructor(public readonly status: number, public readonly outcome: 'rejected' | 'unknown', message: string) {
    super(message); this.name = 'RustokSessionError';
  }
}
function parseSessionResponse(value: unknown): {access_token: string; expires_in: number; user: RustokSessionUser} {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid session response');
  const data = value as Record<string, unknown>;
  const user = data.user as Record<string, unknown> | null;
  if (!user || typeof user !== 'object' || typeof data.access_token !== 'string' ||
      !data.access_token || typeof data.expires_in !== 'number' ||
      !Number.isSafeInteger(data.expires_in) || data.expires_in <= 0 ||
      typeof user.id !== 'string' || !user.id || typeof user.email !== 'string' ||
      typeof user.role !== 'string' || typeof user.status !== 'string' ||
      (user.name !== null && typeof user.name !== 'string')) throw new Error('Invalid session response');
  return {access_token: data.access_token, expires_in: data.expires_in, user: {
    id: user.id, email: user.email, name: typeof user.name === 'string' ? user.name : null,
    role: user.role, status: user.status
  }};
}
function localeHeaders(source: Headers, target: Headers) {
  for (const name of ['x-rustok-effective-locale', 'accept-language']) {
    const value = source.get(name); if (value) target.set(name, value);
  }
}
function toCredentials(data: ReturnType<typeof parseSessionResponse>, cookie: {value: string; maxAge: number}): RustokSessionCredentials {
  return {accessToken: data.access_token, expiresIn: data.expires_in, refreshToken: cookie.value, refreshMaxAge: cookie.maxAge, user: data.user};
}
async function requestSession(path: string, init: RequestInit): Promise<{response: Response; data: ReturnType<typeof parseSessionResponse>}> {
  let response: Response;
  try { response = await fetch(API_BASE_URL + path, {...init, cache: 'no-store', signal: AbortSignal.timeout(TIMEOUT_MS)}); }
  catch { throw new RustokSessionError(0, 'unknown', 'Authentication service outcome is unknown'); }
  if (!response.ok) throw new RustokSessionError(response.status, response.status >= 400 && response.status < 500 ? 'rejected' : 'unknown', 'Authentication request failed');
  try { return {response, data: parseSessionResponse(await response.json())}; }
  catch { throw new RustokSessionError(502, 'unknown', 'Authentication service returned an invalid session'); }
}
export async function createRustokSession(input: {action: 'login'|'register'; email: string; password: string; tenantSlug: string; name?: string; request: Request}): Promise<RustokSessionCredentials> {
  const headers = new Headers({'content-type':'application/json','x-tenant-slug':input.tenantSlug});
  localeHeaders(input.request.headers, headers);
  const userAgent = input.request.headers.get('user-agent'); if (userAgent) headers.set('user-agent', userAgent);
  const {response,data} = await requestSession('/api/auth/session/' + input.action, {
    method:'POST', headers, body:JSON.stringify({email:input.email,password:input.password,...(input.action === 'register' ? {name:input.name ?? null} : {})})
  });
  const cookie = parseRustokSessionCookie(response.headers.get('set-cookie'));
  if (!cookie) throw new RustokSessionError(502, 'unknown', 'Rustok did not issue a valid refresh cookie');
  return toCredentials(data,cookie);
}
async function refreshOnce(refreshToken: string, tenantSlug: string, tenantId?: string|null, source?: Headers): Promise<RustokSessionCredentials> {
  const headers = new Headers({cookie:RUSTOK_SESSION_COOKIE_NAME + '=' + refreshToken,[RUSTOK_SESSION_REQUEST_HEADER]:'1','x-tenant-slug':tenantSlug});
  if (tenantId) headers.set('x-tenant-id',tenantId);
  if (source) localeHeaders(source,headers);
  const {response,data} = await requestSession('/api/auth/session/refresh',{method:'POST',headers});
  const cookie = parseRustokSessionCookie(response.headers.get('set-cookie'));
  if (!cookie) throw new RustokSessionError(502,'unknown','Rustok did not rotate the refresh cookie');
  return toCredentials(data,cookie);
}
const refreshes = new Map<string,Promise<RustokSessionCredentials>>();
export function refreshRustokSession(refreshToken:string,tenantSlug:string,tenantId?:string|null,source?:Headers):Promise<RustokSessionCredentials> {
  const key = tenantSlug + '\u0000' + refreshToken;
  const pending = refreshes.get(key); if (pending) return pending;
  const next = refreshOnce(refreshToken,tenantSlug,tenantId,source);
  refreshes.set(key,next);
  void next.then(()=>{if(refreshes.get(key)===next)refreshes.delete(key);},()=>{if(refreshes.get(key)===next)refreshes.delete(key);});
  return next;
}
export async function revokeRustokSession(refreshToken:string,tenantSlug:string,tenantId?:string|null):Promise<void> {
  const headers = new Headers({cookie:RUSTOK_SESSION_COOKIE_NAME + '=' + refreshToken,[RUSTOK_SESSION_REQUEST_HEADER]:'1','x-tenant-slug':tenantSlug});
  if (tenantId) headers.set('x-tenant-id',tenantId);
  let response:Response;
  try { response=await fetch(API_BASE_URL + '/api/auth/session/logout',{method:'POST',headers,cache:'no-store',signal:AbortSignal.timeout(5_000)}); }
  catch { throw new RustokSessionError(0,'unknown','Logout outcome is unknown'); }
  if (!response.ok) throw new RustokSessionError(response.status,response.status < 500 ? 'rejected':'unknown','Rustok logout failed');
}
function cookieOptions(path:string) { return {httpOnly:true,sameSite:'lax' as const,secure:process.env.NODE_ENV==='production',path}; }
export async function persistRustokSessionCookies(session:RustokSessionCredentials):Promise<void> {
  const store=await cookies();
  store.set(RUSTOK_SESSION_COOKIE_NAME,session.refreshToken,{...cookieOptions(RUSTOK_SESSION_COOKIE_PATH),maxAge:session.refreshMaxAge});
  store.set(RUSTOK_ACCESS_COOKIE_NAME,session.accessToken,{...cookieOptions(RUSTOK_ACCESS_COOKIE_PATH),maxAge:Math.max(1,session.expiresIn-Math.ceil(RUSTOK_ACCESS_REFRESH_SKEW_MS/1000))});
}
export async function readRustokSessionCookies() {
  const store=await cookies();
  return {accessToken:store.get(RUSTOK_ACCESS_COOKIE_NAME)?.value ?? null,refreshToken:store.get(RUSTOK_SESSION_COOKIE_NAME)?.value ?? null};
}
export async function clearRustokSessionCookies():Promise<void> {
  const store=await cookies();
  for (const [name,path] of [[RUSTOK_SESSION_COOKIE_NAME,RUSTOK_SESSION_COOKIE_PATH],[RUSTOK_ACCESS_COOKIE_NAME,RUSTOK_ACCESS_COOKIE_PATH]] as const) {
    store.set(name,'',{...cookieOptions(path),maxAge:0,expires:new Date(0)});
  }
}
