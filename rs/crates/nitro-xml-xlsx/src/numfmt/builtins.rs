// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/builtins.ts; changes listed in PROVENANCE.md.

/// Built-in number formats (`numFmtId` values a workbook may use without declaring them), en-US.
/// Ids 23-36 and 50+ are locale specific and omitted.
pub static BUILTIN_NUMBER_FORMATS: &[(u16, &str)] = &[
    (0, "General"),
    (1, "0"),
    (2, "0.00"),
    (3, "#,##0"),
    (4, "#,##0.00"),
    (5, r##""$"#,##0_);\("$"#,##0\)"##),
    (6, r##""$"#,##0_);[Red]\("$"#,##0\)"##),
    (7, r##""$"#,##0.00_);\("$"#,##0.00\)"##),
    (8, r##""$"#,##0.00_);[Red]\("$"#,##0.00\)"##),
    (9, "0%"),
    (10, "0.00%"),
    (11, "0.00E+00"),
    (12, "# ?/?"),
    (13, "# ??/??"),
    (14, "m/d/yyyy"),
    (15, "d-mmm-yy"),
    (16, "d-mmm"),
    (17, "mmm-yy"),
    (18, "h:mm AM/PM"),
    (19, "h:mm:ss AM/PM"),
    (20, "h:mm"),
    (21, "h:mm:ss"),
    (22, "m/d/yyyy h:mm"),
    (37, "#,##0_);(#,##0)"),
    (38, "#,##0_);[Red](#,##0)"),
    (39, "#,##0.00_);(#,##0.00)"),
    (40, "#,##0.00_);[Red](#,##0.00)"),
    (41, r##"_(* #,##0_);_(* \(#,##0\);_(* "-"_);_(@_)"##),
    (
        42,
        r##"_("$"* #,##0_);_("$"* \(#,##0\);_("$"* "-"_);_(@_)"##,
    ),
    (43, r##"_(* #,##0.00_);_(* \(#,##0.00\);_(* "-"??_);_(@_)"##),
    (
        44,
        r##"_("$"* #,##0.00_);_("$"* \(#,##0.00\);_("$"* "-"??_);_(@_)"##,
    ),
    (45, "mm:ss"),
    (46, "[h]:mm:ss"),
    (47, "mm:ss.0"),
    (48, "##0.0E+0"),
    (49, "@"),
];

/// Spellings other producers use for the same built-in formats (ECMA-376 lists these).
const ALIASES: &[(&str, u16)] = &[
    ("general", 0),
    ("mm-dd-yy", 14),
    ("m/d/yy", 14),
    ("m/d/yy h:mm", 22),
    ("h:mm am/pm", 18),
    ("h:mm:ss am/pm", 19),
    ("#,##0 ;(#,##0)", 37),
    ("#,##0 ;[red](#,##0)", 38),
    ("#,##0.00;(#,##0.00)", 39),
    ("#,##0.00;[red](#,##0.00)", 40),
    (r##"#,##0 ;\(#,##0\)"##, 37),
    (r##"#,##0 ;[red]\(#,##0\)"##, 38),
    (r##"#,##0.00;\(#,##0.00\)"##, 39),
    (r##"#,##0.00;[red]\(#,##0.00\)"##, 40),
    ("mmss.0", 47),
];

/// The Home ribbon's number format drop-down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub format: &'static str,
}

pub static FORMAT_PRESETS: &[FormatPreset] = &[
    FormatPreset {
        id: "general",
        label: "General",
        format: "General",
    },
    FormatPreset {
        id: "number",
        label: "Number",
        format: "0.00",
    },
    FormatPreset {
        id: "currency",
        label: "Currency",
        format: r##""$"#,##0.00"##,
    },
    FormatPreset {
        id: "accounting",
        label: "Accounting",
        format: r##"_("$"* #,##0.00_);_("$"* \(#,##0.00\);_("$"* "-"??_);_(@_)"##,
    },
    FormatPreset {
        id: "shortDate",
        label: "Short Date",
        format: "m/d/yyyy",
    },
    FormatPreset {
        id: "longDate",
        label: "Long Date",
        format: "dddd, mmmm d, yyyy",
    },
    FormatPreset {
        id: "time",
        label: "Time",
        format: "h:mm:ss AM/PM",
    },
    FormatPreset {
        id: "percentage",
        label: "Percentage",
        format: "0.00%",
    },
    FormatPreset {
        id: "fraction",
        label: "Fraction",
        format: "# ?/?",
    },
    FormatPreset {
        id: "scientific",
        label: "Scientific",
        format: "0.00E+00",
    },
    FormatPreset {
        id: "text",
        label: "Text",
        format: "@",
    },
];

/// The built-in id of a format code, or `None` when it has to be declared in `numFmts`.
pub fn builtin_format_id(format: &str) -> Option<u16> {
    if let Some((id, _)) = BUILTIN_NUMBER_FORMATS
        .iter()
        .find(|(_, code)| *code == format)
    {
        return Some(*id);
    }
    let lower = format.to_ascii_lowercase();
    ALIASES
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, id)| *id)
}
