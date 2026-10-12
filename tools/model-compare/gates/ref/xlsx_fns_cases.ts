// SPDX-License-Identifier: MIT
// Sweep cases for Math & Trig, Statistical (#73), and Logical, Text,
// Information, and Date & Time (#74).
// Sources: FUNCTION_CATALOG, plus inputs from
// upstream/src/core/xlsx/formula/functions/functions.test.ts,
// hypothesis-tests.test.ts and distributions-f.test.ts.
// Reads UPSTREAM_DIR. Prints a JSON array of
// {formula, cells?, hidden?, sheet?, row?, col?, sheets?, date1904?, source}.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR;
if (!up) {
	console.error('xlsx_fns_cases: UPSTREAM_DIR is not set');
	process.exit(2);
}

const { FUNCTION_CATALOG, getFunction } = await import(
	`${up}/src/core/xlsx/formula/functions/registry.ts`
);
const { parseFormula } = await import(`${up}/src/core/xlsx/formula/parser.ts`);

const FAMILIES = new Set([
	'Math & Trig',
	'Statistical',
	'Logical',
	'Text',
	'Information',
	'Date & Time',
]);

/** Constants shared by range cases. No formula cells, so RAND's seed stays put. */
const GRID: Record<string, unknown> = {
	A1: 1,
	A2: 2,
	A3: 3,
	A4: 4,
	A5: 5,
	A6: 6,
	B1: 4,
	B2: 5,
	B3: 6,
	B4: 7,
	B5: 8,
	B6: 9,
	C1: 'text',
	C2: true,
	C3: false,
	C4: '',
	C5: '5',
	C6: null,
	D1: 1,
	D2: 1,
	D3: 2,
	D4: 2,
	D5: 3,
	E1: 10,
	E2: 0,
	E3: 2,
	F1: 1,
	F2: 2,
	F3: 3,
	G1: 2,
	G2: 4,
	G3: 6,
};

type Case = {
	formula: string;
	cells?: Record<string, unknown>;
	hidden?: Record<string, number[]>;
	sheet?: string;
	row?: number;
	col?: number;
	sheets?: string[];
	source: string;
	kind?: string;
	date1904?: boolean;
};

const SRC = 'upstream functions.test.ts / hypothesis-tests.test.ts / distributions-f.test.ts';

