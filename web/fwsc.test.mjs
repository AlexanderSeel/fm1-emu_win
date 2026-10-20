// fetchPackage: a Hub package that has gone (the Hub removes a firmware's old package when a new
// version comes) is found again through the Hub's catalog.json.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fetchPackage } from './fwsc.js';

test('a Hub package that has gone is fetched as the one the Hub lists now', async () => {
  const asked = [];
  const files = {
    'https://thefirmwarehub.com/catalog.json': JSON.stringify({ firmwares: [{ id: 'sloop', pkg: 'fw/sloop-2.7.1.fwsc' }] }),
    'https://thefirmwarehub.com/fw/sloop-2.7.1.fwsc': 'new',
  };
  const fetchFn = async (url) => {
    asked.push(url);
    const body = files[url];
    return body == null ? new Response('', { status: 404 }) : new Response(body);
  };
  const entry = { id: 'sloop', package: 'https://thefirmwarehub.com/fw/sloop-2.6.fwsc', hub: 'sloop' };
  const got = await fetchPackage(entry, fetchFn);
  assert.equal(got.url, 'https://thefirmwarehub.com/fw/sloop-2.7.1.fwsc');
  assert.equal(new TextDecoder().decode(got.bytes), 'new');
  await assert.rejects(fetchPackage({ id: 'x', package: 'https://thefirmwarehub.com/fw/x-1.fwsc', hub: 'x' }, fetchFn), /404/);
  // the Hub's catalog is read once
  assert.equal(asked.filter((u) => u.endsWith('catalog.json')).length, 1);
});
