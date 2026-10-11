// SPDX-License-Identifier: MIT
// Per-format scoreboard summary from the per-file results. Entrants are only ranked on files
// where they are comparable (same canonical output as the reference); every number carries the
// file count it is based on.

export const median = (xs) => {
	if (!xs.length) return null;
	const s = [...xs].sort((a, b) => a - b);
	const m = s.length >> 1;
	return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

const geomean = (xs) => (xs.length ? Math.exp(xs.reduce((a, x) => a + Math.log(x), 0) / xs.length) : null);

export function summarise(files, entrants, referenceId) {
	const out = {};
	for (const format of ['xlsx', 'docx', 'pptx']) {
		const fmtFiles = files.filter((f) => f.format === format);
		const rows = {};
		for (const e of entrants) {
			if (!e.formats.includes(format)) continue;
			if (e.status !== 'entered') {
				rows[e.id] = { status: e.status, reason: e.reason };
				continue;
			}
			const row = {
				status: 'entered',
				files: fmtFiles.length,
				ran: 0,
				errors: 0,
				skipped: 0,
				comparable: 0,
				notComparable: 0,
				medianOfMediansMs: null,
				sharedWithReference: 0,
				geomeanRatioVsReference: null,
				winsVsReference: 0,
				lossesVsReference: 0,
				tiesVsReference: 0,
				notComparableFiles: [],
				errorFiles: [],
			};
			const medians = [];
			const ratios = [];
			for (const f of fmtFiles) {
				const r = f.results[e.id];
				if (!r) continue;
				if (r.status === 'skipped') {
					row.skipped++;
					continue;
				}
				if (r.status === 'error') {
					row.errors++;
					row.errorFiles.push({ file: f.file, error: r.error });
					continue;
				}
				row.ran++;
				if (!r.comparable) {
					row.notComparable++;
					row.notComparableFiles.push({ file: f.file, firstDifference: r.firstDifference });
					continue;
				}
				row.comparable++;
				medians.push(r.medianMs);
				const ref = f.results[referenceId];
				if (e.id !== referenceId && ref && ref.status === 'ok' && ref.comparable) {
					row.sharedWithReference++;
					ratios.push(r.medianMs / ref.medianMs);
					if (r.medianMs < ref.medianMs) row.winsVsReference++;
					else if (r.medianMs > ref.medianMs) row.lossesVsReference++;
					else row.tiesVsReference++;
				}
			}
			row.medianOfMediansMs = median(medians);
			row.geomeanRatioVsReference = e.id === referenceId ? 1 : geomean(ratios);
			rows[e.id] = row;
		}
		out[format] = rows;
	}
	return out;
}
