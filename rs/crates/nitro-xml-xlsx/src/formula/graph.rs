// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/{graph,scc,graph-index,area-index}.ts
// and the function-name/volatile flags of functions/registry.ts; changes listed in PROVENANCE.md.
//! Dependency graph: static precedents, Tarjan order, and the area indexes the scheduler uses.
//!
//! Function *bodies* are #73–#75. This module only knows which names exist and which are volatile,
//! so `analyze` matches upstream `getFunction` for `volatile` / `unsupported`.

use std::collections::{HashMap, HashSet};

use crate::formula::ast::{
    BinaryOp, FormulaAst, FormulaError, RefSpec, SheetPrefix, StructuredRef,
};
use crate::model::{CellAddress, CellRange};

/// Names and volatile flags from upstream `FUNCTION_CATALOG` / `getFunction` (481 entries).
/// Generated from ChristopherVR/ooxml@e66aaec; do not hand-edit.
const FUNCTIONS: &[(&str, bool)] = &[
    ("ABS", false),
    ("ACCRINT", false),
    ("ACCRINTM", false),
    ("ACOS", false),
    ("ACOSH", false),
    ("ACOT", false),
    ("ACOTH", false),
    ("ADDRESS", false),
    ("AGGREGATE", false),
    ("AMORLINC", false),
    ("ANCHORARRAY", false),
    ("AND", false),
    ("ARABIC", false),
    ("AREAS", false),
    ("ARRAYTOTEXT", false),
    ("ASIN", false),
    ("ASINH", false),
    ("ATAN", false),
    ("ATAN2", false),
    ("ATANH", false),
    ("AVEDEV", false),
    ("AVERAGE", false),
    ("AVERAGEA", false),
    ("AVERAGEIF", false),
    ("AVERAGEIFS", false),
    ("BASE", false),
    ("BETA.DIST", false),
    ("BETA.INV", false),
    ("BETADIST", false),
    ("BETAINV", false),
    ("BIN2DEC", false),
    ("BIN2HEX", false),
    ("BIN2OCT", false),
    ("BINOM.DIST", false),
    ("BINOM.DIST.RANGE", false),
    ("BINOM.INV", false),
    ("BINOMDIST", false),
    ("BITAND", false),
    ("BITLSHIFT", false),
    ("BITOR", false),
    ("BITRSHIFT", false),
    ("BITXOR", false),
    ("BYCOL", false),
    ("BYROW", false),
    ("CEILING", false),
    ("CEILING.MATH", false),
    ("CEILING.PRECISE", false),
    ("CELL", true),
    ("CHAR", false),
    ("CHIDIST", false),
    ("CHIINV", false),
    ("CHISQ.DIST", false),
    ("CHISQ.DIST.RT", false),
    ("CHISQ.INV", false),
    ("CHISQ.INV.RT", false),
    ("CHISQ.TEST", false),
    ("CHITEST", false),
    ("CHOOSE", false),
    ("CHOOSECOLS", false),
    ("CHOOSEROWS", false),
    ("CLEAN", false),
    ("CODE", false),
    ("COLUMN", false),
    ("COLUMNS", false),
    ("COMBIN", false),
    ("COMBINA", false),
    ("COMPLEX", false),
    ("CONCAT", false),
    ("CONCATENATE", false),
    ("CONFIDENCE", false),
    ("CONFIDENCE.NORM", false),
    ("CONFIDENCE.T", false),
    ("CONVERT", false),
    ("CORREL", false),
    ("COS", false),
    ("COSH", false),
    ("COT", false),
    ("COTH", false),
    ("COUNT", false),
    ("COUNTA", false),
    ("COUNTBLANK", false),
    ("COUNTIF", false),
    ("COUNTIFS", false),
    ("COUPDAYBS", false),
    ("COUPDAYS", false),
    ("COUPDAYSNC", false),
    ("COUPNCD", false),
    ("COUPNUM", false),
    ("COUPPCD", false),
    ("COVAR", false),
    ("COVARIANCE.P", false),
    ("COVARIANCE.S", false),
    ("CRITBINOM", false),
    ("CSC", false),
    ("CSCH", false),
    ("CUMIPMT", false),
    ("CUMPRINC", false),
    ("DATE", false),
    ("DATEDIF", false),
    ("DATEVALUE", false),
    ("DAVERAGE", false),
    ("DAY", false),
    ("DAYS", false),
    ("DAYS360", false),
    ("DB", false),
    ("DCOUNT", false),
    ("DCOUNTA", false),
    ("DDB", false),
    ("DEC2BIN", false),
    ("DEC2HEX", false),
    ("DEC2OCT", false),
    ("DECIMAL", false),
    ("DEGREES", false),
    ("DELTA", false),
    ("DEVSQ", false),
    ("DGET", false),
    ("DISC", false),
    ("DMAX", false),
    ("DMIN", false),
    ("DOLLAR", false),
    ("DOLLARDE", false),
    ("DOLLARFR", false),
    ("DPRODUCT", false),
    ("DROP", false),
    ("DSTDEV", false),
    ("DSTDEVP", false),
    ("DSUM", false),
    ("DURATION", false),
    ("DVAR", false),
    ("DVARP", false),
    ("EDATE", false),
    ("EFFECT", false),
    ("ENCODEURL", false),
    ("EOMONTH", false),
    ("ERF", false),
    ("ERF.PRECISE", false),
    ("ERFC", false),
    ("ERFC.PRECISE", false),
    ("ERROR.TYPE", false),
    ("EVEN", false),
    ("EXACT", false),
    ("EXP", false),
    ("EXPAND", false),
    ("EXPON.DIST", false),
    ("EXPONDIST", false),
    ("F.DIST", false),
    ("F.DIST.RT", false),
    ("F.INV", false),
    ("F.INV.RT", false),
    ("F.TEST", false),
    ("FACT", false),
    ("FACTDOUBLE", false),
    ("FALSE", false),
    ("FDIST", false),
    ("FILTER", false),
    ("FIND", false),
    ("FINDB", false),
    ("FINV", false),
    ("FISHER", false),
    ("FISHERINV", false),
    ("FIXED", false),
    ("FLOOR", false),
    ("FLOOR.MATH", false),
    ("FLOOR.PRECISE", false),
    ("FORECAST", false),
    ("FORECAST.LINEAR", false),
    ("FORMULATEXT", false),
    ("FREQUENCY", false),
    ("FTEST", false),
    ("FV", false),
    ("FVSCHEDULE", false),
    ("GAMMA", false),
    ("GAMMA.DIST", false),
    ("GAMMA.INV", false),
    ("GAMMADIST", false),
    ("GAMMAINV", false),
    ("GAMMALN", false),
    ("GAMMALN.PRECISE", false),
    ("GAUSS", false),
    ("GCD", false),
    ("GEOMEAN", false),
    ("GESTEP", false),
    ("GROWTH", false),
    ("HARMEAN", false),
    ("HEX2BIN", false),
    ("HEX2DEC", false),
    ("HEX2OCT", false),
    ("HLOOKUP", false),
    ("HOUR", false),
    ("HSTACK", false),
    ("HYPERLINK", false),
    ("HYPGEOM.DIST", false),
    ("HYPGEOMDIST", false),
    ("IF", false),
    ("IFERROR", false),
    ("IFNA", false),
    ("IFS", false),
    ("IMABS", false),
    ("IMAGINARY", false),
    ("IMARGUMENT", false),
    ("IMCONJUGATE", false),
    ("IMCOS", false),
    ("IMCOSH", false),
    ("IMCOT", false),
    ("IMCSC", false),
    ("IMCSCH", false),
    ("IMDIV", false),
    ("IMEXP", false),
    ("IMLN", false),
    ("IMLOG10", false),
    ("IMLOG2", false),
    ("IMPOWER", false),
    ("IMPRODUCT", false),
    ("IMREAL", false),
    ("IMSEC", false),
    ("IMSECH", false),
    ("IMSIN", false),
    ("IMSINH", false),
    ("IMSQRT", false),
    ("IMSUB", false),
    ("IMSUM", false),
    ("IMTAN", false),
    ("INDEX", false),
    ("INDIRECT", true),
    ("INT", false),
    ("INTERCEPT", false),
    ("INTRATE", false),
    ("IPMT", false),
    ("IRR", false),
    ("ISBLANK", false),
    ("ISERR", false),
    ("ISERROR", false),
    ("ISEVEN", false),
    ("ISFORMULA", false),
    ("ISLOGICAL", false),
    ("ISNA", false),
    ("ISNONTEXT", false),
    ("ISNUMBER", false),
    ("ISO.CEILING", false),
    ("ISODD", false),
    ("ISOWEEKNUM", false),
    ("ISPMT", false),
    ("ISREF", false),
    ("ISTEXT", false),
    ("KURT", false),
    ("LAMBDA", false),
    ("LARGE", false),
    ("LCM", false),
    ("LEFT", false),
    ("LEFTB", false),
    ("LEN", false),
    ("LENB", false),
    ("LET", false),
    ("LINEST", false),
    ("LN", false),
    ("LOG", false),
    ("LOG10", false),
    ("LOGEST", false),
    ("LOGINV", false),
    ("LOGNORM.DIST", false),
    ("LOGNORM.INV", false),
    ("LOGNORMDIST", false),
    ("LOOKUP", false),
    ("LOWER", false),
    ("MAKEARRAY", false),
    ("MAP", false),
    ("MATCH", false),
    ("MAX", false),
    ("MAXA", false),
    ("MAXIFS", false),
    ("MDETERM", false),
    ("MDURATION", false),
    ("MEDIAN", false),
    ("MID", false),
    ("MIDB", false),
    ("MIN", false),
    ("MINA", false),
    ("MINIFS", false),
    ("MINUTE", false),
    ("MINVERSE", false),
    ("MIRR", false),
    ("MMULT", false),
    ("MOD", false),
    ("MODE", false),
    ("MODE.MULT", false),
    ("MODE.SNGL", false),
    ("MONTH", false),
    ("MROUND", false),
    ("MULTINOMIAL", false),
    ("MUNIT", false),
    ("N", false),
    ("NA", false),
    ("NEGBINOM.DIST", false),
    ("NEGBINOMDIST", false),
    ("NETWORKDAYS", false),
    ("NETWORKDAYS.INTL", false),
    ("NOMINAL", false),
    ("NORM.DIST", false),
    ("NORM.INV", false),
    ("NORM.S.DIST", false),
    ("NORM.S.INV", false),
    ("NORMDIST", false),
    ("NORMINV", false),
    ("NORMSDIST", false),
    ("NORMSINV", false),
    ("NOT", false),
    ("NOW", true),
    ("NPER", false),
    ("NPV", false),
    ("NUMBERVALUE", false),
    ("OCT2BIN", false),
    ("OCT2DEC", false),
    ("OCT2HEX", false),
    ("ODD", false),
    ("OFFSET", true),
    ("OR", false),
    ("PDURATION", false),
    ("PEARSON", false),
    ("PERCENTILE", false),
    ("PERCENTILE.EXC", false),
    ("PERCENTILE.INC", false),
    ("PERCENTRANK", false),
    ("PERCENTRANK.EXC", false),
    ("PERCENTRANK.INC", false),
    ("PERMUT", false),
    ("PERMUTATIONA", false),
    ("PHI", false),
    ("PI", false),
    ("PMT", false),
    ("POISSON", false),
    ("POISSON.DIST", false),
    ("POWER", false),
    ("PPMT", false),
    ("PRICE", false),
    ("PRICEDISC", false),
    ("PRICEMAT", false),
    ("PROB", false),
    ("PRODUCT", false),
    ("PROPER", false),
    ("PV", false),
    ("QUARTILE", false),
    ("QUARTILE.EXC", false),
    ("QUARTILE.INC", false),
    ("QUOTIENT", false),
    ("RADIANS", false),
    ("RAND", true),
    ("RANDARRAY", true),
    ("RANDBETWEEN", true),
    ("RANK", false),
    ("RANK.AVG", false),
    ("RANK.EQ", false),
    ("RATE", false),
    ("RECEIVED", false),
    ("REDUCE", false),
    ("REGEXEXTRACT", false),
    ("REGEXREPLACE", false),
    ("REGEXTEST", false),
    ("REPLACE", false),
    ("REPLACEB", false),
    ("REPT", false),
    ("RIGHT", false),
    ("RIGHTB", false),
    ("ROMAN", false),
    ("ROUND", false),
    ("ROUNDDOWN", false),
    ("ROUNDUP", false),
    ("ROW", false),
    ("ROWS", false),
    ("RRI", false),
    ("RSQ", false),
    ("SCAN", false),
    ("SEARCH", false),
    ("SEARCHB", false),
    ("SEC", false),
    ("SECH", false),
    ("SECOND", false),
    ("SEQUENCE", false),
    ("SERIESSUM", false),
    ("SHEET", false),
    ("SHEETS", false),
    ("SIGN", false),
    ("SIN", false),
    ("SINGLE", false),
    ("SINH", false),
    ("SKEW", false),
    ("SKEW.P", false),
    ("SLN", false),
    ("SLOPE", false),
    ("SMALL", false),
    ("SORT", false),
    ("SORTBY", false),
    ("SQRT", false),
    ("SQRTPI", false),
    ("STANDARDIZE", false),
    ("STDEV", false),
    ("STDEV.P", false),
    ("STDEV.S", false),
    ("STDEVA", false),
    ("STDEVP", false),
    ("STDEVPA", false),
    ("STEYX", false),
    ("SUBSTITUTE", false),
    ("SUBTOTAL", false),
    ("SUM", false),
    ("SUMIF", false),
    ("SUMIFS", false),
    ("SUMPRODUCT", false),
    ("SUMSQ", false),
    ("SUMX2MY2", false),
    ("SUMX2PY2", false),
    ("SUMXMY2", false),
    ("SWITCH", false),
    ("SYD", false),
    ("T", false),
    ("T.DIST", false),
    ("T.DIST.2T", false),
    ("T.DIST.RT", false),
    ("T.INV", false),
    ("T.INV.2T", false),
    ("T.TEST", false),
    ("TAKE", false),
    ("TAN", false),
    ("TANH", false),
    ("TBILLEQ", false),
    ("TBILLPRICE", false),
    ("TBILLYIELD", false),
    ("TDIST", false),
    ("TEXT", false),
    ("TEXTAFTER", false),
    ("TEXTBEFORE", false),
    ("TEXTJOIN", false),
    ("TEXTSPLIT", false),
    ("TIME", false),
    ("TIMEVALUE", false),
    ("TINV", false),
    ("TOCOL", false),
    ("TODAY", true),
    ("TOROW", false),
    ("TRANSPOSE", false),
    ("TREND", false),
    ("TRIM", false),
    ("TRIMMEAN", false),
    ("TRUE", false),
    ("TRUNC", false),
    ("TTEST", false),
    ("TYPE", false),
    ("UNICHAR", false),
    ("UNICODE", false),
    ("UNIQUE", false),
    ("UPPER", false),
    ("VALUE", false),
    ("VALUETOTEXT", false),
    ("VAR", false),
    ("VAR.P", false),
    ("VAR.S", false),
    ("VARA", false),
    ("VARP", false),
    ("VARPA", false),
    ("VDB", false),
    ("VLOOKUP", false),
    ("VSTACK", false),
    ("WEEKDAY", false),
    ("WEEKNUM", false),
    ("WEIBULL", false),
    ("WEIBULL.DIST", false),
    ("WORKDAY", false),
    ("WORKDAY.INTL", false),
    ("WRAPCOLS", false),
    ("WRAPROWS", false),
    ("XIRR", false),
    ("XLOOKUP", false),
    ("XMATCH", false),
    ("XNPV", false),
    ("XOR", false),
    ("YEAR", false),
    ("YEARFRAC", false),
    ("YIELD", false),
    ("YIELDDISC", false),
    ("YIELDMAT", false),
    ("Z.TEST", false),
    ("ZTEST", false),
];

