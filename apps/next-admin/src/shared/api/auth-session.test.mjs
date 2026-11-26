import assert from 'node:assert/strict';
import test from 'node:test';
import { isRustokAccessTokenExpiring, isSameOriginRequest, parseRustokSessionCookie, encodeRustokRefreshCredential, parseRustokRefreshCredential, parseRustokHostCookies, retryAmbiguousRefresh, isCanonicalSessionId, rustokDirectSessionClaims, matchesRustokDirectSession, authRouteAction, handleConfirmedLogout, requestConfirmedLogout } from './auth-session.ts';

const refreshToken = '0123456789abcdef'.repeat(4);
const retrySeed = 'b1'.repeat(32);
const envelope = refreshToken+'.'+retrySeed;
function jwt(exp) {
  return ['header', Buffer.from(JSON.stringify({exp})).toString('base64url'), 'signature'].join('.');
}

test('accepts only an Auth-owned canonical refresh envelope with bounded Max-Age', () => {
  assert.deepEqual(parseRustokSessionCookie('rustok-session='+envelope+'; Path=/api/auth/session; HttpOnly; Max-Age=3600'), {credential:{token:refreshToken,retrySeed},maxAge:3600});
  for (const value of [
    null,
    'other='+refreshToken+'; Max-Age=3600',
    'rustok-session=bad; Max-Age=3600',
    'rustok-session='+refreshToken+'; Max-Age=3600',
    'rustok-session='+envelope+'; Max-Age=0',
    'rustok-session='+envelope+'; Max-Age=999999999'
  ]) assert.equal(parseRustokSessionCookie(value), null);
});

test('refresh threshold treats malformed and near-expiry JWTs as expiring', () => {
  assert.equal(isRustokAccessTokenExpiring('bad',1_000_000),true);
  assert.equal(isRustokAccessTokenExpiring(jwt(1060),1_000_000),true);
  assert.equal(isRustokAccessTokenExpiring(jwt(1061),1_000_000),false);
});

test('BFF origin guard requires exact same origin', () => {
  const url='https://admin.example.test/api/rustok/graphql';
  assert.equal(isSameOriginRequest(url,'https://admin.example.test',null),true);
  assert.equal(isSameOriginRequest(url,null,'https://admin.example.test/dashboard'),true);
  assert.equal(isSameOriginRequest(url,'https://attacker.test',null),false);
  assert.equal(isSameOriginRequest(url,null,null),false);
});


test('BFF credential envelope is canonical and does not accept bare or torn credentials', () => {
  const credential = { token: refreshToken, retrySeed: 'b1'.repeat(32) };
  const wire = encodeRustokRefreshCredential(credential);
  assert.deepEqual(parseRustokRefreshCredential(wire), credential);
  for (const value of [refreshToken, wire + '.extra', '0'.repeat(64)+'.'+credential.retrySeed,
    refreshToken+'.'+'0'.repeat(64), refreshToken+'.'+'AB'.repeat(32), wire+'\n']) {
    assert.equal(parseRustokRefreshCredential(value), null);
  }
  assert.throws(() => encodeRustokRefreshCredential({token: refreshToken, retrySeed:'bad'}));
});

test('the BFF relays the exact Auth-issued envelope across requests', () => {
  const issued = parseRustokSessionCookie('rustok-session='+envelope+'; Max-Age=3600');
  assert.equal(encodeRustokRefreshCredential(issued.credential), envelope);
  const next = parseRustokSessionCookie('rustok-session='+'a2'.repeat(32)+'.'+retrySeed+'; Max-Age=3600');
  assert.equal(next.credential.retrySeed, issued.credential.retrySeed);
  assert.notEqual(next.credential.token, issued.credential.token);
});

test('backend refresh cookie refuses ambiguous and noncanonical Max-Age', () => {
  for (const age of ['3600; Max-Age=3600', '1e3', '+3600', '03600']) {
    assert.equal(parseRustokSessionCookie('rustok-session='+envelope+'; Max-Age='+age), null);
  }
});


