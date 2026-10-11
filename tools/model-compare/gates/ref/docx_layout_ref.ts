// SPDX-License-Identifier: MIT
/** TS oracle: LayoutResult with fake measurer (parity #22). */

import { readFileSync } from 'node:fs';
import { dumpDocxViewIr } from '../../../../parity/oracle/docx.ts';

const path = process.argv[2];
if (!path) {
	console.error('usage: docx_layout_ref.ts <file.docx>');
	process.exit(2);
}

const json = await dumpDocxViewIr(path);
process.stdout.write(JSON.stringify(json));