/// `Some(volatile)` when `name` is an implemented upstream function (already upper-case).
pub(crate) fn function_volatile(name: &str) -> Option<bool> {
    FUNCTIONS
        .binary_search_by(|entry| entry.0.cmp(name))
        .ok()
        .map(|i| FUNCTIONS[i].1)
}

fn reference_argument(name: &str) -> bool {
    matches!(
        name,
        "OFFSET" | "INDEX" | "ROW" | "COLUMN" | "ROWS" | "COLUMNS" | "AREAS" | "ISREF"
    )
}

/// One rectangular precedent on a sheet. Rows and columns are zero-based and normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Area {
    pub sheet: u32,
    pub range: CellRange,
}

/// What [`analyze`] reports. `has_call` is true when a `call` / `invoke` is reachable, including
/// through defined names (same depth limit as precedent walking).
#[derive(Debug, Clone)]
pub(crate) struct DepInfo {
    pub deps: Vec<Area>,
    pub dynamic_deps: Vec<Area>,
    pub volatile: bool,
    pub unsupported: bool,
    pub has_call: bool,
}

/// A formula cell in the calc graph.
#[derive(Debug, Clone)]
pub(crate) struct FormulaNode {
    pub sheet: u32,
    pub row: u32,
    pub col: u32,
    pub formula: String,
    pub ast: Option<FormulaAst>,
    pub deps: Vec<Area>,
    pub dynamic_deps: Vec<Area>,
    pub volatile: bool,
    pub unsupported: bool,
    pub keep_cached: bool,
    pub has_call: bool,
    pub array_range: Option<CellRange>,
    pub legacy: bool,
    pub spill: Option<CellRange>,
    pub blocked_spill: Option<CellRange>,
    pub parse_error: Option<String>,
    pub rank: u32,
    pub cyclic: bool,
    pub ranked: bool,
    /// How many times `compute` entered this node (tests and incremental checks).
    pub computes: u32,
    /// False after the formula cell was removed; the slot stays so node ids remain indexes.
    pub alive: bool,
}

