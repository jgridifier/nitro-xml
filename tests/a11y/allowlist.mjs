// SPDX-License-Identifier: MIT
/**
 * Intentional TS↔Rust a11y diffs (#42). Each entry needs an issue ref.
 *
 * axe: skip a rule id on a side (or both) when the surface is out of scope.
 * ax: skip a normalized role (optionally with name) when comparing content trees.
 * keyboard: rust-only shell inventory is authoritative for Stage-6 chrome (#29).
 */

/**
 * @typedef {object} AxeAllow
 * @property {string} ruleId
 * @property {'ts'|'rust'|'both'} [side]
 * @property {string} issue
 * @property {string} reason
 */

/**
 * @typedef {object} AxAllow
 * @property {string} role
 * @property {string} [name]
 * @property {'ts'|'rust'|'both'} [side]
 * @property {string} issue
 * @property {string} reason
 */

/** @type {AxeAllow[]} */
export const AXE_ALLOW = [
	{
		// Upstream xlsx-editor still mounts the edit focus sink / ribbon chrome even
		// in read-only visual hosts; Stage-6 nitro shell is compact (#29), not a
		// one-to-one ribbon port. Content-surface rules are not allow-listed.
		ruleId: 'landmark-one-main',
		side: 'both',
		issue: '#29',
		reason: 'Viewer shell is not a document landmark layout; editor ribbon differs.',
	},
	{
		ruleId: 'region',
		side: 'ts',
		issue: '#29',
		reason: 'Upstream editor chrome exposes extra regions not in nitro Stage-6 shell.',
	},
];

/** @type {AxAllow[]} */
export const AX_ALLOW = [
	{
		role: 'text',
		side: 'ts',
		issue: '#29',
		reason: 'Upstream editor ribbon/title/status text nodes absent from nitro shell.',
	},
	{
		role: 'toolbar',
		side: 'ts',
		issue: '#29',
		reason: 'Upstream Office ribbon toolbar; nitro uses compact Stage-6 chrome.',
	},
	{
		role: 'menubar',
		side: 'ts',
		issue: '#29',
		reason: 'Upstream ribbon menubar; out of Stage-6 shell scope.',
	},
	{
		role: 'combobox',
		side: 'ts',
		issue: '#29',
		reason: 'Formula bar / name box; editor surface (E-text), not M1 viewer.',
	},
	{
		role: 'textbox',
		side: 'ts',
		issue: '#55',
		reason: 'Grid focus-sink textarea is the editor surface; viewer has no cell editor yet.',
	},
	{
		role: 'region',
		name: 'Spreadsheet editor',
		side: 'ts',
		issue: '#29',
		reason: 'Upstream xlsx-editor host region; nitro Stage-6 shell has no matching landmark.',
	},
	{
		role: 'button',
		name: 'Zoom out',
		side: 'rust',
		issue: '#29',
		reason: 'Nitro Stage-6 shell control; not present on upstream pptx reading stage.',
	},
	{
		role: 'button',
		name: 'Zoom in',
		side: 'rust',
		issue: '#29',
		reason: 'Nitro Stage-6 shell control; not present on upstream pptx reading stage.',
	},
	{
		role: 'button',
		name: 'Find',
		side: 'rust',
		issue: '#29',
		reason: 'Nitro Stage-6 shell control.',
	},
	{
		role: 'button',
		name: 'Copy',
		side: 'rust',
		issue: '#29',
		reason: 'Nitro Stage-6 shell control.',
	},
	{
		role: 'button',
		name: 'Diagnostics',
		side: 'rust',
		issue: '#29',
		reason: 'Nitro Stage-6 shell control.',
	},
];

/**
 * @param {AxeAllow[]} list
 * @param {string} ruleId
 * @param {'ts'|'rust'} side
 */
export function axeAllowed(list, ruleId, side) {
	return list.some(
		(a) => a.ruleId === ruleId && (!a.side || a.side === 'both' || a.side === side),
	);
}

/**
 * @param {AxAllow[]} list
 * @param {{role: string, name: string}} node
 * @param {'ts'|'rust'} side
 */
export function axAllowed(list, node, side) {
	return list.some((a) => {
		if (a.side && a.side !== 'both' && a.side !== side) return false;
		if (a.role !== node.role) return false;
		if (a.name != null && a.name !== node.name) return false;
		return true;
	});
}

/** Shell controls that must be keyboard-reachable on rust viewers (#29 / #42). */
export const RUST_SHELL_INVENTORY = Object.freeze([
	{ action: 'zoom-out', name: 'Zoom out' },
	{ action: 'zoom-in', name: 'Zoom in' },
	{ action: 'find-open', name: 'Find' },
	{ action: 'copy', name: 'Copy' },
	{ action: 'diagnostics-toggle', name: 'Diagnostics' },
]);

/** Home ribbon controls inventoried in xlsx edit-mode a11y (#53). */
export const RUST_RIBBON_INVENTORY = Object.freeze([
	{ command: 'home.bold', name: 'Bold' },
	{ command: 'home.italic', name: 'Italic' },
	{ command: 'home.copy', name: 'Copy' },
	{ command: 'home.paste', name: 'Paste' },
]);

/**
 * Pptx reading-stage oracle has no sheet/slide tablist; nitro Stage-6 does (#29).
 * xlsx compares tab presence (cardinality ignored — see ax-tree.mjs).
 */
export const PPTX_SHELL_ONLY_ROLES = new Set(['tablist', 'tab']);
