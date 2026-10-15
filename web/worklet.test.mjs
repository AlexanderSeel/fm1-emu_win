// Deterministic AudioWorklet harness: no audio device or browser required.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
function harness() {
  let Processor, time = 0, revision = 0;
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
    fm1_ms: () => 1000, fm1_minsns: () => 10, fm1_halted: () => false },
    budget: 100000, board: 0, short: 0, reportFrames: 0, reportCalls: 0,
    busy: 0, worst: 0, lcdRevision: null });
  const render = () => p.process([], [[new Float32Array(128), new Float32Array(128)]]);
  return { p, messages, memory, render, change: () => revision++ };
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