impl FormulaNode {
    pub(crate) fn blank(sheet: u32, row: u32, col: u32, formula: String) -> Self {
        Self {
            sheet,
            row,
            col,
            formula,
            ast: None,
            deps: Vec::new(),
            dynamic_deps: Vec::new(),
            volatile: false,
            unsupported: false,
            keep_cached: false,
            has_call: false,
            array_range: None,
            legacy: false,
            spill: None,
            blocked_spill: None,
            parse_error: None,
            rank: 0,
            cyclic: false,
            ranked: false,
            computes: 0,
            alive: true,
        }
    }
}

/// Host surface `analyze` needs: sheet names, defined-name formulas, structured areas, parsing.
pub(crate) trait DepHost {
    fn sheet_names(&self) -> &[String];
    fn parse_dep(&mut self, formula: &str) -> Result<FormulaAst, FormulaError>;
    fn defined_formula(&self, name: &str, sheet: u32, explicit: bool) -> Option<String>;
    fn structured_areas(
        &self,
        reference: &StructuredRef,
        sheet: u32,
        row: u32,
        col: u32,
    ) -> Vec<Area>;
}

struct DepCtx {
    sheet: u32,
    row: u32,
    col: u32,
    deps: Vec<Area>,
    dynamic_deps: Vec<Area>,
    dynamic: u32,
    volatile: bool,
    unsupported: bool,
    has_call: bool,
    depth: u32,
}

