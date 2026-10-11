// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xml/namespaces.ts; changes listed in PROVENANCE.md.
//! Namespace URI constants used across OOXML part families.

/// Namespace URIs used across the OOXML part families.
pub mod ns {
    /// WordprocessingML main.
    pub const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    /// PresentationML main.
    pub const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
    /// SpreadsheetML main.
    pub const X: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    /// DrawingML main.
    pub const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    /// DrawingML chart.
    pub const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
    /// Office 2010 chart extensions.
    pub const C14: &str = "http://schemas.microsoft.com/office/drawing/2007/8/2/chart";
    /// Office 2014 chart extensions (`c16:`).
    pub const C16: &str = "http://schemas.microsoft.com/office/drawing/2014/chart";
    /// Office 2015 chart extensions (`c16r2:`).
    pub const C16R2: &str = "http://schemas.microsoft.com/office/drawing/2015/06/chart";
    /// DrawingML diagram (SmartArt) main.
    pub const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
    /// Office 2007+ diagram drawing (`dsp`).
    pub const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";
    /// DrawingML Word drawing (`wp:inline`, `wp:anchor`).
    pub const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
    /// DrawingML picture.
    pub const PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
    /// Word 2010 shapes (`wps`).
    pub const WPS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";
    /// Relationships (`r:id`, `r:embed`).
    pub const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    /// Markup compatibility (`mc:AlternateContent`).
    pub const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
    /// OPC relationships part.
    pub const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    /// OPC content types part.
    pub const CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
    /// Office Math.
    pub const M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
    /// Core properties.
    pub const CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
    /// Dublin Core elements.
    pub const DC: &str = "http://purl.org/dc/elements/1.1/";
    /// Legacy VML.
    pub const V: &str = "urn:schemas-microsoft-com:vml";
    /// Office VML extensions.
    pub const O: &str = "urn:schemas-microsoft-com:office:office";
    /// Word VML extensions.
    pub const W10: &str = "urn:schemas-microsoft-com:office:word";
    /// SpreadsheetML drawing (`xdr:wsDr`, cell anchors).
    pub const XDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
    /// SpreadsheetML 2009 extensions.
    pub const X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
    /// SpreadsheetML 2009 attribute extensions.
    pub const X14AC: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac";
    /// SpreadsheetML revision ids.
    pub const XR: &str = "http://schemas.microsoft.com/office/spreadsheetml/2014/revision";
    /// Excel threaded comments and persons.
    pub const TC: &str = "http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments";
    /// Legacy VML Excel client data.
    pub const XV: &str = "urn:schemas-microsoft-com:office:excel";
    /// Extended (app) properties.
    pub const EXTENDED_PROPERTIES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";
    /// Dublin Core terms.
    pub const DCTERMS: &str = "http://purl.org/dc/terms/";
    /// Custom document properties.
    pub const CUSTOM_PROPERTIES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties";
    /// Document property variant types.
    pub const VT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes";
    /// XML itself (`xml:space`).
    pub const XML: &str = "http://www.w3.org/XML/1998/namespace";
}
