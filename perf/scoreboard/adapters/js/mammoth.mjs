// SPDX-License-Identifier: MIT
// mammoth.js 1.13.0: extractRawText, its cheapest documented API that yields all text
// (https://github.com/mwilliamson/mammoth.js#mammothextractrawtextinput). It parses and produces
// the text in one call, so the timed parse includes text generation; paragraphs end with "\n\n".
import mammoth from 'mammoth/mammoth.browser.js';

export const parse = (bytes) =>
	mammoth.extractRawText({ arrayBuffer: bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) });

export const extract = (result) => ({ paragraphs: result.value.split('\n\n') });
