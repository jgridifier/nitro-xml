// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/default-styles.ts; changes listed in PROVENANCE.md.

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn heading(id: &str, name: &str, level: u32, before: u32, size: u32, color: &str) -> String {
    format!(
        r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="{before}" w:after="0"/><w:outlineLvl w:val="{level}"/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light" w:eastAsia="Calibri Light" w:cs="Times New Roman"/><w:color w:val="{color}"/><w:sz w:val="{size}"/><w:szCs w:val="{size}"/></w:rPr></w:style>"#
    )
}

fn toc(level: u32) -> String {
    let ind = if level > 1 {
        format!(r#"<w:ind w:left="{}"/>"#, (level - 1) * 220)
    } else {
        String::new()
    };
    format!(
        r#"<w:style w:type="paragraph" w:styleId="TOC{level}"><w:name w:val="toc {level}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="39"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="100"/>{ind}</w:pPr></w:style>"#
    )
}

fn note_styles(id: &str, name: &str) -> String {
    format!(
        r#"<w:style w:type="paragraph" w:styleId="{id}Text"><w:name w:val="{name} text"/><w:basedOn w:val="Normal"/><w:link w:val="{id}TextChar"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style><w:style w:type="character" w:styleId="{id}TextChar"><w:name w:val="{name} Text Char"/><w:basedOn w:val="DefaultParagraphFont"/><w:link w:val="{id}Text"/><w:uiPriority w:val="99"/><w:semiHidden/><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style><w:style w:type="character" w:styleId="{id}Reference"><w:name w:val="{name} reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>"#
    )
}

/// Paragraph style element for the gallery ids the editor can apply.
/// `None` when `style_id` is not one of Title / Heading1 / Heading2 / Heading3.
pub fn builtin_paragraph_style_xml(style_id: &str) -> Option<String> {
    Some(match style_id {
        "Title" => r#"<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="10"/><w:qFormat/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/><w:contextualSpacing/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light" w:eastAsia="Calibri Light" w:cs="Times New Roman"/><w:spacing w:val="-10"/><w:kern w:val="28"/><w:sz w:val="56"/><w:szCs w:val="56"/></w:rPr></w:style>"#.to_owned(),
        "Heading1" => heading("Heading1", "heading 1", 0, 240, 32, "2F5496"),
        "Heading2" => heading("Heading2", "heading 2", 1, 40, 26, "2F5496"),
        "Heading3" => heading("Heading3", "heading 3", 2, 40, 24, "1F3763"),
        _ => return None,
    })
}

/// Built-in `word/styles.xml` for documents without a styles part (matches upstream `DEFAULT_STYLES_XML`).
pub fn default_styles_xml() -> String {
    let mut s = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:eastAsia="Calibri" w:hAnsi="Calibri" w:cs="Times New Roman"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="en-US" w:eastAsia="en-US" w:bidi="ar-SA"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="numbering" w:default="1" w:styleId="NoList"><w:name w:val="No List"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="10"/><w:qFormat/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/><w:contextualSpacing/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light" w:eastAsia="Calibri Light" w:cs="Times New Roman"/><w:spacing w:val="-10"/><w:kern w:val="28"/><w:sz w:val="56"/><w:szCs w:val="56"/></w:rPr></w:style>"#
    );
    s.push_str(&heading("Heading1", "heading 1", 0, 240, 32, "2F5496"));
    s.push_str(&heading("Heading2", "heading 2", 1, 40, 26, "2F5496"));
    s.push_str(&heading("Heading3", "heading 3", 2, 40, 24, "1F3763"));
    s.push_str(&toc(1));
    s.push_str(&toc(2));
    s.push_str(&toc(3));
    s.push_str(&note_styles("Footnote", "footnote"));
    s.push_str(&note_styles("Endnote", "endnote"));
    s.push_str(
        r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style></w:styles>"#,
    );
    s
}