fn push(ctx: &mut DepCtx, area: Area) {
    if ctx.dynamic > 0 {
        ctx.dynamic_deps.push(area);
    } else {
        ctx.deps.push(area);
    }
}

fn normalize_range(range: CellRange) -> CellRange {
    CellRange {
        start: CellAddress {
            row: range.start.row.min(range.end.row),
            col: range.start.col.min(range.end.col),
        },
        end: CellAddress {
            row: range.start.row.max(range.end.row),
            col: range.start.col.max(range.end.col),
        },
    }
}

fn coord(v: i32) -> u32 {
    if v < 0 {
        0
    } else {
        v as u32
    }
}

pub(crate) fn spec_range(spec: &RefSpec) -> CellRange {
    normalize_range(CellRange {
        start: CellAddress {
            row: coord(spec.start.row),
            col: coord(spec.start.col),
        },
        end: CellAddress {
            row: coord(spec.end.row),
            col: coord(spec.end.col),
        },
    })
}

pub(crate) fn sheet_index(names: &[String], name: &str) -> Option<u32> {
    if let Some(i) = names.iter().position(|n| n == name) {
        return Some(i as u32);
    }
    let lower = name.to_lowercase();
    names
        .iter()
        .position(|n| n.to_lowercase() == lower)
        .map(|i| i as u32)
}

pub(crate) fn prefix_sheets(
    names: &[String],
    prefix: &Option<SheetPrefix>,
    current: u32,
) -> Option<Vec<u32>> {
    let Some(prefix) = prefix else {
        return Some(vec![current]);
    };
    if prefix.book.is_some() {
        return None;
    }
    let first = sheet_index(names, &prefix.sheet)?;
    let Some(sheet2) = &prefix.sheet2 else {
        return Some(vec![first]);
    };
    let last = sheet_index(names, sheet2)?;
    let (lo, hi) = if first <= last {
        (first, last)
    } else {
        (last, first)
    };
    Some((lo..=hi).collect())
}

