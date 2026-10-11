// SPDX-License-Identifier: MIT
// Reference side of the #11 gate: dumps upstream readZipParts() output in the zip_dump JSON shape.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
const up = process.env.UPSTREAM_DIR!;
const { readZipParts } = await import(`${up}/src/core/xlsx/read/package.ts`);
const file = process.argv[2]!;
try {
	const parts: Map<string, Uint8Array> = await readZipParts(new Uint8Array(readFileSync(file)));
	const out = [...parts.entries()]
		.map(([name, data]) => ({ name, size: data.byteLength, sha256: createHash('sha256').update(data).digest('hex') }))
		.sort((a, b) => Buffer.compare(Buffer.from(a.name), Buffer.from(b.name)));
	console.log(JSON.stringify({ parts: out }));
} catch (e) {
	console.log(JSON.stringify({ error: String((e as Error).message).slice(0, 80) }));
	process.exit(2);
}
