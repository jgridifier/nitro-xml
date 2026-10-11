// SPDX-License-Identifier: MIT
/** Shared types for the Stage 0 parity harness (#5). */

export type ParityFormat = 'xml' | 'opc' | 'xlsx' | 'docx' | 'pptx';

export interface FixtureRow {
	id: string;
	format: string;
	path: string;
	sha256: string;
	bytes: number;
	tier: string;
	source: string;
	licence: string;
	features: string[];
	redistributable: boolean;
	on_demand?: boolean;
}

export interface Manifest {
	version: number;
	fixtures: FixtureRow[];
}

/** One registered comparison (foundation layer or format view IR). */
export interface Comparison {
	/** Stable id used in known-diffs and logs (usually the format name). */
	id: ParityFormat;
	/** Human label for CI summaries. */
	label: string;
	/**
	 * When false, `--ci` skips this comparison. Format parsers flip this to true
	 * when their Rust dump is available (#15 workbook / #20 docx styles / #24 pptx).
	 */
	ready: boolean;
	/** Issue that lands the Rust side, when not ready. */
	todo?: string;
	/** Produce the TS oracle IR for one fixture path (repo-relative or absolute). */
	oracle(fixturePath: string): Promise<unknown>;
	/** Produce the Rust IR for one fixture path. */
	rust(fixturePath: string): Promise<unknown>;
	/**
	 * Select fixture rows from the manifest for corpus / CI runs.
	 * Return [] when the comparison uses a non-manifest corpus (xml crafted cases).
	 */
	selectFixtures(manifest: Manifest): FixtureRow[];
	/**
	 * Optional corpus runner used by `--ci` instead of per-fixture oracle/rust
	 * (xml and opc batch differentials).
	 */
	runCorpus?(): Promise<CorpusResult>;
}

export interface CorpusResult {
	compared: number;
	mismatched: number;
	notes: string[];
}

export interface KnownDiffEntry {
	/** Comparison id (`xlsx`, `opc`, …). */
	format: ParityFormat | string;
	/** Fixture id from the manifest, or a synthetic id for non-manifest rows. */
	fixture: string;
	/**
	 * JSON-pointer-like path of an allowed difference (`/` = whole document).
	 * Omit or use `/` to allow any diff for that fixture (still needs a reason).
	 * A path starting with the two-star slash prefix matches by suffix
	 * (e.g. star-star-slash-text → "/slides/0/elements/1/text").
	 */
	path?: string;
	/** Required reason; CI rejects entries without one. */
	reason: string;
}

export interface KnownDiffsFile {
	entries: KnownDiffEntry[];
}

export interface DiffEntry {
	path: string;
	kind: 'missing' | 'extra' | 'changed';
	oracle?: unknown;
	rust?: unknown;
}