/// Bounding box per sheet, in first-seen sheet order.
fn spans(areas: &[Area]) -> Vec<Area> {
    let mut order = Vec::new();
    let mut boxes: HashMap<u32, CellRange> = HashMap::new();
    for area in areas {
        if let Some(box_range) = boxes.get_mut(&area.sheet) {
            box_range.start.row = box_range.start.row.min(area.range.start.row);
            box_range.start.col = box_range.start.col.min(area.range.start.col);
            box_range.end.row = box_range.end.row.max(area.range.end.row);
            box_range.end.col = box_range.end.col.max(area.range.end.col);
        } else {
            order.push(area.sheet);
            boxes.insert(area.sheet, area.range.clone());
        }
    }
    order
        .into_iter()
        .map(|sheet| Area {
            sheet,
            range: boxes.remove(&sheet).expect("sheet inserted"),
        })
        .collect()
}

fn is_xlpm(name: &str) -> bool {
    name.len() >= 6 && name[..6].eq_ignore_ascii_case("_xlpm.")
}

fn visit(node: &FormulaAst, ctx: &mut DepCtx, host: &mut dyn DepHost) {
    match node {
        FormulaAst::Ref {
            prefix,
            reference,
            spill,
        } => {
            let Some(reference) = reference else {
                return;
            };
            let Some(sheets) = prefix_sheets(host.sheet_names(), prefix, ctx.sheet) else {
                return;
            };
            let range = spec_range(reference);
            let target = if *spill {
                CellRange {
                    start: range.start,
                    end: range.start,
                }
            } else {
                range
            };
            for sheet in sheets {
                push(
                    ctx,
                    Area {
                        sheet,
                        range: target.clone(),
                    },
                );
            }
        }
        FormulaAst::Name { prefix, name } => {
            let mut sheet = ctx.sheet;
            if prefix.is_some() {
                let Some(sheets) = prefix_sheets(host.sheet_names(), prefix, ctx.sheet) else {
                    return;
                };
                let Some(first) = sheets.first().copied() else {
                    return;
                };
                sheet = first;
            }
            if let Some(formula) = host.defined_formula(name, sheet, prefix.is_some()) {
                if ctx.depth > 16 {
                    return;
                }
                let Ok(ast) = host.parse_dep(&formula) else {
                    return;
                };
                ctx.depth += 1;
                visit(&ast, ctx, host);
                ctx.depth -= 1;
                return;
            }
            let reference = StructuredRef {
                table: Some(name.clone()),
                specials: Vec::new(),
                column: None,
                column2: None,
            };
            for area in host.structured_areas(&reference, ctx.sheet, ctx.row, ctx.col) {
                push(ctx, area);
            }
        }
        FormulaAst::Structured { reference, .. } => {
            for area in host.structured_areas(reference, ctx.sheet, ctx.row, ctx.col) {
                push(ctx, area);
            }
        }
        FormulaAst::Unary { operand, .. } | FormulaAst::Percent { operand } => {
            visit(operand, ctx, host);
        }
        FormulaAst::Binary { .. } => visit_binary(node, ctx, host),
        FormulaAst::Call {
            name,
            raw_name,
            args,
        } => {
            ctx.has_call = true;
            let spec = function_volatile(name);
            if spec == Some(true) {
                ctx.volatile = true;
            }
            if spec.is_none()
                && !is_xlpm(raw_name)
                && host.defined_formula(raw_name, ctx.sheet, false).is_none()
            {
                ctx.unsupported = true;
            }
            let reference = spec.is_some() && reference_argument(name);
            for (i, arg) in args.iter().enumerate() {
                if reference && i == 0 {
                    ctx.dynamic += 1;
                    visit(arg, ctx, host);
                    ctx.dynamic -= 1;
                } else {
                    visit(arg, ctx, host);
                }
            }
        }
        FormulaAst::Invoke { callee, args } => {
            ctx.has_call = true;
            visit(callee, ctx, host);
            for arg in args {
                visit(arg, ctx, host);
            }
        }
        FormulaAst::Number { .. }
        | FormulaAst::String { .. }
        | FormulaAst::Boolean { .. }
        | FormulaAst::Error { .. }
        | FormulaAst::Array { .. }
        | FormulaAst::Missing => {}
    }
}

fn visit_binary(node: &FormulaAst, ctx: &mut DepCtx, host: &mut dyn DepHost) {
    let mut spine: Vec<&FormulaAst> = Vec::new();
    let mut leftmost = node;
    while let FormulaAst::Binary { left, .. } = leftmost {
        spine.push(leftmost);
        leftmost = left;
    }
    let static_start = ctx.deps.len();
    let dynamic_start = ctx.dynamic_deps.len();
    visit(leftmost, ctx, host);
    for op_node in spine.iter().rev() {
        let FormulaAst::Binary { op, left, right } = op_node else {
            continue;
        };
        visit(right, ctx, host);
        if *op != BinaryOp::Range {
            continue;
        }
        let both_refs = matches!(left.as_ref(), FormulaAst::Ref { .. })
            && matches!(right.as_ref(), FormulaAst::Ref { .. });
        if both_refs {
            let list = if ctx.dynamic > 0 {
                &ctx.dynamic_deps
            } else {
                &ctx.deps
            };
            if list.len() >= 2 {
                let left_area = list[list.len() - 2].clone();
                let right_area = list[list.len() - 1].clone();
                if left_area.sheet == right_area.sheet {
                    if let Some(box_area) = spans(&[left_area, right_area]).into_iter().next() {
                        push(ctx, box_area);
                    }
                }
            }
            continue;
        }
        let mentioned: Vec<Area> = ctx.deps[static_start..]
            .iter()
            .cloned()
            .chain(ctx.dynamic_deps[dynamic_start..].iter().cloned())
            .collect();
        ctx.dynamic_deps.extend(spans(&mentioned));
    }
}

