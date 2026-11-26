import assert from 'node:assert/strict';
import test from 'node:test';
import { isRustokAccessTokenExpiring, isSameOriginRequest, parseRustokSessionCookie, encodeRustokRefreshCredential, parseRustokRefreshCredential, parseRustokHostCookies, retryAmbiguousRefresh } from './auth-session.ts';

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