/** Meaningful calls. Names omitted here get `NAME(1..minArgs)`. */
const OVERRIDES: Record<string, string> = {
	ABS: 'ABS(-3.5)',
	ACOS: 'ACOS(0.5)',
	ACOSH: 'ACOSH(2)',
	ACOT: 'ACOT(1)',
	ACOTH: 'ACOTH(2)',
	AGGREGATE: 'AGGREGATE(9,6,A1:A6)',
	ARABIC: 'ARABIC("XIV")',
	ASIN: 'ASIN(0.5)',
	ASINH: 'ASINH(1)',
	ATAN: 'ATAN(1)',
	ATAN2: 'ATAN2(1,1)',
	ATANH: 'ATANH(0.5)',
	BASE: 'BASE(13,2,4)',
	CEILING: 'CEILING(2.5,1)',
	'CEILING.MATH': 'CEILING.MATH(-2.5,1,-1)',
	'CEILING.PRECISE': 'CEILING.PRECISE(-2.5,1)',
	COMBIN: 'COMBIN(8,3)',
	COMBINA: 'COMBINA(4,3)',
	COS: 'COS(1)',
	COSH: 'COSH(1)',
	COT: 'COT(1)',
	COTH: 'COTH(1)',
	CSC: 'CSC(1)',
	CSCH: 'CSCH(1)',
	DECIMAL: 'DECIMAL("1101",2)',
	DEGREES: 'DEGREES(PI())',
	EVEN: 'EVEN(-3)',
	EXP: 'EXP(1)',
	FACT: 'FACT(5)',
	FACTDOUBLE: 'FACTDOUBLE(7)',
	FLOOR: 'FLOOR(-2.5,-1)',
	'FLOOR.MATH': 'FLOOR.MATH(-2.5,1,-1)',
	'FLOOR.PRECISE': 'FLOOR.PRECISE(-2.5,1)',
	GCD: 'GCD(24,18,12)',
	INT: 'INT(-1.2)',
	'ISO.CEILING': 'ISO.CEILING(-2.5,1)',
	LCM: 'LCM(4,6,8)',
	LN: 'LN(10)',
	LOG: 'LOG(1000,10)',
	LOG10: 'LOG10(1000)',
	MDETERM: 'MDETERM({1,2;3,4})',
	MINVERSE: 'MINVERSE({1,2;3,5})',
	MMULT: 'MMULT({1,2;3,4},{1,0;0,1})',
	MOD: 'MOD(0.3,0.1)',
	MROUND: 'MROUND(6.05,0.1)',
	MULTINOMIAL: 'MULTINOMIAL(2,3,4)',
	MUNIT: 'MUNIT(3)',
	ODD: 'ODD(-3)',
	PI: 'PI()',
	POWER: 'POWER(-8,1/3)',
	PRODUCT: 'PRODUCT(A1:A3,2)',
	QUOTIENT: 'QUOTIENT(-10,3)',
	RADIANS: 'RADIANS(180)',
	RAND: 'RAND()',
	RANDBETWEEN: 'RANDBETWEEN(1,10)',
	ROMAN: 'ROMAN(1994)',
	ROUND: 'ROUND(2.567,)',
	ROUNDDOWN: 'ROUNDDOWN(-1.9,0)',
	ROUNDUP: 'ROUNDUP(-1.1,0)',
	SEC: 'SEC(1)',
	SECH: 'SECH(1)',
	SERIESSUM: 'SERIESSUM(2,0,1,{1,2,3})',
	SIGN: 'SIGN(-2)',
	SIN: 'SIN(1)',
	SINH: 'SINH(1)',
	SQRT: 'SQRT(2)',
	SQRTPI: 'SQRTPI(1)',
	SUBTOTAL: 'SUBTOTAL(9,A1:A3)',
	SUM: 'SUM(A1:A3)',
	SUMIF: 'SUMIF(A1:A3,">0",B1:B3)',
	SUMIFS: 'SUMIFS(B1:B3,A1:A3,">1",E1:E3,">=0")',
	SUMPRODUCT: 'SUMPRODUCT(A1:A3,B1:B3)',
	SUMSQ: 'SUMSQ(A1:A3)',
	SUMX2MY2: 'SUMX2MY2(A1:A3,B1:B3)',
	SUMX2PY2: 'SUMX2PY2(A1:A3,B1:B3)',
	SUMXMY2: 'SUMXMY2(A1:A3,B1:B3)',
	TAN: 'TAN(1)',
	TANH: 'TANH(1)',
	TRUNC: 'TRUNC(-1.9)',
	AVEDEV: 'AVEDEV(A1:A6)',
	AVERAGE: 'AVERAGE(A1:A6)',
	AVERAGEA: 'AVERAGEA(A1:C3)',
	AVERAGEIF: 'AVERAGEIF(A1:A6,">2")',
	AVERAGEIFS: 'AVERAGEIFS(B1:B6,A1:A6,">=2")',
	'BETA.DIST': 'BETA.DIST(0.4,2,3,TRUE,0,1)',
	'BETA.INV': 'BETA.INV(0.5,2,3)',
	BETADIST: 'BETADIST(2,8,10,1,3)',
	BETAINV: 'BETAINV(0.6854706,8,10,1,3)',
	'BINOM.DIST': 'BINOM.DIST(3,10,0.5,TRUE)',
	'BINOM.DIST.RANGE': 'BINOM.DIST.RANGE(10,0.5,2,4)',
	'BINOM.INV': 'BINOM.INV(10,0.5,0.5)',
	BINOMDIST: 'BINOMDIST(3,10,0.5,FALSE)',
	CHIDIST: 'CHIDIST(2,3)',
	CHIINV: 'CHIINV(0.05,3)',
	'CHISQ.DIST': 'CHISQ.DIST(2,3,TRUE)',
	'CHISQ.DIST.RT': 'CHISQ.DIST.RT(2,3)',
	'CHISQ.INV': 'CHISQ.INV(0.95,3)',
	'CHISQ.INV.RT': 'CHISQ.INV.RT(0.05,3)',
	'CHISQ.TEST': 'CHISQ.TEST(A1:B2,F1:G2)',
	CHITEST: 'CHITEST(A1:B2,F1:G2)',
	CONFIDENCE: 'CONFIDENCE(0.05,1,30)',
	'CONFIDENCE.NORM': 'CONFIDENCE.NORM(0.05,1,30)',
	'CONFIDENCE.T': 'CONFIDENCE.T(0.05,1,30)',
	CORREL: 'CORREL(A1:A6,B1:B6)',
	COUNT: 'COUNT(A1:C6)',
	COUNTA: 'COUNTA(A1:C6)',
	COUNTBLANK: 'COUNTBLANK(C1:C6)',
	COUNTIF: 'COUNTIF(A1:A6,">3")',
	COUNTIFS: 'COUNTIFS(A1:A6,">=2",B1:B6,">4")',
	COVAR: 'COVAR(A1:A6,B1:B6)',
	'COVARIANCE.P': 'COVARIANCE.P(A1:A6,B1:B6)',
	'COVARIANCE.S': 'COVARIANCE.S(A1:A6,B1:B6)',
	CRITBINOM: 'CRITBINOM(10,0.5,0.5)',
	DEVSQ: 'DEVSQ(A1:A6)',
	'EXPON.DIST': 'EXPON.DIST(0.5,2,TRUE)',
	EXPONDIST: 'EXPONDIST(0.5,2,FALSE)',
	'F.DIST': 'F.DIST(2,5,8,TRUE)',
	'F.DIST.RT': 'F.DIST.RT(2,5,8)',
	'F.INV': 'F.INV(0.95,5,8)',
	'F.INV.RT': 'F.INV.RT(0.05,5,8)',
	'F.TEST': 'F.TEST(A1:A6,B1:B6)',
	FDIST: 'FDIST(2,5,8)',
	FINV: 'FINV(0.05,5,8)',
	FISHER: 'FISHER(0.5)',
	FISHERINV: 'FISHERINV(0.5)',
	FORECAST: 'FORECAST(6,B1:B6,A1:A6)',
	'FORECAST.LINEAR': 'FORECAST.LINEAR(6,B1:B6,A1:A6)',
	FREQUENCY: 'FREQUENCY({1,2,3,4,5},{2,4})',
	FTEST: 'FTEST(A1:A6,B1:B6)',
	GAMMA: 'GAMMA(2.5)',
	'GAMMA.DIST': 'GAMMA.DIST(2,3,1,TRUE)',
	'GAMMA.INV': 'GAMMA.INV(0.5,3,1)',
	GAMMADIST: 'GAMMADIST(2,3,1,TRUE)',
	GAMMAINV: 'GAMMAINV(0.5,3,1)',
	GAMMALN: 'GAMMALN(2.5)',
	'GAMMALN.PRECISE': 'GAMMALN.PRECISE(2.5)',
	GAUSS: 'GAUSS(1)',
	GEOMEAN: 'GEOMEAN(A1:A4)',
	GROWTH: 'GROWTH(B1:B4,A1:A4)',
	HARMEAN: 'HARMEAN(A1:A4)',
	'HYPGEOM.DIST': 'HYPGEOM.DIST(1,4,8,20,FALSE)',
	HYPGEOMDIST: 'HYPGEOMDIST(1,4,8,20)',
	INTERCEPT: 'INTERCEPT(B1:B6,A1:A6)',
	KURT: 'KURT(A1:A6,B1)',
	LARGE: 'LARGE(A1:A6,2)',
	LINEST: 'LINEST(B1:B6,A1:A6,TRUE,TRUE)',
	LOGEST: 'LOGEST(B1:B6,A1:A6,TRUE,TRUE)',
	LOGINV: 'LOGINV(0.5,1,0.5)',
	'LOGNORM.DIST': 'LOGNORM.DIST(2,1,0.5,TRUE)',
	'LOGNORM.INV': 'LOGNORM.INV(0.5,1,0.5)',
	LOGNORMDIST: 'LOGNORMDIST(2,1,0.5)',
	MAX: 'MAX(A1:C6)',
	MAXA: 'MAXA(A1:C3)',
	MAXIFS: 'MAXIFS(B1:B6,A1:A6,">=3")',
	MEDIAN: 'MEDIAN(A1:A6)',
	MIN: 'MIN(A1:C6)',
	MINA: 'MINA(A1:C3)',
	MINIFS: 'MINIFS(B1:B6,A1:A6,">=3")',
	MODE: 'MODE(D1:D5)',
	'MODE.MULT': 'MODE.MULT({1,1,2,2,3})',
	'MODE.SNGL': 'MODE.SNGL(D1:D5)',
	'NEGBINOM.DIST': 'NEGBINOM.DIST(2,3,0.5,FALSE)',
	NEGBINOMDIST: 'NEGBINOMDIST(2,3,0.5)',
	'NORM.DIST': 'NORM.DIST(1,0,1,TRUE)',
	'NORM.INV': 'NORM.INV(0.9,0,1)',
	'NORM.S.DIST': 'NORM.S.DIST(1,TRUE)',
	'NORM.S.INV': 'NORM.S.INV(0.975)',
	NORMDIST: 'NORMDIST(1,0,1,TRUE)',
	NORMINV: 'NORMINV(0.9,0,1)',
	NORMSDIST: 'NORMSDIST(1)',
	NORMSINV: 'NORMSINV(0.975)',
	PEARSON: 'PEARSON(A1:A6,B1:B6)',
	PERCENTILE: 'PERCENTILE(A1:A6,0.9)',
	'PERCENTILE.EXC': 'PERCENTILE.EXC(A1:A6,0.5)',
	'PERCENTILE.INC': 'PERCENTILE.INC(A1:A6,0.9)',
	PERCENTRANK: 'PERCENTRANK(A1:A6,3)',
	'PERCENTRANK.EXC': 'PERCENTRANK.EXC(A1:A6,3)',
	'PERCENTRANK.INC': 'PERCENTRANK.INC(A1:A6,3)',
	PERMUT: 'PERMUT(8,3)',
	PERMUTATIONA: 'PERMUTATIONA(4,3)',
	PHI: 'PHI(0)',
	POISSON: 'POISSON(2,3,FALSE)',
	'POISSON.DIST': 'POISSON.DIST(2,3,TRUE)',
	PROB: 'PROB({1,2,3},{0.2,0.3,0.5},2,3)',
	QUARTILE: 'QUARTILE(A1:A6,1)',
	'QUARTILE.EXC': 'QUARTILE.EXC(A1:A6,1)',
	'QUARTILE.INC': 'QUARTILE.INC(A1:A6,3)',
	RANK: 'RANK(3,A1:A6)',
	'RANK.AVG': 'RANK.AVG(2,D1:D5)',
	'RANK.EQ': 'RANK.EQ(3,A1:A6,1)',
	RSQ: 'RSQ(B1:B6,A1:A6)',
	SKEW: 'SKEW(A1:A6)',
	'SKEW.P': 'SKEW.P(A1:A6)',
	SLOPE: 'SLOPE(B1:B6,A1:A6)',
	SMALL: 'SMALL(A1:A6,2)',
	STANDARDIZE: 'STANDARDIZE(4,3,1.5)',
	STDEV: 'STDEV(A1:A6)',
	'STDEV.P': 'STDEV.P(A1:A6)',
	'STDEV.S': 'STDEV.S(A1:A6)',
	STDEVA: 'STDEVA(A1:C3)',
	STDEVPA: 'STDEVPA(A1:C3)',
	STDEVP: 'STDEVP(A1:A6)',
	STEYX: 'STEYX(B1:B6,A1:A6)',
	'T.DIST': 'T.DIST(1.5,10,TRUE)',
	'T.DIST.2T': 'T.DIST.2T(1.5,10)',
	'T.DIST.RT': 'T.DIST.RT(1.5,10)',
	'T.INV': 'T.INV(0.95,10)',
	'T.INV.2T': 'T.INV.2T(0.05,10)',
	'T.TEST': 'T.TEST(A1:A6,B1:B6,2,1)',
	TDIST: 'TDIST(1.5,10,2)',
	TINV: 'TINV(0.05,10)',
	TREND: 'TREND(B1:B4,A1:A4)',
	TRIMMEAN: 'TRIMMEAN(A1:A6,0.2)',
	TTEST: 'TTEST(A1:A6,B1:B6,2,1)',
	VAR: 'VAR(A1:A6)',
	'VAR.P': 'VAR.P(A1:A6)',
	'VAR.S': 'VAR.S(A1:A6)',
	VARA: 'VARA(A1:C3)',
	VARP: 'VARP(A1:A6)',
	VARPA: 'VARPA(A1:C3)',
	WEIBULL: 'WEIBULL(2,3,1,TRUE)',
	'WEIBULL.DIST': 'WEIBULL.DIST(2,3,1,FALSE)',
	'Z.TEST': 'Z.TEST(A1:A6,4)',
	ZTEST: 'ZTEST(A1:A6,4,1)',
	AND: 'AND(TRUE,1,"TRUE")',
	BYCOL: 'BYCOL(A1:B2,LAMBDA(c,SUM(c)))',
	BYROW: 'BYROW(A1:B2,LAMBDA(r,SUM(r)))',
	FALSE: 'FALSE()',
	IF: 'IF(C2,A1,B1)',
	IFERROR: 'IFERROR(1/0,9)',
	IFNA: 'IFNA(NA(),3)',
	IFS: 'IFS(FALSE,1,TRUE,2)',
	LAMBDA: 'LAMBDA(x,x+1)(5)',
	LET: 'LET(x,1,y,x+2,y+3)',
	MAKEARRAY: 'MAKEARRAY(2,3,LAMBDA(r,c,r*10+c))',
	MAP: 'MAP(A1:A3,LAMBDA(x,x*2))',
	NOT: 'NOT(C3)',
	OR: 'OR(FALSE,0,C2)',
	REDUCE: 'REDUCE(0,A1:A3,LAMBDA(a,b,a+b))',
	SCAN: 'SCAN(0,A1:A3,LAMBDA(a,b,a+b))',
	SWITCH: 'SWITCH(2,1,"a",2,"b","c")',
	TRUE: 'TRUE()',
	XOR: 'XOR(TRUE,TRUE,FALSE)',
	ARRAYTOTEXT: 'ARRAYTOTEXT(A1:B2,1)',
	CHAR: 'CHAR(65)',
	CLEAN: 'CLEAN(CHAR(9)&"ab")',
	CODE: 'CODE("A")',
	CONCAT: 'CONCAT("a",C1,1)',
	CONCATENATE: 'CONCATENATE("a","b",C5)',
	DOLLAR: 'DOLLAR(1234.567,2)',
	ENCODEURL: 'ENCODEURL("a b/c")',
	EXACT: 'EXACT("Ab","ab")',
	FIND: 'FIND("b","abc",1)',
	FINDB: 'FINDB("b","abc")',
	FIXED: 'FIXED(1234.567,1,FALSE)',
	LEFT: 'LEFT("héllo",3)',
	LEFTB: 'LEFTB("héllo",3)',
	LEN: 'LEN("héllo")',
	LENB: 'LENB("héllo")',
	LOWER: 'LOWER("HeLLo")',
	MID: 'MID("abcd",2,2)',
	MIDB: 'MIDB("abcd",2,2)',
	NUMBERVALUE: 'NUMBERVALUE("1.234,56",",",".")',
	PROPER: 'PROPER("hello WORLD")',
	REGEXEXTRACT: 'REGEXEXTRACT("ab12cd","\\d+")',
	REGEXREPLACE: 'REGEXREPLACE("a1b2","\\d","#")',
	REGEXTEST: 'REGEXTEST("abc","B",1)',
	REPLACE: 'REPLACE("abcd",2,2,"XY")',
	REPLACEB: 'REPLACEB("abcd",2,2,"XY")',
	REPT: 'REPT("ab",3)',
	RIGHT: 'RIGHT("abcd",2)',
	RIGHTB: 'RIGHTB("abcd",2)',
	SEARCH: 'SEARCH("B*","xabc")',
	SEARCHB: 'SEARCHB("B*","xabc")',
	SUBSTITUTE: 'SUBSTITUTE("abab","a","x",2)',
	T: 'T(C1)',
	TEXT: 'TEXT(DATE(2020,1,15),"yyyy-mm-dd")',
	TEXTAFTER: 'TEXTAFTER("a-b-c","-",2)',
	TEXTBEFORE: 'TEXTBEFORE("a-b-c","-",2)',
	TEXTJOIN: 'TEXTJOIN(",",TRUE,A1,C4,C1)',
	TEXTSPLIT: 'TEXTSPLIT("a,b;c,d",",",";")',
	TRIM: 'TRIM("  a   b  ")',
	UNICHAR: 'UNICHAR(233)',
	UNICODE: 'UNICODE("é")',
	UPPER: 'UPPER("hello")',
	VALUE: 'VALUE("$1,234.50")',
	VALUETOTEXT: 'VALUETOTEXT("a b",1)',
	CELL: 'CELL("address",A1)',
	'ERROR.TYPE': 'ERROR.TYPE(1/0)',
	ISBLANK: 'ISBLANK(C6)',
	ISERR: 'ISERR(1/0)',
	ISERROR: 'ISERROR(NA())',
	ISEVEN: 'ISEVEN(2.9)',
	ISFORMULA: 'ISFORMULA(A1)',
	ISLOGICAL: 'ISLOGICAL(C2)',
	ISNA: 'ISNA(NA())',
	ISNONTEXT: 'ISNONTEXT(A1)',
	ISNUMBER: 'ISNUMBER(C5)',
	ISODD: 'ISODD(3)',
	ISREF: 'ISREF(A1:B2)',
	ISTEXT: 'ISTEXT(C1)',
	N: 'N(C2)',
	NA: 'NA()',
	SHEET: 'SHEET("Sheet2")',
	SHEETS: 'SHEETS()',
	TYPE: 'TYPE(A1:A2)',
	DATE: 'DATE(2020,1,15)',
	DATEDIF: 'DATEDIF(DATE(2020,1,31),DATE(2021,3,1),"YM")',
	DATEVALUE: 'DATEVALUE("2020-01-15")',
	DAY: 'DAY(60)',
	DAYS: 'DAYS(DATE(2020,1,10),DATE(2020,1,1))',
	DAYS360: 'DAYS360(DATE(2020,1,31),DATE(2020,2,29),FALSE)',
	EDATE: 'EDATE(DATE(2020,1,31),1)',
	EOMONTH: 'EOMONTH(DATE(2020,1,15),1)',
	HOUR: 'HOUR(TIME(13,2,3))',
	ISOWEEKNUM: 'ISOWEEKNUM(1)',
	MINUTE: 'MINUTE(TIME(1,2,3))',
	MONTH: 'MONTH(DATE(2020,2,29))',
	NETWORKDAYS: 'NETWORKDAYS(DATE(2020,1,1),DATE(2020,1,10))',
	'NETWORKDAYS.INTL': 'NETWORKDAYS.INTL(DATE(2020,1,1),DATE(2020,1,10),"0000011")',
	NOW: 'NOW()',
	SECOND: 'SECOND(TIME(1,2,3))',
	TIME: 'TIME(1,30,0)',
	TIMEVALUE: 'TIMEVALUE("1:30 PM")',
	TODAY: 'TODAY()',
	WEEKDAY: 'WEEKDAY(1,1)',
	WEEKNUM: 'WEEKNUM(DATE(2020,1,15),2)',
	WORKDAY: 'WORKDAY(DATE(2020,1,1),5)',
	'WORKDAY.INTL': 'WORKDAY.INTL(DATE(2020,1,1),5,"0000011")',
	YEAR: 'YEAR(DATE(2020,1,15))',
	YEARFRAC: 'YEARFRAC(DATE(2020,1,1),DATE(2021,1,1),1)',
};