/// Static precedents of `ast`, plus volatile / unsupported / call flags.
pub(crate) fn analyze(
    ast: &FormulaAst,
    host: &mut dyn DepHost,
    sheet: u32,
    row: u32,
    col: u32,
) -> DepInfo {
    let mut ctx = DepCtx {
        sheet,
        row,
        col,
        deps: Vec::new(),
        dynamic_deps: Vec::new(),
        dynamic: 0,
        volatile: false,
        unsupported: false,
        has_call: false,
        depth: 0,
    };
    visit(ast, &mut ctx, host);
    DepInfo {
        deps: ctx.deps,
        dynamic_deps: ctx.dynamic_deps,
        volatile: ctx.volatile,
        unsupported: ctx.unsupported,
        has_call: ctx.has_call,
    }
}

// ---- Tarjan + formula order -------------------------------------------------

struct SccFrame {
    node: u32,
    succ: Vec<u32>,
    i: usize,
}

struct SccState {
    index: HashMap<u32, u32>,
    low: HashMap<u32, u32>,
    on_stack: HashSet<u32>,
    stack: Vec<u32>,
    counter: u32,
    work: Vec<SccFrame>,
}

/// Strongly connected components. Edges point at precedents, so components come out precedents-first.
pub(crate) fn strongly_connected(
    order: &[u32],
    successors: &mut dyn FnMut(u32) -> Vec<u32>,
) -> Vec<Vec<u32>> {
    let mut state = SccState {
        index: HashMap::new(),
        low: HashMap::new(),
        on_stack: HashSet::new(),
        stack: Vec::new(),
        counter: 0,
        work: Vec::new(),
    };
    let mut out: Vec<Vec<u32>> = Vec::new();

    for &root in order {
        if state.index.contains_key(&root) {
            continue;
        }
        open(&mut state, root, successors);
        while !state.work.is_empty() {
            let last = state.work.len() - 1;
            if state.work[last].i < state.work[last].succ.len() {
                let next = state.work[last].succ[state.work[last].i];
                state.work[last].i += 1;
                if !state.index.contains_key(&next) {
                    open(&mut state, next, successors);
                } else if state.on_stack.contains(&next) {
                    let node = state.work[last].node;
                    let lowered = state.low[&node].min(state.index[&next]);
                    state.low.insert(node, lowered);
                }
                continue;
            }
            let frame_node = state.work[last].node;
            state.work.pop();
            if let Some(parent) = state.work.last() {
                let lowered = state.low[&parent.node].min(state.low[&frame_node]);
                state.low.insert(parent.node, lowered);
            }
            if state.low[&frame_node] == state.index[&frame_node] {
                let mut component = Vec::new();
                loop {
                    let member = state.stack.pop().expect("scc stack");
                    state.on_stack.remove(&member);
                    component.push(member);
                    if member == frame_node {
                        break;
                    }
                }
                out.push(component);
            }
        }
    }
    out
}

fn open(state: &mut SccState, node: u32, successors: &mut dyn FnMut(u32) -> Vec<u32>) {
    state.index.insert(node, state.counter);
    state.low.insert(node, state.counter);
    state.counter += 1;
    state.stack.push(node);
    state.on_stack.insert(node);
    let succ = successors(node);
    state.work.push(SccFrame { node, succ, i: 0 });
}

pub(crate) struct OrderComponent {
    pub nodes: Vec<u32>,
    pub cyclic: bool,
}

pub(crate) fn formula_order(
    order: &[u32],
    successors: &mut dyn FnMut(u32) -> Vec<u32>,
) -> (Vec<OrderComponent>, HashMap<u32, u32>) {
    let sccs = strongly_connected(order, successors);
    let mut ranks = HashMap::new();
    let mut components = Vec::with_capacity(sccs.len());
    for (rank, members) in sccs.into_iter().enumerate() {
        for &member in &members {
            ranks.insert(member, rank as u32);
        }
        let first = members[0];
        let succ = successors(first);
        let cyclic = members.len() > 1 || succ.contains(&first);
        components.push(OrderComponent {
            nodes: members,
            cyclic,
        });
    }
    (components, ranks)
}

// ---- Spatial indexes --------------------------------------------------------

const BUCKET_WIDTH: u32 = 64;
const CHUNK_SHIFT: u32 = 6;
const CHUNKS: u32 = 64;

fn cell_key(row: u32, col: u32) -> u64 {
    (row as u64) * 16_384 + col as u64
}

fn key_to_rc(key: u64) -> (u32, u32) {
    ((key / 16_384) as u32, (key % 16_384) as u32)
}

