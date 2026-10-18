// Page lifecycle regression tests with deterministic DOM, storage and audio boundaries.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as persistence from './persistence.js';
const settle = () => new Promise(resolve => setImmediate(resolve));
class Element {
  constructor(text = '', value = '') {
    this.text = this.textContent = text; this.value = String(value); this.options = []; this.listeners = {};
    this.dataset = {}; this.style = { setProperty() {} }; this.classList = { add() {}, remove() {}, toggle() {} };
    this.parentElement = this;
  }
  add(e, at = this.options.length) { this.options.splice(at, 0, e); }
  addEventListener(type, fn) { this.listeners[type] = fn; }
  append() {} replaceChildren() {} setAttribute() {} blur() {} click() {}
  hasAttribute() { return false; }
  querySelector() { return new Element(); }
  getBoundingClientRect() { return { top: 0 }; }
  getContext() { return {}; }
}
function storage() {
  return { records: new Map(), entries: new Map(), writes: [], fail: false,
    async entry(key) { return this.load(this.entries.get(key)); },
    async load(hash) { return structuredClone(this.records.get(hash) || null); },
    async get() { return null; },
    async save(record, entry) {
      if (this.fail) throw new Error('quota exceeded');
      assert.equal(record.revision, this.records.get(record.hash)?.revision || 0, 'stale write');
      const revision = record.revision + 1;
      this.records.set(record.hash, structuredClone({ ...record, revision }));
      this.entries.set(entry, record.hash); this.writes.push({ entry, record: structuredClone(record) });
      return revision;
    } };
}
async function page(disk = storage(), initialSearch = '', selected = '') {
  const elements = new Map();
  const document = { documentElement: new Element(),
    getElementById(id) { if (!elements.has(id)) elements.set(id, new Element()); return elements.get(id); },
    createElement() { return new Element(); }, querySelector() { return new Element(); }, addEventListener() {} };
  const cat = { default: 'a', firmwares: ['a', 'b'].map(id => ({ id, name: id, version: '1',
    package: `https://example.test/${id}.fwsc`, author: 'test', license: 'MIT', about: '' })) };
  let audio;
  class AudioContext {
    constructor() { this.state = 'running'; this.audioWorklet = { addModule: async () => {} }; }
    async resume() { this.state = 'running'; } async close() { this.state = 'closed'; }
  }
  class AudioWorkletNode {
    constructor() {
      audio = this; this.messages = [];
      this.port = { postMessage: message => {
        this.messages.push(message);
        queueMicrotask(() => {
          if (message.type === 'boot') {
            this.seq = message.seq;
            this.base = new Uint8Array(persistence.FLASH_SIZE).fill(170);
            this.flash = this.base.slice();
            const packed = new Uint8Array(message.sectors), view = new DataView(packed.buffer);
            for (let o = 0; o < packed.length; o += 4100) this.flash.set(packed.subarray(o + 4, o + 4100), view.getUint32(o, true));
            if (message.syx) this.flash.set(new Uint8Array(message.syx), 0x90000);
            this.port.onmessage({ data: { type: 'booted', seq: this.seq, base: this.base, flash: this.flash.slice(), board: 0 } });
          } else if (message.type === 'snapshot') {
            this.port.onmessage({ data: { type: 'snapshot', seq: this.seq, request: message.request, flash: this.flash.slice() } });
          }
        });
      } };
    }
    connect() {} disconnect() {}
  }
  const local = new Map(selected ? [['fm1.fw', selected]] : []);
  const location = { search: initialSearch, origin: 'http://localhost', pathname: '/' };
  const environment = { document, location, history: { replaceState(_a, _b, url) { location.search = url; } },
    localStorage: { getItem: key => local.get(key), setItem: (key, value) => local.set(key, value) },
    ResizeObserver: class { observe() {} }, scrollY: 0, Option: Element, navigator: {},
    fetch: async url => ({ json: async () => url === 'smk37/firmwares.json' ? { firmwares: [] } : cat,
      arrayBuffer: async () => new ArrayBuffer(0) }),
    WebAssembly: { instantiate: async () => ({ instance: { exports: { panel_board() {}, panel_width: () => 100, panel_height: () => 100 } } }) },
    requestAnimationFrame() {}, addEventListener() {}, AudioContext, AudioWorkletNode,
    matchMedia: () => ({ matches: false }), confirm: () => true,
    fetchPackage: async f => ({ bytes: new Uint8Array(960).fill(f.id === 'a' ? 1 : 2), url: f.package }),
    ...persistence, SaveStore: class { constructor() { return disk; } } };
  const source = readFileSync(new URL('./main.js', import.meta.url), 'utf8').replace(/^import .*;$/gm, '');
  const run = new Function(...Object.keys(environment), `return (async () => { ${source}\nreturn {
    startAudio, boot, flushRunning, setLatency, get running() { return running; },
    setFirmware(id) { fw = byId.get(id); fwSel.value = id; }, setBank(bytes) { bank = bytes; },
    get fw() { return fw; }
  }; })()`);
  const api = await run(...Object.values(environment));
  return { api, disk, elements, location, get audio() { return audio; } };
}
test('firmware switch, clock reboot and audio-context replacement retain the latest flash', async () => {
  const p = await page(); await p.api.startAudio(); await p.api.boot(); await settle();
  p.audio.flash[0x74000] = 19;
  p.api.setFirmware('b'); await p.api.boot();
  assert.equal(p.disk.writes.find(w => w.entry === 'fm1.a' && w.record.sectors.length).record.sectors[0].data[0], 19);
  p.api.setFirmware('a'); await p.api.boot();
  assert.equal(p.audio.flash[0x74000], 19);
  p.audio.flash[0x74001] = 20;
  await p.api.boot(); assert.equal(p.audio.flash[0x74001], 20);
  await p.api.setLatency('playback'); assert.equal(p.audio.flash[0x74001], 20);
});
test('DX7 seed is consumed once; later edits to imported voices survive a reboot', async () => {
  const p = await page(); await p.api.startAudio();
  p.api.setBank(new Uint8Array([13, 14])); await p.api.boot();
  assert.equal(p.audio.flash[0x90000], 13);
  p.audio.flash[0x90000] = 27;
  await p.api.boot();
  assert.equal(p.audio.messages.at(-1).syx, null);
  assert.equal(p.audio.flash[0x90000], 27);
});
test('backup import replaces the same running hash without a stale revision and restores local selection', async () => {
  const p = await page(); await p.api.startAudio(); await p.api.boot(); await settle();
  const record = { ...p.api.running.record, sectors: [{ at: 0x74000, data: new Uint8Array(4096).fill(53) }] };
  const input = p.elements.get('import-save');
  input.files = [{ size: 1000, text: async () => persistence.exportSave(record) }];
  await input.listeners.change(); await settle();
  assert.equal(p.audio.flash[0x74000], 53);
  assert.equal(p.elements.get('save-status').textContent, 'Saved in this browser');
  const reloaded = await page(p.disk, '?fw=local', 'local');
  assert.equal(reloaded.api.fw.id, 'local');
  await reloaded.api.startAudio(); await reloaded.api.boot();
  assert.equal(reloaded.audio.flash[0x74000], 53);
});
test('failed storage retains changes in memory across a reboot and exposes an error', async () => {
  const p = await page(); await p.api.startAudio(); await p.api.boot(); await settle();
  p.disk.fail = true; p.audio.flash[0x74000] = 83;
  await p.api.boot(); await settle();
  assert.equal(p.audio.flash[0x74000], 83);
  assert.match(p.elements.get('save-status').textContent, /quota exceeded/);
});

test('reset removes only the selected firmware differences', async () => {
  const p = await page(); await p.api.startAudio(); await p.api.boot();
  p.audio.flash[0x74000] = 10;
  p.api.setFirmware('b'); await p.api.boot();
  p.audio.flash[0x74000] = 11;
  p.api.setFirmware('a'); await p.api.boot();
  await p.api.boot({ reset: true });
  assert.equal(p.audio.flash[0x74000], 170);
  p.api.setFirmware('b'); await p.api.boot();
  assert.equal(p.audio.flash[0x74000], 11);
});
