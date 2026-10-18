// Deterministic AudioWorklet harness: no audio device or browser required.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
function harness() {
  let Processor, time = 0, revision = 0, flashRevision = 0, busy = false;
  const messages = [];
  const memory = new WebAssembly.Memory({ initial: 4 });
  const context = vm.createContext({ WebAssembly, Float32Array, Uint16Array, Uint32Array, Uint8Array,
    sampleRate: 44100, performance: { now: () => time },
    AudioWorkletProcessor: class { constructor() { this.port = { postMessage(m) { messages.push(m); time += 2; } }; } },
    registerProcessor(_, cls) { Processor = cls; } });
  vm.runInContext(readFileSync(new URL('./worklet.js', import.meta.url), 'utf8'), context);
  const p = new Processor();
  Object.assign(p, { x: { memory, fm1_render(n) { time += .1; return n; },
    fm1_out_l: () => 0, fm1_out_r: () => 4096, fm1_lcd: () => 8192,
    fm1_lcd_revision: () => revision, fm1_leds: () => 131072,
    fm1_flash_revision: () => flashRevision, fm1_flash_busy: () => busy,
    fm1_flash: () => 140000, fm1_flash_len: () => 4096,
    fm1_ms: () => 1000, fm1_minsns: () => 10, fm1_halted: () => false },
    budget: 100000, board: 0, short: 0, reportFrames: 0, reportCalls: 0,
    busy: 0, worst: 0, lcdRevision: null, seq: 1, flashRevision: 0, observedRevision: 0,
    flashChangedAt: 0, lastSnapshot: 0 });
  const render = () => p.process([], [[new Float32Array(128), new Float32Array(128)]]);
  return { p, messages, memory, render, change: () => revision++, advance: ms => time += ms,
    flashChange: () => flashRevision++, flashBusy: value => busy = value };
}
test('unchanged LCD is omitted; changed partial frame is sent at capped rate', () => {
  const h = harness();
  for (let i = 0; i < 24; i++) h.render();
  assert.equal(h.messages.length, 2);
  assert.equal(h.messages[0].lcd.length, 57600);
  assert.equal(h.messages[1].lcd, null);
  h.change();
  for (let i = 0; i < 12; i++) h.render();
  assert.equal(h.messages[2].lcd.length, 57600);
});
test('timing includes postMessage; audio views reused and refreshed on memory growth', () => {
  const h = harness(); h.render(); const views = h.p.views;
  h.render(); assert.equal(h.p.views, views);
  h.memory.grow(1); h.render(); assert.notEqual(h.p.views, views);
  for (let i = 3; i < 24; i++) h.render();
  assert.ok(Math.abs(h.messages[1].busy - 3.2) < 1e-8);
  assert.ok(Math.abs(h.messages[1].worst - 2.1) < 1e-8);
  assert.equal(h.messages[1].quanta, 12);
});

test('snapshot copies flash, preserves boot identity and waits for an SPI command', async () => {
  const h = harness();
  new Uint8Array(h.memory.buffer, 140000, 4096).fill(42);
  h.advance(500);
  h.flashBusy(true);
  await h.p.port.onmessage({ data: { type: 'snapshot', seq: 1, request: 7, pause: true } });
  assert.equal(h.messages.length, 0);
  h.flashBusy(false);
  h.p.flushSnapshots();
  assert.equal(h.messages[0].type, 'snapshot');
  assert.equal(h.messages[0].seq, 1);
  assert.equal(h.messages[0].request, 7);
  assert.equal(h.messages[0].flash[0], 42);
  new Uint8Array(h.memory.buffer, 140000, 4096).fill(13);
  assert.equal(h.messages[0].flash[0], 42);
  assert.equal(h.p.paused, true);
  await h.p.port.onmessage({ data: { type: 'resume' } });
  assert.equal(h.p.paused, false);
  await h.p.port.onmessage({ data: { type: 'snapshot', seq: 0, request: 8 } });
  assert.equal(h.messages.length, 1);
});
test('autosave waits for quiet flash and does not resend unchanged flash', () => {
  const h = harness();
  h.flashChange(); h.advance(1100);
  for (let i = 0; i < 12; i++) h.render();
  assert.equal(h.messages.filter(m => m.type === 'snapshot').length, 0);
  h.advance(300);
  for (let i = 0; i < 12; i++) h.render();
  assert.equal(h.messages.filter(m => m.type === 'snapshot').length, 1);
  h.advance(2000);
  for (let i = 0; i < 12; i++) h.render();
  assert.equal(h.messages.filter(m => m.type === 'snapshot').length, 1);
});

test('manual snapshot acknowledges a revision before the next frame report', async () => {
  const h = harness();
  h.flashChange();
  await h.p.port.onmessage({ data: { type: 'snapshot', seq: 1, request: 9 } });
  assert.equal(h.p.observedRevision, 1);
  for (let i = 0; i < 12; i++) h.render();
  assert.equal(h.messages.filter(m => m.type === 'flash-changed').length, 1);
});
test('an invalid replacement package leaves the previous machine available', async () => {
  const h = harness(), previous = h.p.x;
  h.p.paused = true;
  await h.p.port.onmessage({ data: { type: 'boot', seq: 2, wasm: new Uint8Array([0]).buffer } });
  assert.equal(h.p.x, previous);
  assert.equal(h.p.paused, false);
  assert.equal(h.messages[0].type, 'error');
  assert.equal(h.messages[0].seq, 2);
});

test('replacement snapshots wait for a quiet interval and timed-out requests can be cancelled', async () => {
  const h = harness();
  h.flashChange();
  await h.p.port.onmessage({ data: { type: 'snapshot', seq: 1, request: 10, pause: true } });
  assert.equal(h.messages.filter(m => m.type === 'snapshot').length, 0);
  h.advance(300); h.p.flushSnapshots();
  assert.equal(h.messages.filter(m => m.type === 'snapshot').length, 1);
  const pending = harness(); pending.flashBusy(true);
  await pending.p.port.onmessage({ data: { type: 'snapshot', seq: 1, request: 11, pause: true } });
  await pending.p.port.onmessage({ data: { type: 'cancel-snapshot', request: 11 } });
  pending.flashBusy(false); pending.advance(500); pending.p.flushSnapshots();
  assert.equal(pending.p.paused, undefined);
  assert.equal(pending.messages.length, 0);
});