test('host cookie admission rejects duplicate names and orphan access credentials', () => {
  const credential = {token: refreshToken, retrySeed:'b1'.repeat(32)};
  const wire = encodeRustokRefreshCredential(credential);
  const pair = 'rustok-session='+wire;
  assert.deepEqual(parseRustokHostCookies(pair+'; rustok-access=access'), {
    accessToken:'access', refreshCredential:credential, invalid:false
  });
  assert.equal(parseRustokHostCookies(null).invalid, false);
  assert.equal(parseRustokHostCookies(pair).invalid, false);
  for (const value of [pair+'; '+pair, pair+'; rustok-access=a; rustok-access=b',
    'rustok-access=orphan', 'rustok-session='+refreshToken, pair+'; rustok-access=a,b']) {
    const result = parseRustokHostCookies(value);
    assert.equal(result.invalid, true);
    assert.equal(result.accessToken, null);
    assert.equal(result.refreshCredential, null);
  }
});

test('ambiguous refresh repeats once, never after confirmed rejection', async () => {
  let calls = 0;
  const ambiguous = new Error('unknown');
  const rejected = new Error('rejected');
  assert.equal(await retryAmbiguousRefresh(async () => {
    if (++calls === 1) throw ambiguous;
    return 'recovered';
  }, (error) => error === ambiguous), 'recovered');
  assert.equal(calls, 2);
  calls = 0;
  await assert.rejects(retryAmbiguousRefresh(async () => {
    calls++; throw rejected;
  }, (error) => error === ambiguous), (error) => error === rejected);
  assert.equal(calls, 1);
  calls = 0;
  await assert.rejects(retryAmbiguousRefresh(async () => {
    calls++; throw ambiguous;
  }, (error) => error === ambiguous), (error) => error === ambiguous);
  assert.equal(calls, 2);
});


test('direct access projection must match session, subject and tenant', () => {
  const sessionId = 'a1111111-1111-4111-8111-111111111111';
  const userId = 'b2222222-2222-4222-8222-222222222222';
  const tenantId = 'c3333333-3333-4333-8333-333333333333';
  const claims = {session_id:sessionId, sub:userId, tenant_id:tenantId, grant_type:'direct'};
  const wire = (value) => ['header',Buffer.from(JSON.stringify(value)).toString('base64url'),'signature'].join('.');
  assert.deepEqual(rustokDirectSessionClaims(wire(claims)), {sessionId,userId,tenantId});
  assert.equal(matchesRustokDirectSession(wire(claims),sessionId,userId,tenantId),true);
  assert.equal(matchesRustokDirectSession(wire(claims),tenantId,userId,tenantId),false);
  assert.equal(matchesRustokDirectSession(wire(claims),sessionId,tenantId,tenantId),false);
  assert.equal(matchesRustokDirectSession(wire(claims),sessionId,userId,userId),false);
  for (const value of ['00000000-0000-0000-0000-000000000000',sessionId.toUpperCase(),sessionId+'\n',null]) {
    assert.equal(isCanonicalSessionId(value),false);
    assert.equal(rustokDirectSessionClaims(wire({...claims,session_id:value})),null);
  }
  assert.equal(rustokDirectSessionClaims(wire({...claims,grant_type:'client_credentials'})),null);
});


test('generic Auth.js routes cannot bypass confirmed signout', () => {
  assert.equal(authRouteAction('GET',['signout']), 'logout');
  assert.equal(authRouteAction('POST',['signout']), 'logout');
  for (const segments of [['signout','credentials'], ['SIGNOUT'], ['callback/credentials'], ['callback','other'], ['unknown']]) {
    assert.equal(authRouteAction('POST',segments), 'reject');
  }
  for (const action of ['session','csrf','providers','signin','error']) {
    assert.equal(authRouteAction('GET',[action]), 'authjs');
  }
  assert.equal(authRouteAction('POST',['callback','credentials']), 'authjs');
  assert.equal(authRouteAction('POST',['session']), 'authjs');
});

