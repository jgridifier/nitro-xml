// SPDX-License-Identifier: MIT
// Bundles every JS adapter for the browser with esbuild, fetches the pinned PPTXjs sources, and
// writes dist/bundles/status.json (which bundles built, with the error when one did not).
// Prerequisites: `npm ci` here, upstream built (upstream/src/core/dist), Rust shims built
// (adapters/rust/build.sh). A missing prerequisite marks that entrant "not entered"; it never
// stops the build of the others.
import { build } from 'esbuild';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..');
const upstreamDist = join(repo, 'upstream', 'src', 'core', 'dist');
const out = join(here, 'dist', 'bundles');
mkdirSync(out, { recursive: true });

// PPTXjs is not on npm: fetch the pinned commit's files and verify each one.
const PPTXJS_SHA = '1a9260b2062f89ba822aeb54da792236780af8e7';
const PPTXJS_FILES = {
	'js/jquery-1.11.3.min.js': 'ecb916133a9376911f10bc5c659952eb0031e457f5df367cde560edbfba38fb8',
	'js/jszip.min.js': '215fb2537b13d82daabd46e1ee59ffe4dce90abd0acb0ac5432e77071f422e9c',
	'js/filereader.js': '96df9b7a2c5801e64fbf4917ea8d10167e22b90de17767352e286e8202737079',
	'js/d3.min.js': '46c05e6969d585cd0ecb89f5de826501e7aa574f416a4a1823a500ae0a8254e3',
	'js/nv.d3.min.js': 'a88a6fcf2999cce9654cb119e8c2aa50872cc55da25341099b872f1a788ce233',
	'js/dingbat.js': '9c1e836e97bf59bef1246addcc8028744a3abfa4dae82fcf90c0a44414362bf1',
	'js/pptxjs.js': '108fb8dca723240f98387d46507dee372fb1146cab71e42678a555e7986ac35e',
};

async function fetchPptxjs() {
	const dir = join(here, 'vendor', 'pptxjs');
	mkdirSync(dir, { recursive: true });
	const parts = [];
	for (const [file, sha] of Object.entries(PPTXJS_FILES)) {
		const local = join(dir, file.replace('/', '_'));
		let bytes = existsSync(local) ? readFileSync(local) : null;
		if (!bytes) {
			const res = await fetch(`https://raw.githubusercontent.com/meshesha/PPTXjs/${PPTXJS_SHA}/${file}`);
			if (!res.ok) throw new Error(`PPTXjs ${file}: HTTP ${res.status}`);
			bytes = Buffer.from(await res.arrayBuffer());
			writeFileSync(local, bytes);
		}
		const got = createHash('sha256').update(bytes).digest('hex');
		if (got !== sha) throw new Error(`PPTXjs ${file}: sha256 ${got} != pinned ${sha}`);
		parts.push(`/* ${file} @ meshesha/PPTXjs ${PPTXJS_SHA} */\n${bytes.toString('utf8')}\n;`);
	}
	writeFileSync(join(out, 'pptxjs-vendor.js'), parts.join('\n'));
}

// `ooxml-core/<area>` -> the upstream build, so upstream's own dependencies resolve from upstream.
const upstreamAlias = {
	name: 'ooxml-core',
	setup(b) {
		b.onResolve({ filter: /^ooxml-core(\/.*)?$/ }, (args) => {
			const sub = args.path.slice('ooxml-core'.length).replace(/^\//, '');
			const file = join(upstreamDist, sub, 'index.mjs');
			if (!existsSync(file)) return { errors: [{ text: `upstream build missing: ${file} (run the upstream build)` }] };
			return { path: file };
		});
	},
};

const ADAPTERS = [
	'upstream-xlsx',
	'upstream-docx',
	'upstream-pptx',
	'sheetjs',
	'exceljs',
	'docx-preview',
	'mammoth',
	'pptxjs',
	'rust-calamine',
	'rust-umya',
	'rust-docx-rs',
	'rust-nitro-xlsx',
	'rust-nitro-docx',
	'rust-nitro-pptx',
	'betteroffice-docx',
];

const status = {};
try {
	await fetchPptxjs();
	status['pptxjs-vendor'] = { built: true };
} catch (e) {
	status['pptxjs-vendor'] = { built: false, error: String(e.message ?? e) };
}

for (const name of ADAPTERS) {
	try {
		const result = await build({
			entryPoints: [join(here, 'adapters', 'js', `${name}.mjs`)],
			bundle: true,
			format: 'esm',
			platform: 'browser',
			target: 'es2022',
			minify: true,
			sourcemap: false,
			outfile: join(out, `${name}.js`),
			define: { 'process.env.NODE_ENV': '"production"', global: 'globalThis' },
			plugins: [upstreamAlias],
			logLevel: 'silent',
			metafile: true,
		});
		const bytes = Object.values(result.metafile.outputs).reduce((n, o) => n + o.bytes, 0);
		status[name] = { built: true, bundleBytes: bytes };
		console.log(`bundled ${name} (${bytes} bytes)`);
	} catch (e) {
		const msg = (e.errors?.[0]?.text ?? String(e.message ?? e)).slice(0, 400);
		status[name] = { built: false, error: msg };
		console.log(`FAILED ${name}: ${msg}`);
	}
}

// The harness itself (normalisers run in the page so only canonical output crosses CDP).
await build({
	entryPoints: [join(here, 'page', 'harness.mjs')],
	bundle: true,
	format: 'esm',
	platform: 'browser',
	target: 'es2022',
	outfile: join(out, 'harness.js'),
	logLevel: 'warning',
});

// BetterOffice parseDocx loads opc+parse wasm via new URL(..., import.meta.url)
// relative to the bundled adapter. Copy those assets next to dist/bundles/.
if (status['betteroffice-docx']?.built) {
	try {
		const distDir = join(here, 'node_modules', '@betteroffice', 'docx', 'dist');
		const copies = [
			['generated/opc/ooxml_opc_bg.wasm', 'generated/opc/ooxml_opc_bg.wasm'],
			['generated/parse/docx_parse_bg.wasm', 'generated/parse/docx_parse_bg.wasm'],
		];
		for (const [rel, destRel] of copies) {
			const src = join(distDir, rel);
			if (!existsSync(src)) throw new Error(`missing ${src}`);
			const dest = join(out, destRel);
			mkdirSync(dirname(dest), { recursive: true });
			copyFileSync(src, dest);
		}
		status['betteroffice-docx'].wasmAssets = copies.map(([, d]) => d);
		console.log('copied betteroffice opc+parse wasm next to bundles/');
	} catch (e) {
		status['betteroffice-docx'].wasmAssetsError = String(e.message ?? e);
		console.log(`FAILED betteroffice wasm copy: ${status['betteroffice-docx'].wasmAssetsError}`);
	}
}

writeFileSync(join(out, 'status.json'), JSON.stringify(status, null, 2));
