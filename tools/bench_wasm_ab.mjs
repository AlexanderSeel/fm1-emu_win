// Interleaved WASM A/B comparison with identical scheduled input.
// node tools/bench_wasm_ab.mjs A.wasm B.wasm [rounds=3] [firmware ids...]
// MS=6000 MHZ=240 FM1_BENCH_KEYS='...' customize the workload.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
const [a, b, count = '3', ...ids] = process.argv.slice(2);
const rounds = Number(count);
if (!a || !b || !Number.isInteger(rounds) || rounds < 1) throw new Error('A.wasm B.wasm [positive rounds] [firmwares...]');
const root = fileURLToPath(new URL('../', import.meta.url));
const median = xs => { const s = [...xs].sort((a, b) => a - b); const n = s.length; return (s[(n - 1) >> 1] + s[n >> 1]) / 2; };
for (const id of ids.length ? ids : ['melodee', 'felucca', 'x0x', 'stock']) {
  const pkg = id === 'stock' ? 'reference/FM-1-RE/firmware-images/v13/raw_fw/FM-1.fwsc' : `reference/firmwares/${id}.fwsc`;
  const samples = [[], []];
  for (let i = 0; i < rounds; i++) {
    for (const j of i % 2 ? [1, 0] : [0, 1]) {
      const run = spawnSync(process.execPath, ['web/bench.mjs', process.env.MHZ || '240', String(Number(process.env.MS || 6000) / 1000)], {
        cwd: root, encoding: 'utf8', env: { ...process.env, FM1_WASM: resolve([a, b][j]), FM1_FWSC: resolve(root, pkg) },
        timeout: 600_000, maxBuffer: 4 << 20 });
      if (run.error || run.status !== 0) throw new Error(`${id}: ${run.error || run.stderr || run.stdout}`);
      const result = JSON.parse(run.stdout.trim().split('\n').at(-1));
      samples[j].push(result);
    }
  }
  const rates = samples.map(r => median(r.map(x => x.steadyRealtime ?? x.realtime)));
  console.log(JSON.stringify({ firmware: id, a: rates[0], b: rates[1], ratio: rates[1] / rates[0], samples }));
}