function autoFormula(name: string, minArgs: number): string {
	if (minArgs <= 0) return `${name}()`;
	const args: string[] = [];
	for (let i = 1; i <= minArgs; i++) args.push(String(i));
	return `${name}(${args.join(',')})`;
}

function sweepCases(): Case[] {
	const catalog = FUNCTION_CATALOG.filter((info: { category: string }) => FAMILIES.has(info.category));
	const cases: Case[] = [];
	const seen = new Set<string>();
	for (const info of catalog) {
		const spec = getFunction(info.name);
		if (!spec) throw new Error(`missing spec ${info.name}`);
		const formula = OVERRIDES[info.name] ?? autoFormula(info.name, spec.minArgs);
		seen.add(info.name);
		const volatile = info.name === 'RAND' || info.name === 'RANDBETWEEN';
		const item: Case = {
			formula,
			cells: volatile ? {} : GRID,
			source: OVERRIDES[info.name] ? SRC : `FUNCTION_CATALOG minArgs ${info.category}`,
			kind: 'sweep',
		};
		cases.push(item);
		if (info.category === 'Date & Time') {
			cases.push({ ...item, date1904: true, source: `${item.source} (1904)` });
		}
	}
	const missing = catalog.filter((info: { name: string }) => !seen.has(info.name)).map((info: { name: string }) => info.name);
	if (missing.length) throw new Error(`sweep missed ${missing.join(',')}`);
	// Edges the minArgs call does not reach. Each name above is already covered.
	const edges: Case[] = [
		{ formula: 'SUM(1,"2",TRUE)', source: SRC, kind: 'sweep' },
		{ formula: 'SUM(A1:A3)', cells: { A1: 1, A2: '2', A3: true }, source: SRC, kind: 'sweep' },
		{ formula: 'SUM("x")', source: SRC, kind: 'sweep' },
		{ formula: 'POWER(0,0)', source: 'operators.ts 0^0 is #NUM!', kind: 'sweep' },
		{ formula: '1/0', source: 'operators.ts divide by zero', kind: 'sweep' },
		{ formula: 'FACT(171)', source: 'math-combin.ts factorial past 170', kind: 'sweep' },
		{ formula: 'MOD(-3,2)', source: 'math-mod.ts remainder sign', kind: 'sweep' },
		{ formula: 'MINVERSE({1,2;2,4})', source: SRC, kind: 'sweep' },
		{ formula: 'MMULT({1,2},{1,2})', source: SRC, kind: 'sweep' },
		{
			formula: 'SUBTOTAL(9,A1:A3)',
			cells: { A1: 1, A2: 2, A3: '=SUBTOTAL(9,A1:A2)' },
			source: SRC,
			kind: 'sweep',
		},
		{
			formula: 'SUBTOTAL(109,A1:A3)',
			cells: { A1: 10, A2: 20, A3: 30 },
			hidden: { Sheet1: [1] },
			source: 'math-sum.ts hidden rows',
			kind: 'sweep',
		},
		{
			formula: 'COUNTIF(A1:A6,5)',
			cells: { A1: 5, A2: '5', A3: 5, A4: 'a*b', A5: '', A6: 'x5' },
			source: SRC,
			kind: 'sweep',
		},
		{
			formula: 'COUNTIF(A1:A6,"=")',
			cells: { A1: 5, A2: '5', A3: 5, A4: 'a*b', A5: '', A6: 'x5' },
			source: SRC,
			kind: 'sweep',
		},
		{
			formula: 'COUNTIF(A1:A6,"*5")',
			cells: { A1: 5, A2: '5', A3: 5, A4: 'a*b', A5: '', A6: 'x5' },
			source: SRC,
			kind: 'sweep',
		},
		{ formula: 'ROUND({1.25,1.35},1)', source: 'math-mod.ts array lift', kind: 'sweep' },
		{ formula: 'SUM(A1:B2 B2:C3)', cells: GRID, source: 'intersect operator', kind: 'sweep' },
		{ formula: 'AVERAGE(A1:A2)', cells: { A1: 'a' }, source: SRC, kind: 'sweep' },
		{ formula: 'MAX(A1:A2)', cells: { A1: 'a' }, source: SRC, kind: 'sweep' },
		{ formula: 'NORM.S.INV(0.5)', source: 'distributions.ts', kind: 'sweep' },
		{ formula: 'LINEST(B1:B3)', cells: GRID, source: 'regression.ts default x', kind: 'sweep' },
		{ formula: 'IF(TRUE,1,1/0)', cells: GRID, source: 'logical.ts short-circuit', kind: 'sweep' },
		{ formula: 'IF(FALSE,1/0,2)', cells: GRID, source: 'logical.ts short-circuit', kind: 'sweep' },
		{ formula: 'IF({TRUE,FALSE},10,20)', cells: GRID, source: 'logical.ts elementwise', kind: 'sweep' },
		{
			formula: 'IFERROR(H1:I1,9)',
			cells: { ...GRID, H1: 1, I1: '=1/0' },
			source: 'logical.ts',
			kind: 'sweep',
		},
		{ formula: 'IFNA(1/0,3)', cells: GRID, source: 'logical.ts', kind: 'sweep' },
		{ formula: 'AND(A1:C3)', cells: GRID, source: 'logical.ts range skip', kind: 'sweep' },
		{ formula: 'OR(C1:C6)', cells: GRID, source: 'logical.ts', kind: 'sweep' },
		{ formula: 'LET(f,LAMBDA(x,x+1),f(4))', cells: GRID, source: 'logical.ts', kind: 'sweep' },
		{ formula: 'MAP({1,2},{10;20},LAMBDA(a,b,a+b))', cells: GRID, source: 'lambda.ts', kind: 'sweep' },
		{
			formula: 'ISFORMULA(B1)',
			cells: { ...GRID, B1: '=A1+1' },
			source: 'info.ts',
			kind: 'sweep',
		},
		{ formula: 'TYPE(1)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'TYPE("a")', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'TYPE(TRUE)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'TYPE(NA())', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'TYPE({1,2})', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'TYPE(LAMBDA(x,x))', cells: GRID, source: 'info.ts lambda', kind: 'sweep' },
		{ formula: 'ERROR.TYPE(NA())', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'ERROR.TYPE(1)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'SHEET()', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'SHEET(Sheet2!A1)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'SHEETS(A1)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'SHEETS(Sheet1:Sheet2!A1)', cells: GRID, source: 'info.ts 3d', kind: 'sweep' },
		{ formula: 'CELL("address")', cells: GRID, source: 'info.ts anchor', kind: 'sweep' },
		{ formula: 'CELL("contents",C6)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'CELL("type",C1)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'CELL("col",A1)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'ISEVEN(TRUE)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'ISODD(1/0)', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'ISBLANK(C4)', cells: GRID, source: 'info.ts empty text', kind: 'sweep' },
		{ formula: 'N("x")', cells: GRID, source: 'info.ts', kind: 'sweep' },
		{ formula: 'FIND("é","aé",1)', cells: GRID, source: 'text.ts utf16', kind: 'sweep' },
		{ formula: 'SEARCH("a?c","xabc")', cells: GRID, source: 'text.ts wildcard', kind: 'sweep' },
		{ formula: 'TEXTBEFORE("a-b","/",,,,"missing")', cells: GRID, source: 'text-extra.ts', kind: 'sweep' },
		{ formula: 'TEXTSPLIT("a,,b",",",,TRUE)', cells: GRID, source: 'text-extra.ts', kind: 'sweep' },
		{ formula: 'VALUETOTEXT(1,0)', cells: GRID, source: 'text-extra.ts', kind: 'sweep' },
		{ formula: 'CODE("€")', cells: GRID, source: 'text-convert.ts', kind: 'sweep' },
		{ formula: 'CHAR(128)', cells: GRID, source: 'text-convert.ts cp1252', kind: 'sweep' },
	];
	const dateEdges: Case[] = [
		{ formula: 'DATE(1900,2,29)', cells: GRID, source: 'date-calc.ts leap day', kind: 'sweep' },
		{ formula: 'DATE(1900,3,1)', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'DATE(1899,12,31)', cells: GRID, source: 'date-calc.ts year window', kind: 'sweep' },
		{ formula: 'DAY(DATE(2020,2,29))', cells: GRID, source: 'datetime.ts', kind: 'sweep' },
		{ formula: 'WEEKDAY(1,3)', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'WEEKNUM(DATE(2020,1,1),21)', cells: GRID, source: 'date-calc.ts iso', kind: 'sweep' },
		{ formula: 'EDATE(DATE(2020,1,31),-1)', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'EOMONTH(DATE(2020,1,31),1)', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'DATEDIF(DATE(2020,1,31),DATE(2020,3,30),"MD")', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'DAYS360(DATE(2020,1,31),DATE(2020,3,31),TRUE)', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'YEARFRAC(DATE(2020,1,1),DATE(2020,7,1),0)', cells: GRID, source: 'date-calc.ts', kind: 'sweep' },
		{ formula: 'YEARFRAC(DATE(2019,1,1),DATE(2020,1,1),1)', cells: GRID, source: 'date-calc.ts actual/actual', kind: 'sweep' },
		{
			formula: 'NETWORKDAYS(DATE(2020,1,1),DATE(2020,1,10),DATE(2020,1,1))',
			cells: GRID,
			source: 'workdays.ts holiday',
			kind: 'sweep',
		},
		{
			formula: 'WORKDAY(DATE(2020,1,1),1,H1)',
			cells: { ...GRID, H1: '=DATE(2020,1,2)' },
			source: 'workdays.ts holiday',
			kind: 'sweep',
		},
		{ formula: 'WORKDAY.INTL(DATE(2020,1,6),1,1)', cells: GRID, source: 'workdays.ts weekend code', kind: 'sweep' },
		{ formula: 'TIME(25,0,0)', cells: GRID, source: 'datetime.ts', kind: 'sweep' },
		{ formula: 'DATEVALUE("1/2/2020")', cells: GRID, source: 'datetime.ts', kind: 'sweep' },
		{ formula: 'TIMEVALUE("13:05:07")', cells: GRID, source: 'datetime.ts', kind: 'sweep' },
		{ formula: 'TEXT(DATE(2020,1,15),"yyyy-mm-dd")', cells: GRID, source: 'text-convert.ts date format', kind: 'sweep' },
		{ formula: 'VALUE("2020-01-15")', cells: GRID, source: 'text-convert.ts date text', kind: 'sweep' },
	];
	cases.push(...edges);
	for (const item of dateEdges) {
		cases.push(item);
		cases.push({ ...item, date1904: true, source: `${item.source} (1904)` });
	}
	const names = new Set<string>();
	for (const item of cases) {
		let ast: { type?: string; name?: string; args?: unknown[]; operand?: unknown; left?: unknown; right?: unknown; callee?: unknown };
		try {
			ast = parseFormula(item.formula);
		} catch (err) {
			throw new Error(`sweep formula does not parse: ${item.formula}: ${err}`);
		}
		walk(ast, names);
	}
	const uncovered = catalog
		.map((info: { name: string }) => info.name)
		.filter((name: string) => !names.has(name));
	if (uncovered.length) throw new Error(`catalog names missing from sweep: ${uncovered.join(',')}`);
	return cases;
}

function walk(node: unknown, names: Set<string>): void {
	if (!node || typeof node !== 'object') return;
	const ast = node as { type?: string; name?: string; args?: unknown[]; operand?: unknown; left?: unknown; right?: unknown; callee?: unknown };
	if (ast.type === 'call' && ast.name) names.add(ast.name);
	if (Array.isArray(ast.args)) for (const arg of ast.args) walk(arg, names);
	walk(ast.operand, names);
	walk(ast.left, names);
	walk(ast.right, names);
	walk(ast.callee, names);
}

function inScope(node: unknown, allowed: Set<string>): boolean {
	if (!node || typeof node !== 'object') return true;
	const ast = node as {
		type?: string;
		name?: string;
		args?: unknown[];
		operand?: unknown;
		left?: unknown;
		right?: unknown;
	};
	if (ast.type === 'name' || ast.type === 'structured') return false;
	if (ast.type === 'invoke') {
		const invoked = ast as { callee?: unknown; args?: unknown[] };
		return inScope(invoked.callee, allowed) && (invoked.args ?? []).every((arg) => inScope(arg, allowed));
	}
	if (ast.type === 'call') {
		if (!ast.name || !allowed.has(ast.name)) return false;
		return (ast.args ?? []).every((arg) => inScope(arg, allowed));
	}
	if (ast.type === 'unary' || ast.type === 'percent') return inScope(ast.operand, allowed);
	if (ast.type === 'binary') return inScope(ast.left, allowed) && inScope(ast.right, allowed);
	return true;
}

function filterCorpus(path: string): void {
	const allowed = new Set(
		FUNCTION_CATALOG.filter((info: { category: string }) => FAMILIES.has(info.category)).map(
			(info: { name: string }) => info.name,
		),
	);
	const raw = JSON.parse(readFileSync(path, 'utf8')) as Case[];
	const kept: Case[] = [];
	for (const item of raw) {
		if (kept.length >= 80) break;
		try {
			const ast = parseFormula(item.formula);
			if (!inScope(ast, allowed)) continue;
		} catch {
			continue;
		}
		kept.push({ ...item, source: item.source ?? 'xlsx corpus cached values', kind: 'corpus' });
	}
	process.stdout.write(`${JSON.stringify(kept)}\n`);
}

if (process.argv[2] === '--filter') {
	const path = process.argv[3];
	if (!path) {
		console.error('usage: xlsx_fns_cases.ts --filter corpus.json');
		process.exit(2);
	}
	filterCorpus(path);
} else {
	process.stdout.write(`${JSON.stringify(sweepCases())}\n`);
}
