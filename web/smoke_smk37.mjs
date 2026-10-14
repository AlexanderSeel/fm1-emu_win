// CI check of the SMK-37 page (web/build.sh DIR: DIR/smk37): its wasm boots GHOULBOX's SMK-37
// build, fetched from genkigenki's install page as the page does, as the SMK-37 board; draws a
// screen; a key held on the keybed sounds.
//   node web/smoke_smk37.mjs [web/dist]
import { readFileSync } from "node:fs";
const dir = new URL(`${process.argv[2] || "web/dist"}/smk37/`, `file://${process.cwd()}/`);
const { fetchPackage } = await import(new URL("fwsc.js", dir));
const cat = JSON.parse(readFileSync(new URL("firmwares.json", dir)));
const { bytes, url } = await fetchPackage(cat.firmwares.find((f) => f.id === "ghoulbox"));
const { jitImports } = await import(new URL("jit.js", dir));
let x;
const { instance } = await WebAssembly.instantiate(readFileSync(new URL("fm1_web.wasm", dir)), { env: jitImports(() => x) });
x = instance.exports;
x.fm1_jit(1, 16);
const p = x.fm1_alloc(bytes.length);
new Uint8Array(x.memory.buffer, p, bytes.length).set(bytes);
if (x.fm1_boot_fwsc(p, bytes.length, 0, 0, 100) !== 0) {
  console.error(`${url}: not an update package`);
  process.exit(1);
}
const level = (s) => {
  let sum = 0, n = 0;
  for (let i = 0; i < s * 44100; i += 128) {
    x.fm1_render(128, 50_000_000);
    for (const v of new Float32Array(x.memory.buffer, x.fm1_out_l(), 128)) { sum += v * v; n++; }
  }
  return Math.sqrt(sum / n);
};
const quiet = level(3);
const lcd = new Uint16Array(x.memory.buffer, x.fm1_lcd(), 240 * 240);
const lit = lcd.reduce((n, v) => n + (v ? 1 : 0), 0);
x.fm1_velocity(4000);
x.fm1_key(12, 1); // C4
const note = level(0.5);
console.log(`${url}: board ${x.fm1_board()}, ${lit} lit pixels; key C4: level ${quiet.toFixed(4)} -> ${note.toFixed(4)}`);
if (x.fm1_halted() || x.fm1_board() !== 1 || lit < 1000 || !(note > 10 * quiet + 0.001)) {
  console.error("smoke test FAILED");
  process.exit(1);
}
