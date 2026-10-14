// The README's tables of firmwares, from web/firmwares.json (the FM-1) and web/smk37/firmwares.json
// (the SMK-37): each name a link that opens its page with that firmware picked (?fw=id), its author
// linked to the firmware's source. Rewrites the lines between each table's two markers in
// README.md; --check only says whether they are current.
//   node tools/readme_firmwares.mjs [--check]
import { readFileSync, writeFileSync } from "node:fs";

const SITE = "https://joakimeriksson.github.io/fm1-emu/";
const readme = new URL("../README.md", import.meta.url);
const cell = (s) => s.replace(/\|/g, "\\|");
// the first sentence of the page's text: a ". " ends it, a ":" inside it does not
const blurb = (f) => (f.about.match(/^.*?\.(?=\s|$)/) || [f.about])[0];

const TABLES = [
  { catalog: "../web/firmwares.json", page: SITE, stock: "FM-1.fwsc", marker: "firmwares" },
  { catalog: "../web/smk37/firmwares.json", page: `${SITE}smk37/`, stock: "SMK-37 Pro_016.fwsc", marker: "smk37-firmwares" },
];

let text = readFileSync(readme, "utf8");
const next0 = text;
const counts = [];
for (const t of TABLES) {
  const cat = JSON.parse(readFileSync(new URL(t.catalog, import.meta.url)));
  const rows = cat.firmwares.map((f) => {
    const name = `[${cell(`${f.name} ${f.version}`.trim())}](${t.page}?fw=${f.id})`;
    const who = f.porter ? `${f.author} (port: ${f.porter})` : f.author;
    const by = f.repo ? `[${cell(who)}](${f.repo})` : cell(who);
    return `| ${name} | ${by} | ${cell(blurb(f))}${f.byo ? ` Bring your own \`${t.stock}\`.` : ""} |`;
  });
  const table = ["| Firmware (opens it on the page) | By | What it is |", "|---|---|---|", ...rows].join("\n");
  const BEGIN = `<!-- ${t.marker}: node tools/readme_firmwares.mjs -->`, END = `<!-- /${t.marker} -->`;
  const a = text.indexOf(BEGIN), b = text.indexOf(END);
  if (a < 0 || b < a) throw new Error(`README.md has no ${BEGIN} ... ${END} section`);
  text = `${text.slice(0, a + BEGIN.length)}\n${table}\n${text.slice(b)}`;
  counts.push(`${rows.length} ${t.marker}`);
}
if (process.argv.includes("--check")) {
  if (text !== next0) {
    console.error("README.md's firmware tables are out of date: node tools/readme_firmwares.mjs");
    process.exit(1);
  }
  console.log(`README.md: ${counts.join(", ")}, current`);
} else {
  writeFileSync(readme, text);
  console.log(`README.md: ${counts.join(", ")}`);
}
