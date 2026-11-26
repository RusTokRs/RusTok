import assert from 'node:assert/strict';
import test from 'node:test';
import { isRustokAccessTokenExpiring, isSameOriginRequest, parseRustokSessionCookie } from './auth-session.ts';

const refreshToken = '0123456789abcdef'.repeat(4);
function jwt(exp) {
  return ['header', Buffer.from(JSON.stringify({exp})).toString('base64url'), 'signature'].join('.');
}

test('accepts only a canonical 256-bit refresh cookie with bounded Max-Age', () => {
  assert.deepEqual(parseRustokSessionCookie('rustok-session='+refreshToken+'; Path=/api/auth/session; HttpOnly; Max-Age=3600'), {value:refreshToken,maxAge:3600});
  for (const value of [
    null,
    'other='+refreshToken+'; Max-Age=3600',
    'rustok-session=bad; Max-Age=3600',
    'rustok-session='+refreshToken+'; Max-Age=0',
    'rustok-session='+refreshToken+'; Max-Age=999999999'
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
