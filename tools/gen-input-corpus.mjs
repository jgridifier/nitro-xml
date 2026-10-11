// SPDX-License-Identifier: MIT
// One-shot helper to record upstream parseCellInput outputs into the Rust corpus.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const mjs = new URL("../upstream/src/core/dist/xlsx/index.mjs", import.meta.url);
const { parseCellInput } = await import(mjs.href);

const texts = [
	["1,234", false],
	["1,234.5", false],
	["1,234,567.891", false],
	["$1,234.5", false],
	["$1234", false],
	["-$5", false],
	["$-5", false],
	["$ 12", false],
	["£5.25", false],
	["€5", false],
	["10%", false],
	["12.5%", false],
	["1,234%", false],
	["12 %", false],
	["1.5E+3", false],
	["1e5", false],
	["-1.5e-3", false],
	["(123)", false],
	["(1,234.50)", false],
	["  12  ", false],
	["+5", false],
	[".5", false],
	["00123", false],
	["-0", false],
	["0 1/2", false],
	["1 3/4", false],
	["2023-03-15", false],
	["2023/3/15", false],
	["3/15/2023", false],
	["3/15/23", false],
	["3-15-2023", false],
	["15-Mar-2023", false],
	["15 March 2023", false],
	["March 15, 2023", false],
	["Jan 2023", false],
	["Mar-2023", false],
	["9999-12-31", false],
	["1900-02-29", false],
	["13:45", false],
	["1:5", false],
	["1:45 PM", false],
	["12:30 am", false],
	["13:45:30", false],
	["8:30:15 PM", false],
	["25:00", false],
	["45:30.5", false],
	["10:30:00.5", false],
	["1:02:03.456", false],
	["0:30 am", false],
	["0:30 pm", false],
	["0:30:15 am", false],
	["Jan-32", false],
	["Jan 32", false],
	["Jan-99", false],
	["( 5 )", false],
	["($5)", false],
	[" 5 ", false],
	["3/15/2023 13:45", false],
	["2023-03-15 08:30", false],
	["1904-01-02", true],
	["1903-12-31", true],
	["12,34", false],
	["1.2.3", false],
	["5-", false],
	["5e", false],
	["1899-12-31", false],
	["2023-02-30", false],
	["hello", false],
	["13:75", false],
	["$1e3", false],
	["(-5)", false],
	["(+5)", false],
	["(-$5)", false],
	["-(5)", false],
	["TRUE ", false],
	[" true", false],
	["FALSE  ", false],
	[" #DIV/0!", false],
	["#N/A ", false],
	["13:30 pm", false],
	["Jan. 5", false],
	["=SUM(A1:A3)", false],
	["'00123", false],
	["'=1+1", false],
	["TRUE", false],
	["false", false],
	["#N/A", false],
	["#div/0!", false],
	["", false],
	["   ", false],
	["=", false],
];

function enc(v) {
	if (v === null) return { k: "z" };
	if (typeof v === "number") {
		const buf = new ArrayBuffer(8);
		new Float64Array(buf)[0] = v;
		const b = new BigUint64Array(buf)[0].toString(16).padStart(16, "0");
		return { k: "n", b };
	}
	if (typeof v === "boolean") return { k: "b", v };
	if (typeof v === "object" && v && "error" in v) return { k: "e", s: v.error };
	return { k: "t", s: String(v) };
}

const rows = [];
for (const [text, date1904] of texts) {
	const p = parseCellInput(text, { date1904 });
	rows.push({
		text,
		date1904,
		...enc(p.value),
		numFmt: p.numFmt ?? null,
		formula: p.formula ?? null,
	});
}

const out = fileURLToPath(
	new URL("../rs/crates/nitro-xml-xlsx/tests/fixtures/input_corpus.json", import.meta.url),
);
writeFileSync(out, JSON.stringify(rows, null, 2) + "\n");
console.log(`wrote ${rows.length} rows to ${out}`);
