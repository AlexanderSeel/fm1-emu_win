import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FLASH_SIZE, changedSectors, packSectors, validateSectors, packageHash, exportSave, importSave, SaveStore } from './persistence.js';
const sector = (at, value) => ({ at, data: new Uint8Array(4096).fill(value) });

test('sparse saves include erases, omit unchanged sectors, and remove reverted sectors', () => {
  const base = new Uint8Array(FLASH_SIZE).fill(17), flash = base.slice();
  assert.deepEqual(changedSectors(base, flash), []);
  flash.fill(255, 4096, 8192); flash[FLASH_SIZE - 1] = 12;
  const sectors = changedSectors(base, flash);
  assert.deepEqual(sectors.map(s => s.at), [4096, FLASH_SIZE - 4096]);
  const packed = packSectors(sectors);
  assert.equal(new DataView(packed.buffer).getUint32(0, true), 4096);
  assert.equal(packed[4], 255);
  flash.set(base);
  assert.deepEqual(changedSectors(base, flash), []);
});
test('sector input rejects duplicates, bounds, truncation and misalignment', () => {
  for (const sectors of [[sector(0, 1), sector(0, 2)], [sector(-4096, 0)], [sector(1, 0)],
    [sector(FLASH_SIZE, 0)], [{ at: 0, data: new Uint8Array(4095) }]]) {
    assert.throws(() => validateSectors(sectors));
  }
});
test('backup round trip includes exact firmware, large base64 data and erased flash', async () => {
  const pkg = new Uint8Array(1 << 20).fill(123);
  const record = { hash: await packageHash(pkg), package: pkg, name: 'test', sectors: [sector(4096, 255)] };
  const restored = await importSave(exportSave(record));
  assert.deepEqual(restored.package, pkg);
  assert.deepEqual(restored.sectors, record.sectors);
  const changedPackage = pkg.slice(); changedPackage[0] = 0;
  assert.notEqual(await packageHash(changedPackage), record.hash);
});
test('backups reject modified firmware, unsupported versions and malformed sectors', async () => {
  const pkg = new Uint8Array(960);
  const record = { hash: await packageHash(pkg), package: pkg, name: 'test', sectors: [sector(0, 0)] };
  const valid = JSON.parse(exportSave(record));
  for (const bad of [{ ...valid, version: 2 }, { ...valid, hash: 'wrong' },
    { ...valid, sectors: [valid.sectors[0], valid.sectors[0]] },
    { ...valid, sectors: [{ at: 1, data: valid.sectors[0].data }] },
    { ...valid, sectors: [{ at: 0, data: 'AA==' }] }]) {
    await assert.rejects(importSave(JSON.stringify(bad)));
  }
});
test('storage unavailability is an explicit failure', async () => {
  await assert.rejects(new SaveStore(null).get('packages', 'test'), /unavailable/);
});

// A deliberately controlled transaction: prove we await commit, not request success, and
// abort before writing any objects when an optimistic revision conflicts with another tab.
function transactionStore(existing) {
  let tx, request, puts = 0;
  const store = new SaveStore(null);
  store.open = async () => ({ transaction() {
    tx = { objectStore() { return { get() { request = {}; return request; }, put() { puts++; } }; },
      abort() { queueMicrotask(() => tx.onabort()); } };
    queueMicrotask(() => { request.result = existing; request.onsuccess(); });
    return tx;
  } });
  return { store, tx: () => tx, puts: () => puts };
}
test('save succeeds only at transaction commit and propagates aborts', async () => {
  const h = transactionStore(null);
  let done = false;
  const pending = h.store.save({ hash: 'test', revision: 0, package: new Uint8Array(960), sectors: [] }, 'fm1.local');
  pending.then(() => { done = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(done, false);
  assert.equal(h.puts(), 3);
  h.tx().oncomplete();
  assert.equal(await pending, 1);
  const failed = transactionStore(null);
  const failure = failed.store.save({ hash: 'test', revision: 0, package: new Uint8Array(960), sectors: [] }, 'fm1.local');
  await new Promise(resolve => setImmediate(resolve));
  failed.tx().error = new Error('quota exceeded');
  failed.tx().onabort();
  await assert.rejects(failure, /quota exceeded/);
});
test('a stale tab cannot overwrite a newer save', async () => {
  const h = transactionStore({ revision: 3 });
  await assert.rejects(h.store.save({ hash: 'test', revision: 2, package: new Uint8Array(960), sectors: [] }, 'fm1.local'), /another tab/);
  assert.equal(h.puts(), 0);
});
