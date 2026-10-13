// The README's table of firmwares, from web/firmwares.json: each name a link that opens the page
// with that firmware picked (?fw=id), its author linked to the firmware's source. Rewrites the
// lines between the two markers in README.md; --check only says whether they are current.
//   node tools/readme_firmwares.mjs [--check]
import { readFileSync, writeFileSync } from "node:fs";

const PAGE = "https://joakimeriksson.github.io/fm1-emu/";
const BEGIN = "<!-- firmwares: node tools/readme_firmwares.mjs -->";
const END = "<!-- /firmwares -->";

const readme = new URL("../README.md", import.meta.url);
const cat = JSON.parse(readFileSync(new URL("../web/firmwares.json", import.meta.url)));
const cell = (s) => s.replace(/\|/g, "\\|");
// the first sentence of the page's text: a ". " ends it, a ":" inside it does not
const blurb = (f) => (f.about.match(/^.*?\.(?=\s|$)/) || [f.about])[0];

const rows = cat.firmwares.map((f) => {
  const name = `[${cell(`${f.name} ${f.version}`.trim())}](${PAGE}?fw=${f.id})`;
  const by = f.repo ? `[${cell(f.author)}](${f.repo})` : cell(f.author);
  return `| ${name} | ${by} | ${cell(blurb(f))}${f.byo ? " Bring your own `FM-1.fwsc`." : ""} |`;
});
const table = ["| Firmware (opens it on the page) | By | What it is |", "|---|---|---|", ...rows].join("\n");

const text = readFileSync(readme, "utf8");
const a = text.indexOf(BEGIN), b = text.indexOf(END);
if (a < 0 || b < a) throw new Error(`README.md has no ${BEGIN} ... ${END} section`);
const next = `${text.slice(0, a + BEGIN.length)}\n${table}\n${text.slice(b)}`;
if (process.argv.includes("--check")) {
  if (next !== text) {
    console.error("README.md's firmware table is out of date: node tools/readme_firmwares.mjs");
    process.exit(1);
  }
  console.log(`README.md: ${rows.length} firmwares, current`);
} else {
  writeFileSync(readme, next);
  console.log(`README.md: ${rows.length} firmwares`);
}
