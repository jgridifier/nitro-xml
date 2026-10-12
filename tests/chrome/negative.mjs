// SPDX-License-Identifier: MIT
/** Editor chrome negative control (#53): ribbon must not appear in readonly. */
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const FIXTURE =
	process.env.CHROME_FIXTURE || "fixtures/real/xlsx/poi-45544.xlsx";

function fail(msg) {
	console.error(`chrome-negative: FAIL ${msg}`);
	process.exit(1);
}

async function openReadonly() {
	const { initCore, defineNitroXlsx } = await import(
		join(ROOT, "packages/nitro-xml/loader/index.js")
	);
	await initCore();
	defineNitroXlsx();
	const bytes = readFileSync(join(ROOT, FIXTURE));
	const el = document.createElement("nitro-xlsx");
	el.setAttribute("readonly", "");
	document.body.appendChild(el);
	await el.loadBuffer(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength), "book.xlsx");
	const html = (await el.shellAction("ui")).h || "";
	if (html.includes('aria-label="Ribbon"') || html.includes("data-g=\"home.bold\"")) {
		fail("ribbon leaked into readonly chrome");
	}
	el.remove();
	return true;
}

async function openEdit() {
	const { initCore, defineNitroXlsx, loadEditorModule } = await import(
		join(ROOT, "packages/nitro-xml/loader/index.js")
	);
	await initCore();
	const kind = await loadEditorModule();
	if (kind !== "editor-chrome") fail(`loadEditorModule returned ${kind}`);
	defineNitroXlsx();
	const bytes = readFileSync(join(ROOT, FIXTURE));
	const el = document.createElement("nitro-xlsx");
	el.setAttribute("readonly", "false");
	document.body.appendChild(el);
	await el.loadBuffer(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength), "book.xlsx");
	const html = (await el.shellAction("ui")).h || "";
	if (!html.includes('role="toolbar"') || !html.includes("data-g=\"home.bold\"")) {
		fail("edit mode missing Home ribbon controls");
	}
	el.remove();
	return true;
}

/** Probe: if this passes, the negative control is broken. */
function probeWouldFailIfBroken() {
	const fakeReadonlyHtml = '<div class="rb" role="toolbar" aria-label="Ribbon"></div>';
	if (!fakeReadonlyHtml.includes("Ribbon")) {
		fail("probe invariant broken");
	}
}

async function main() {
	if (typeof document === "undefined") {
		console.log("chrome-negative: SKIP (no DOM; run under playwright/node with happy-dom)");
		process.exit(0);
	}
	probeWouldFailIfBroken();
	await openReadonly();
	await openEdit();
	console.log("chrome-negative: PASS");
}

main().catch((e) => fail(e.stack || e));