pub(crate) fn ranges_intersect(a: &CellRange, b: &CellRange) -> bool {
    a.start.row <= b.end.row
        && b.start.row <= a.end.row
        && a.start.col <= b.end.col
        && b.start.col <= a.end.col
}

pub(crate) fn same_range(a: &Option<CellRange>, b: &Option<CellRange>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.start.row == b.start.row
                && a.start.col == b.start.col
                && a.end.row == b.end.row
                && a.end.col == b.end.col
        }
        _ => false,
    }
}

pub(crate) fn range_has(range: &CellRange, row: u32, col: u32) -> bool {
    row >= range.start.row && row <= range.end.row && col >= range.start.col && col <= range.end.col
}

pub(crate) fn cell_range(row: u32, col: u32) -> CellRange {
    CellRange {
        start: CellAddress { row, col },
        end: CellAddress { row, col },
    }
}

pub(crate) fn bounding_box(ranges: &[CellRange]) -> CellRange {
    let mut start = ranges[0].start;
    let mut end = ranges[0].end;
    for range in ranges.iter().skip(1) {
        start.row = start.row.min(range.start.row);
        start.col = start.col.min(range.start.col);
        end.row = end.row.max(range.end.row);
        end.col = end.col.max(range.end.col);
    }
    CellRange { start, end }
}

#[derive(Clone)]
struct IxEntry {
    range: CellRange,
    node: u32,
}

struct InsMap<K, V> {
    order: Vec<K>,
    map: HashMap<K, V>,
}

impl<K, V> Default for InsMap<K, V>
where
    K: Copy + Eq + std::hash::Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> InsMap<K, V>
