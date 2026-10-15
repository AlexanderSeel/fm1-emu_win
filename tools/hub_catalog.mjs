// The FM-1 page's catalog (web/firmwares.json) from the FM-1 Firmware Hub's (thefirmwarehub.com,
// zvenson's): every firmware it lists that runs here, its package fetched from the Hub (whose
// packages never change under their name, sent with Access-Control-Allow-Origin: *). Firmwares
// the Hub does not list (M-VAVE's own, FM-1 SID, ...) stay as they are; for one it lists, what
// is ours is kept (the page's description, the licence, the installer link) and the rest comes
// from the Hub. The Hub's catalog changes every few hours: run this when it has.
//   node tools/hub_catalog.mjs [--check] [--catalog=hub.json]
// (--check: only say whether the file is current; --catalog: the Hub's catalog from a file)
import { readFileSync, writeFileSync } from "node:fs";

const HUB = "https://thefirmwarehub.com/";
const OUT = new URL("../web/firmwares.json", import.meta.url);
// our ids where the Hub's differ (the page takes either in ?fw=)
const OURS = { salt: "felucca-salt", floyd: "sloop-floyd", sloopbanks: "sloop-banks" };
// listed on the Hub, not run here yet
const SKIP = { bubba: "boots to a black screen, no sound" };
// first in the list (the page's default first), then by id
const FIRST = ["mvave", "felucca", "felucca-salt"];

const arg = process.argv.find((a) => a.startsWith("--catalog="));
const hub = arg ? JSON.parse(readFileSync(arg.slice(10))) : await (await fetch(HUB + "catalog.json")).json();
const cur = JSON.parse(readFileSync(OUT));
const old = new Map(cur.firmwares.map((f) => [f.id, f]));

// a repository's licence (GitHub's SPDX id), for firmwares new to the list
async function licence(repo) {
  try {
    const r = await fetch(`https://api.github.com/repos/${repo}`, { headers: { "User-Agent": "fm1-emu hub_catalog" } });
    const j = await r.json();
    const id = j.license?.spdx_id;
    return id && id !== "NOASSERTION" ? id : "see the repository";
  } catch {
    return "see the repository";
  }
}

const firmwares = [];
const listed = new Set();
for (const h of hub.firmwares) {
  if (!h.pkg) continue;
  const id = OURS[h.id] || h.id;
  listed.add(id);
  if (SKIP[h.id]) continue;
  const o = old.get(id) || {};
  firmwares.push({
    id,
    name: h.name,
    version: String(h.version),
    author: o.author || h.author,
    license: o.license || (h.repo ? await licence(h.repo) : "see the repository"),
    about: o.about || h.what,
    repo: o.repo || (h.repo ? `https://github.com/${h.repo}` : h.site),
    package: new URL(h.pkg, HUB).href,
    ...(o.installer ? { installer: o.installer } : {}),
    hub: h.id,
  });
}
for (const f of cur.firmwares) {
  if (!listed.has(f.id)) firmwares.push(f);
}
const rank = (f) => (FIRST.includes(f.id) ? FIRST.indexOf(f.id) : FIRST.length);
firmwares.sort((a, b) => rank(a) - rank(b) || a.id.localeCompare(b.id));

// The file's layout, as it was written by hand: an entry a few lines, `{ "id": ...` opening each
// (fm1-live's small reader scans for that, and `"key": "value"`), the description and the source
// on lines of their own.
const fields = (f, keys) => keys.filter((k) => k in f).map((k) => `"${k}": ${JSON.stringify(f[k])}`).join(", ");
const entry = (f) => {
  const head = ["id", "name", "version", "author", "license"], own = ["about", "repo"];
  const rest = Object.keys(f).filter((k) => !head.includes(k) && !own.includes(k));
  return [`    { ${fields(f, head)},`, ...own.filter((k) => k in f).map((k) => `      ${fields(f, [k])},`), `      ${fields(f, rest)} }`]
    .join("\n").replace(/,\n      \s*}$/, " }");
};
const note = "Community firmwares for the M-VAVE FM-1: the FM-1 Firmware Hub's list (thefirmwarehub.com, " +
  "tools/hub_catalog.mjs; its packages fetched from the Hub), plus a few more (fm1-editor.com/firmware) " +
  "and this repository's FM-1 SID (fm1-sid/). The web page fetches each package when started and " +
  "unpacks it with web/fwsc.js; tools/fetch_firmwares.mjs does the same into reference/firmwares for " +
  "fm1-live. \"hub\": the firmware's id on the Hub (its card: thefirmwarehub.com/#fw-<hub>).";
const json = `{\n  "note": ${JSON.stringify(note)},\n  "default": ${JSON.stringify(cur.default)},\n  "firmwares": [\n` +
  firmwares.map(entry).join(",\n") + "\n  ]\n}\n";
JSON.parse(json); // (still JSON)

if (process.argv.includes("--check")) {
  let now = "";
  try { now = readFileSync(OUT, "utf8"); } catch {}
  console.log(now === json ? "web/firmwares.json: current with the Hub" : "web/firmwares.json: the Hub has changed (node tools/hub_catalog.mjs)");
  process.exit(now === json ? 0 : 1);
}
writeFileSync(OUT, json);
console.log(`web/firmwares.json: ${firmwares.length} firmwares (Hub catalog of ${hub.updated})`);
for (const f of firmwares) {
  const o = old.get(f.id);
  const what = !o ? "new" : o.version !== f.version ? `${o.version} -> ${f.version}` : "";
  console.log(`  ${f.id.padEnd(14)} ${f.version.padEnd(20)} ${f.hub ? "hub" : "   "} ${what}`);
}
for (const [id, why] of Object.entries(SKIP)) console.log(`  (not listed: ${id}: ${why})`);