const logoutSessionId = 'a1111111-1111-4111-8111-111111111111';
const logoutTenantId = 'b2222222-2222-4222-8222-222222222222';
function logoutRequest({origin='https://admin.example.test', expected=logoutSessionId, method='POST'}={}) {
  const headers = new Headers();
  if (origin !== null) headers.set('origin',origin);
  if (expected !== null) headers.set('x-rustok-expected-session-id',expected);
  return new Request('https://admin.example.test/api/auth/signout',{method,headers});
}
function logoutDependencies({revocationError, identity, invalid=false, missing=false, cleanupError=false}={}) {
  const calls = [];
  return {
    calls,
    dependencies: {
      async readIdentity() {
        calls.push('identity');
        return identity === undefined ? {sessionId:logoutSessionId,tenantId:logoutTenantId,tenantSlug:'shop'} : identity;
      },
      async readCookies() {
        calls.push('cookies');
        return {accessToken:null,refreshCredential:missing ? null : {token:refreshToken,retrySeed},invalid};
      },
      async revoke(credential,slug,id,tenantId) {
        calls.push('revoke');
        assert.deepEqual(credential,{token:refreshToken,retrySeed});
        assert.equal(slug,'shop');
        assert.equal(id,logoutSessionId);
        assert.equal(tenantId,logoutTenantId);
        if (revocationError) throw revocationError;
      },
      async clearApplicationSession() {
        calls.push('authjs');
        if (cleanupError) throw new Error('failed');
      },
      async clearRustokCookies() { calls.push('rustok'); }
    }
  };
}
test('logout confirms owner revocation before either cookie cleanup', async () => {
  const {calls,dependencies}=logoutDependencies();
  const response=await handleConfirmedLogout(logoutRequest(),dependencies);
  assert.equal(response.status,200);
  assert.deepEqual(await response.json(),{status:'ok'});
  assert.equal(response.headers.get('cache-control'),'no-store');
  assert.deepEqual(calls,['identity','cookies','revoke','authjs','rustok']);
});
test('logout rejects cross-origin, malformed, duplicate and stale intent before side effects', async () => {
  for (const [request,status] of [
    [logoutRequest({origin:null}),403],
    [logoutRequest({origin:'https://other.example.test'}),403],
    [logoutRequest({method:'GET'}),405],
    [logoutRequest({expected:null}),409],
    [logoutRequest({expected:logoutSessionId+', '+logoutSessionId}),409],
    [logoutRequest({expected:logoutTenantId}),409]
  ]) {
    const {calls,dependencies}=logoutDependencies();
    assert.equal((await handleConfirmedLogout(request,dependencies)).status,status);
    assert.ok(!calls.includes('cookies') && !calls.includes('revoke'));
  }
});
test('unconfirmed or conflicting revocation never clears cookies and can be retried', async () => {
  for (const [error,status] of [[new Error('timeout'),503],[{status:409},409],[{status:429},429]]) {
    const {calls,dependencies}=logoutDependencies({revocationError:error});
    assert.equal((await handleConfirmedLogout(logoutRequest(),dependencies)).status,status);
    assert.deepEqual(calls,['identity','cookies','revoke']);
  }
  for (const options of [{missing:true},{invalid:true},{identity:null}]) {
    const {calls,dependencies}=logoutDependencies(options);
    assert.equal((await handleConfirmedLogout(logoutRequest(),dependencies)).status,409);
    assert.ok(!calls.includes('revoke'));
  }
  const {calls,dependencies}=logoutDependencies({cleanupError:true});
  assert.equal((await handleConfirmedLogout(logoutRequest(),dependencies)).status,503);
  assert.deepEqual(calls,['identity','cookies','revoke','authjs']);
});


test('browser logout sends a frozen lineage and requires explicit success', async () => {
  let calls = 0;
  await requestConfirmedLogout(logoutSessionId, async (url, init) => {
    calls++;
    assert.equal(url,'/api/auth/signout');
    assert.equal(init.method,'POST');
    assert.equal(init.credentials,'same-origin');
    assert.equal(init.mode,'same-origin');
    assert.equal(init.cache,'no-store');
    assert.equal(init.redirect,'error');
    assert.equal(new Headers(init.headers).get('x-rustok-expected-session-id'),logoutSessionId);
    assert.ok(init.signal);
    return Response.json({status:'ok'});
  });
  assert.equal(calls,1);
  await assert.rejects(requestConfirmedLogout(null,async () => {
    calls++;
    throw new Error('must not send');
  }));
  assert.equal(calls,1);
  for (const response of [
    Response.json({status:'rejected'},{status:409}),
    Response.json({status:'ok'},{status:503}),
    Response.json({url:'/auth/sign-in'}),
    new Response(null,{status:204}),
    new Response('<html>signed out</html>',{status:200}),
    Response.json({status:'pending'})
  ]) {
    await assert.rejects(requestConfirmedLogout(logoutSessionId,async () => response));
  }
  await assert.rejects(requestConfirmedLogout(logoutSessionId,async () => {
    throw new Error('network outage');
  }));
});
