// Every firmware in web/firmwares.json still downloads and unpacks:  node web/check_firmwares.mjs
import { readFileSync } from "node:fs";
const { appFromPackage, fetchPackage } = await import(new URL("./fwsc.js", import.meta.url));
const cat = JSON.parse(readFileSync(new URL("./firmwares.json", import.meta.url)));
// node follows an HTTP redirect where a browser stops: GitHub Pages' 301 to an author's own
// domain has no CORS header, so the page fails to fetch what this check downloads fine
let redirects = [];
const fetch0 = globalThis.fetch;
globalThis.fetch = async (u, o) => {
  const res = await fetch0(u, o);
  if (res.redirected) redirects.push(`${u} -> ${res.url}`);
  return res;
};
let bad = 0;
for (const f of cat.firmwares) {
  if (f.byo) continue;                     // brought by the user (M-VAVE's own), not fetched
  try {
    redirects = [];
    const { bytes, url } = await fetchPackage(f);
    if (redirects.length) throw new Error(`redirected (a browser can't follow it): ${redirects.join(", ")}`);
    const app = appFromPackage(bytes);
    const moved = url === (f.package || f.installer) ? "" : `  (moved on: ${url})`;   // embedded: the page
    console.log(`${f.id.padEnd(13)} ${String(app.length).padStart(7)} bytes${moved}`);
  } catch (e) {
    bad++;
    console.log(`${f.id.padEnd(13)} FAILED ${e.message}`);
  }
}
process.exit(bad ? 1 : 0);