where
    K: Copy + Eq + std::hash::Hash,
{
    fn new() -> Self {
        Self {
            order: Vec::new(),
            map: HashMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.order.len()
    }

    fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    fn keys(&self) -> &[K] {
        &self.order
    }

    fn or_default(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        if !self.map.contains_key(&key) {
            self.order.push(key);
            self.map.insert(key, V::default());
        }
        self.map.get_mut(&key).expect("just inserted")
    }
}

impl<K> InsMap<K, Vec<u32>>
where
    K: Copy + Eq + std::hash::Hash,
{
    fn push_node(&mut self, key: K, node: u32) {
        self.or_default(key).push(node);
    }
}

impl<K> InsMap<K, Vec<IxEntry>>
where
    K: Copy + Eq + std::hash::Hash,
{
    fn push_entry(&mut self, key: K, entry: IxEntry) {
        self.or_default(key).push(entry);
    }
}

/// Insertion-ordered unique node ids (JavaScript `Set` iteration).
#[derive(Default)]
pub(crate) struct InsSet {
    seen: HashSet<u32>,
    pub order: Vec<u32>,
}

impl InsSet {
    pub(crate) fn insert(&mut self, id: u32) -> bool {
        if self.seen.insert(id) {
            self.order.push(id);
            true
        } else {
            false
        }
    }

    pub(crate) fn contains(&self, id: u32) -> bool {
        self.seen.contains(&id)
    }
}

/// Items keyed by the areas they cover.
pub(crate) struct AreaIndex {
    cells: HashMap<u32, InsMap<u64, Vec<u32>>>,
    chunked: HashMap<u32, InsMap<u32, InsMap<u32, Vec<IxEntry>>>>,
    tall: HashMap<u32, InsMap<u32, Vec<IxEntry>>>,
    wide: HashMap<u32, Vec<IxEntry>>,
}

impl AreaIndex {
    pub(crate) fn new() -> Self {
        Self {
            cells: HashMap::new(),
            chunked: HashMap::new(),
            tall: HashMap::new(),
            wide: HashMap::new(),
        }
    }

    pub(crate) fn add(&mut self, area: &Area, node: u32) {
        let start = area.range.start;
        let end = area.range.end;
        let width = end.col - start.col + 1;
        if start.row == end.row && width <= BUCKET_WIDTH {
            let cells = self.cells.entry(area.sheet).or_default();
            for col in start.col..=end.col {
                cells.push_node(cell_key(start.row, col), node);
            }
            return;
        }
        let entry = IxEntry {
            range: area.range.clone(),
            node,
        };
        if width > BUCKET_WIDTH {
            self.wide.entry(area.sheet).or_default().push(entry);
            return;
        }
        let first = start.row >> CHUNK_SHIFT;
        let last = end.row >> CHUNK_SHIFT;
        if last - first + 1 > CHUNKS {
            let cols = self.tall.entry(area.sheet).or_default();
            for col in start.col..=end.col {
                cols.push_entry(col, entry.clone());
            }
            return;
        }
        let cols = self.chunked.entry(area.sheet).or_default();
        for col in start.col..=end.col {
            let chunks = cols.or_default(col);
            for chunk in first..=last {
                chunks.push_entry(chunk, entry.clone());
            }
        }
    }

    pub(crate) fn dependents_into(&self, sheet: u32, range: &CellRange, out: &mut InsSet) {
        self.collect(sheet, range, out);
    }

    fn collect(&self, sheet: u32, range: &CellRange, out: &mut InsSet) {
        let start = range.start;
        let end = range.end;
        if let Some(cells) = self.cells.get(&sheet) {
            let size =
                (end.row as u64 - start.row as u64 + 1) * (end.col as u64 - start.col as u64 + 1);
            if size <= 4096 {
                for row in start.row..=end.row {
                    for col in start.col..=end.col {
                        if let Some(list) = cells.get(&cell_key(row, col)) {
                            for &node in list {
                                out.insert(node);
                            }
                        }
                    }
                }
            } else {
                for &key in cells.keys() {
                    let (row, col) = key_to_rc(key);
                    if row >= start.row && row <= end.row && col >= start.col && col <= end.col {
                        if let Some(list) = cells.get(&key) {
                            for &node in list {
                                out.insert(node);
                            }
                        }
                    }
                }
            }
        }
        let first = start.row >> CHUNK_SHIFT;
        let last = end.row >> CHUNK_SHIFT;
        if let Some(chunked) = self.chunked.get(&sheet) {
            let mut cols: Vec<u32> = Vec::new();
            if end.col - start.col < chunked.len() as u32 {
                for col in start.col..=end.col {
                    if chunked.contains(&col) {
                        cols.push(col);
                    }
                }
            } else {
                for &col in chunked.keys() {
                    if col >= start.col && col <= end.col {
                        cols.push(col);
                    }
                }
            }
            for col in cols {
                let Some(chunks) = chunked.get(&col) else {
                    continue;
                };
                if last - first < chunks.len() as u32 {
                    for chunk in first..=last {
                        push_hits(chunks.get(&chunk), range, out);
                    }
                } else {
                    for &chunk in chunks.keys() {
                        if chunk >= first && chunk <= last {
                            push_hits(chunks.get(&chunk), range, out);
                        }
                    }
                }
            }
        }
        if let Some(tall) = self.tall.get(&sheet) {
            let mut cols: Vec<u32> = Vec::new();
            if end.col - start.col < tall.len() as u32 {
                for col in start.col..=end.col {
                    if tall.contains(&col) {
                        cols.push(col);
                    }
                }
            } else {
                for &col in tall.keys() {
                    if col >= start.col && col <= end.col {
                        cols.push(col);
                    }
                }
            }
            for col in cols {
                push_hits(tall.get(&col), range, out);
            }
        }
        push_hits(self.wide.get(&sheet), range, out);
    }
}

fn push_hits(list: Option<&Vec<IxEntry>>, range: &CellRange, out: &mut InsSet) {
    let Some(list) = list else {
        return;
    };
    for entry in list {
        if ranges_intersect(&entry.range, range) {
            out.insert(entry.node);
        }
    }
}

struct ColSheet {
    order: Vec<u32>,
    cols: HashMap<u32, Vec<(u32, u32)>>,
}

/// Formula nodes by sheet and column, rows sorted.
pub(crate) struct ColumnIndex {
    sheets: HashMap<u32, ColSheet>,
}

impl ColumnIndex {
    pub(crate) fn build(nodes: &[FormulaNode]) -> Self {
        let mut sheets: HashMap<u32, ColSheet> = HashMap::new();
        for (id, node) in nodes.iter().enumerate() {
            if !node.alive {
                continue;
            }
            let sheet = sheets.entry(node.sheet).or_insert_with(|| ColSheet {
                order: Vec::new(),
                cols: HashMap::new(),
            });
            if let Some(list) = sheet.cols.get_mut(&node.col) {
                list.push((node.row, id as u32));
            } else {
                sheet.order.push(node.col);
                sheet.cols.insert(node.col, vec![(node.row, id as u32)]);
            }
        }
        for sheet in sheets.values_mut() {
            for list in sheet.cols.values_mut() {
                list.sort_by_key(|(row, _)| *row);
            }
        }
        Self { sheets }
    }

    pub(crate) fn nodes_in(&self, area: &Area, out: &mut Vec<u32>) {
        let Some(sheet) = self.sheets.get(&area.sheet) else {
            return;
        };
        let start = area.range.start;
        let end = area.range.end;
        let width = end.col - start.col + 1;
        let scan = |list: &[(u32, u32)], out: &mut Vec<u32>| {
            let mut lo = 0usize;
            let mut hi = list.len();
            while lo < hi {
                let mid = (lo + hi) >> 1;
                if list[mid].0 < start.row {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            for item in list.iter().skip(lo) {
                if item.0 > end.row {
                    break;
                }
                out.push(item.1);
            }
        };
        if width <= sheet.cols.len() as u32 {
            for col in start.col..=end.col {
                if let Some(list) = sheet.cols.get(&col) {
                    scan(list, out);
                }
            }
        } else {
            for &col in &sheet.order {
                if col >= start.col && col <= end.col {
                    if let Some(list) = sheet.cols.get(&col) {
                        scan(list, out);
                    }
                }
            }
        }
    }
}

/// Which formulas read a cell: static and dynamic precedents.
pub(crate) struct ReverseIndex {
    index: AreaIndex,
}

impl ReverseIndex {
    pub(crate) fn new() -> Self {
        Self {
            index: AreaIndex::new(),
        }
    }

    pub(crate) fn add_node(&mut self, node_id: u32, deps: &[Area], dynamic_deps: &[Area]) {
        for dep in deps {
            self.index.add(dep, node_id);
        }
        for dep in dynamic_deps {
            self.index.add(dep, node_id);
        }
    }

    pub(crate) fn dependents(&self, sheet: u32, range: &CellRange, out: &mut InsSet) {
        self.index.dependents_into(sheet, range, out);
    }
}
