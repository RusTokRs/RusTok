import { auth } from '@/auth';
import { clearRustokSessionCookies, isRustokAccessTokenExpiring, persistRustokSessionCookies, readRustokSessionCookies, refreshRustokSession, RustokSessionError } from '@/shared/api/auth-server';
import { isSameOriginRequest } from '@/shared/api/auth-session';
const API_BASE_URL=(process.env.RUSTOK_API_URL ?? process.env.NEXT_PUBLIC_API_URL ?? 'http://127.0.0.1:5150').replace(/\/+$/,'');
const GRAPHQL_URL=API_BASE_URL+'/api/graphql';
const MAX_BODY=1024*1024;
function error(code:string,message:string,status=200):Response {
 return Response.json({errors:[{message,extensions:{code,retryable:false,outcome:'rejected'}}]},{status,headers:{'cache-control':'no-store'}});
}
async function readBoundedBody(request: Request): Promise<string | null> {
 const reader=request.body?.getReader();
 if(!reader) return '';
 const chunks:Uint8Array[]=[];
 let size=0;
 try {
  while(true) {
   const {done,value}=await reader.read();
   if(done) break;
   size+=value.byteLength;
   if(size>MAX_BODY) { await reader.cancel(); return null; }
   chunks.push(value);
  }
 } finally { reader.releaseLock(); }
 const bytes=new Uint8Array(size);
 let offset=0;
 for(const chunk of chunks) { bytes.set(chunk,offset); offset+=chunk.byteLength; }
 return new TextDecoder().decode(bytes);
}
export async function POST(request:Request) {
 if(!isSameOriginRequest(request.url,request.headers.get('origin'),request.headers.get('referer'))) return error('FORBIDDEN','Cross-origin GraphQL requests are not allowed',403);
 if((request.headers.get('content-type')??'').split(';',1)[0].trim().toLowerCase()!=='application/json') return error('BAD_USER_INPUT','GraphQL requests must use application/json',415);
 const declared=Number(request.headers.get('content-length')??0);
 if(Number.isFinite(declared)&&declared>MAX_BODY) return error('PAYLOAD_TOO_LARGE','GraphQL request body is too large',413);
 const body=await readBoundedBody(request);
 if(body===null) return error('PAYLOAD_TOO_LARGE','GraphQL request body is too large',413);
 const session=await auth();
 const stored=await readRustokSessionCookies();
 const headers=new Headers({'content-type':'application/json'});
 if(session?.user) {
  const slug=session.user.tenantSlug, id=session.user.tenantId;
  if(!slug || (request.headers.get('x-tenant-slug')!==null && request.headers.get('x-tenant-slug')!==slug)) return error('FORBIDDEN','Workspace does not match the authenticated session',403);
  const requestedId=request.headers.get('x-tenant-id');
  if(id && requestedId!==null && requestedId!==id) return error('FORBIDDEN','Tenant does not match the authenticated session',403);
  if(!stored.accessToken&&!stored.refreshToken) return error('UNAUTHORIZED','Rustok session is unavailable. Sign in again.',401);
  let access=stored.accessToken;
  if(!access||isRustokAccessTokenExpiring(access)) {
   if(!stored.refreshToken) return error('UNAUTHORIZED','Rustok session expired. Sign in again.',401);
   try {
    const next=await refreshRustokSession(stored.refreshToken,slug,id,request.headers);
    await persistRustokSessionCookies(next); access=next.accessToken;
   } catch(e) {
    if(e instanceof RustokSessionError && e.outcome==='rejected') {
     await clearRustokSessionCookies();
     return error('UNAUTHORIZED','Rustok session could not be refreshed. Sign in again.',401);
    }
    return Response.json({errors:[{message:'Session refresh outcome is unknown; inspect current state before retrying.',extensions:{code:'SERVICE_UNAVAILABLE',retryable:false,outcome:'unknown'}}]},{status:503,headers:{'cache-control':'no-store'}});
   }
  }
  headers.set('authorization','Bearer '+access); headers.set('x-tenant-slug',slug);
  if(id) headers.set('x-tenant-id',id);
 } else {
  if(stored.accessToken||stored.refreshToken) {
   await clearRustokSessionCookies();
   return error('UNAUTHORIZED','Application session is unavailable. Sign in again.',401);
  }
  const slug=request.headers.get('x-tenant-slug'); if(slug) headers.set('x-tenant-slug',slug);
 }
 for(const name of ['x-rustok-effective-locale','accept-language']) {
  const value=request.headers.get(name); if(value) headers.set(name,value);
 }
 let upstream:Response;
 try { upstream=await fetch(GRAPHQL_URL,{method:'POST',headers,body,cache:'no-store'}); }
 catch { return Response.json({errors:[{message:'GraphQL request outcome is unknown; inspect current state before retrying.',extensions:{code:'SERVICE_UNAVAILABLE',retryable:false,outcome:'unknown'}}]},{status:503,headers:{'cache-control':'no-store'}}); }
 return new Response(upstream.body,{status:upstream.status,statusText:upstream.statusText,headers:{'cache-control':'no-store','content-type':upstream.headers.get('content-type')??'application/json'}});
}
