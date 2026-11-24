// SPDX-License-Identifier: MIT
// ribbon option, execCommand, nitro-command event (Part of #590).
// Command ids / editing flags must stay in sync with CHROME_COMMANDS in
// rs/crates/nitro-xml-wasm/src/chrome/commands.rs (guard: guard_js_command_ids unit test).

/** Built-in command metadata derived from commands.rs CHROME_COMMANDS and ribbon/commands.js. */
const CHROME_META = [
  // Clipboard
  { id: "home.paste",              label: "Paste",                           shortcut: "Ctrl+V",        editing: true  },
  { id: "home.paste-values",       label: "Paste Values",                    shortcut: null,            editing: true  },
  { id: "home.paste-formulas",     label: "Paste Formulas",                  shortcut: null,            editing: true  },
  { id: "home.paste-formatting",   label: "Paste Formatting",                shortcut: null,            editing: true  },
  { id: "home.paste-transpose",    label: "Transpose",                       shortcut: null,            editing: true  },
  { id: "home.paste-special",      label: "Paste Special...",                shortcut: "Ctrl+Alt+V",    editing: true  },
  { id: "home.cut",                label: "Cut",                             shortcut: "Ctrl+X",        editing: true  },
  { id: "home.copy",               label: "Copy",                            shortcut: "Ctrl+C",        editing: false },
  { id: "home.format-painter",     label: "Format Painter",                  shortcut: null,            editing: true  },
  // Font (xlsx)
  { id: "home.font-name",          label: "Font",                            shortcut: null,            editing: true  },
  { id: "home.font-size",          label: "Font Size",                       shortcut: null,            editing: true  },
  { id: "home.grow-font",          label: "Increase Font Size",              shortcut: "Ctrl+Shift+>",  editing: true  },
  { id: "home.shrink-font",        label: "Decrease Font Size",              shortcut: "Ctrl+Shift+<",  editing: true  },
  { id: "home.bold",               label: "Bold",                            shortcut: "Ctrl+B",        editing: true  },
  { id: "home.italic",             label: "Italic",                          shortcut: "Ctrl+I",        editing: true  },
  { id: "home.underline",          label: "Underline",                       shortcut: "Ctrl+U",        editing: true  },
  { id: "home.underline-double",   label: "Double Underline",                shortcut: null,            editing: true  },
  { id: "home.strikethrough",      label: "Strikethrough",                   shortcut: "Ctrl+5",        editing: true  },
  { id: "home.borders",            label: "Borders",                         shortcut: null,            editing: true  },
  { id: "home.borders-more",       label: "More Borders...",                 shortcut: null,            editing: true  },
  { id: "home.fill-color",         label: "Fill Color",                      shortcut: null,            editing: true  },
  { id: "home.font-color",         label: "Font Color",                      shortcut: null,            editing: true  },
  { id: "home.font-settings",      label: "Font Settings",                   shortcut: null,            editing: true  },
  // Alignment
  { id: "home.align-top",          label: "Top Align",                       shortcut: null,            editing: true  },
  { id: "home.align-middle",       label: "Middle Align",                    shortcut: null,            editing: true  },
  { id: "home.align-bottom",       label: "Bottom Align",                    shortcut: null,            editing: true  },
  { id: "home.orientation",        label: "Orientation",                     shortcut: null,            editing: true  },
  { id: "home.alignment-settings", label: "Format Cell Alignment",           shortcut: null,            editing: true  },
  { id: "home.wrap-text",          label: "Wrap Text",                       shortcut: null,            editing: true  },
  { id: "home.align-left",         label: "Align Left",                      shortcut: null,            editing: true  },
  { id: "home.align-center",       label: "Center",                          shortcut: null,            editing: true  },
  { id: "home.align-right",        label: "Align Right",                     shortcut: null,            editing: true  },
  { id: "home.indent-decrease",    label: "Decrease Indent",                 shortcut: null,            editing: true  },
  { id: "home.indent-increase",    label: "Increase Indent",                 shortcut: null,            editing: true  },
  { id: "home.merge-center",       label: "Merge & Center",                  shortcut: null,            editing: true  },
  { id: "home.merge-across",       label: "Merge Across",                    shortcut: null,            editing: true  },
  { id: "home.merge-cells",        label: "Merge Cells",                     shortcut: null,            editing: true  },
  { id: "home.unmerge",            label: "Unmerge Cells",                   shortcut: null,            editing: true  },
  // Number
  { id: "home.number-format",      label: "Number Format",                   shortcut: null,            editing: true  },
  { id: "home.number-settings",    label: "Number Format Settings",          shortcut: null,            editing: true  },
  { id: "home.accounting",         label: "Accounting Number Format",        shortcut: null,            editing: true  },
  { id: "home.accounting-more",    label: "More Accounting Formats...",      shortcut: null,            editing: true  },
  { id: "home.percent",            label: "Percent Style",                   shortcut: null,            editing: true  },
  { id: "home.comma",              label: "Comma Style",                     shortcut: null,            editing: true  },
  { id: "home.decimal-increase",   label: "Increase Decimal",                shortcut: null,            editing: true  },
  { id: "home.decimal-decrease",   label: "Decrease Decimal",                shortcut: null,            editing: true  },
  // Conditional Formatting
  { id: "home.cf-quick",           label: "Conditional Formatting",          shortcut: null,            editing: true  },
  { id: "home.cf-data-bar",        label: "Data Bars",                       shortcut: null,            editing: true  },
  { id: "home.cf-color-scale",     label: "Color Scales",                    shortcut: null,            editing: true  },
  { id: "home.cf-icon-set",        label: "Icon Sets",                       shortcut: null,            editing: true  },
  { id: "home.cf-new-rule",        label: "New Rule...",                     shortcut: null,            editing: true  },
  { id: "home.cf-clear-selection", label: "Clear Rules from Selected Cells", shortcut: null,            editing: true  },
  { id: "home.cf-clear-sheet",     label: "Clear Rules from Entire Sheet",   shortcut: null,            editing: true  },
  { id: "home.cf-manage",          label: "Manage Rules...",                 shortcut: null,            editing: true  },
  { id: "home.format-as-table",    label: "Format as Table",                 shortcut: null,            editing: true  },
  { id: "home.cell-styles",        label: "Cell Styles",                     shortcut: null,            editing: true  },
  // Cells
  { id: "cells.insert-split",      label: "Insert",                          shortcut: null,            editing: true  },
  { id: "cells.delete-split",      label: "Delete",                          shortcut: null,            editing: true  },
  { id: "cells.insert",            label: "Insert Cells...",                 shortcut: "Ctrl+Shift++",  editing: true  },
  { id: "cells.delete",            label: "Delete Cells...",                 shortcut: "Ctrl+-",        editing: true  },
  { id: "cells.insert-rows",       label: "Insert Sheet Rows",               shortcut: null,            editing: true  },
  { id: "cells.delete-rows",       label: "Delete Sheet Rows",               shortcut: null,            editing: true  },
  { id: "cells.insert-columns",    label: "Insert Sheet Columns",            shortcut: null,            editing: true  },
  { id: "cells.delete-columns",    label: "Delete Sheet Columns",            shortcut: null,            editing: true  },
  { id: "sheet.insert",            label: "Insert Sheet",                    shortcut: null,            editing: true  },
  { id: "sheet.delete",            label: "Delete Sheet",                    shortcut: null,            editing: true  },
  // Format
  { id: "format.row-height",       label: "Row Height...",                   shortcut: null,            editing: true  },
  { id: "format.autofit-rows",     label: "AutoFit Row Height",              shortcut: null,            editing: true  },
  { id: "format.column-width",     label: "Column Width...",                 shortcut: null,            editing: true  },
  { id: "format.autofit-columns",  label: "AutoFit Column Width",            shortcut: null,            editing: true  },
  { id: "format.default-width",    label: "Default Width...",                shortcut: null,            editing: true  },
  { id: "format.hide-rows",        label: "Hide Rows",                       shortcut: null,            editing: true  },
  { id: "format.hide-columns",     label: "Hide Columns",                    shortcut: null,            editing: true  },
  { id: "sheet.hide",              label: "Hide Sheet",                      shortcut: null,            editing: true  },
  { id: "format.unhide-rows",      label: "Unhide Rows",                     shortcut: null,            editing: true  },
  { id: "format.unhide-columns",   label: "Unhide Columns",                  shortcut: null,            editing: true  },
  { id: "sheet.unhide",            label: "Unhide Sheet...",                 shortcut: null,            editing: true  },
  { id: "sheet.rename",            label: "Rename Sheet",                    shortcut: null,            editing: true  },
  { id: "sheet.move-copy",         label: "Move or Copy Sheet...",           shortcut: null,            editing: true  },
  { id: "sheet.tab-color",         label: "Tab Color",                       shortcut: null,            editing: true  },
  { id: "review.protect-sheet",    label: "Protect Sheet",                   shortcut: null,            editing: true  },
  { id: "format.lock-cell",        label: "Lock Cell",                       shortcut: null,            editing: true  },
  { id: "format.cells",            label: "Format Cells...",                 shortcut: "Ctrl+1",        editing: true  },
  // Editing
  { id: "home.autosum",            label: "AutoSum",                         shortcut: "Alt+=",         editing: true  },
  { id: "home.autosum-average",    label: "Average",                         shortcut: null,            editing: true  },
  { id: "home.autosum-count",      label: "Count Numbers",                   shortcut: null,            editing: true  },
  { id: "home.autosum-max",        label: "Max",                             shortcut: null,            editing: true  },
  { id: "home.autosum-min",        label: "Min",                             shortcut: null,            editing: true  },
  { id: "home.autosum-more",       label: "More Functions...",               shortcut: null,            editing: true  },
  { id: "home.fill-down",          label: "Down",                            shortcut: "Ctrl+D",        editing: true  },
  { id: "home.fill-right",         label: "Right",                           shortcut: "Ctrl+R",        editing: true  },
  { id: "home.fill-up",            label: "Up",                              shortcut: null,            editing: true  },
  { id: "home.fill-left",          label: "Left",                            shortcut: null,            editing: true  },
  { id: "home.fill-series",        label: "Series...",                       shortcut: null,            editing: true  },
  { id: "home.clear-all",          label: "Clear All",                       shortcut: null,            editing: true  },
  { id: "home.clear-formats",      label: "Clear Formats",                   shortcut: null,            editing: true  },
  { id: "home.clear-contents",     label: "Clear Contents",                  shortcut: null,            editing: true  },
  { id: "home.clear-comments",     label: "Clear Comments and Notes",        shortcut: null,            editing: true  },
  { id: "home.clear-hyperlinks",   label: "Clear Hyperlinks",                shortcut: null,            editing: true  },
  // Sort/Filter/Find
  { id: "data.sort-asc",           label: "Sort A to Z",                     shortcut: null,            editing: true  },
  { id: "data.sort-desc",          label: "Sort Z to A",                     shortcut: null,            editing: true  },
  { id: "data.sort-custom",        label: "Custom Sort...",                  shortcut: null,            editing: true  },
  { id: "data.filter",             label: "Filter",                          shortcut: "Ctrl+Shift+L",  editing: true  },
  { id: "data.filter-clear",       label: "Clear",                           shortcut: null,            editing: true  },
  { id: "data.filter-reapply",     label: "Reapply",                         shortcut: null,            editing: true  },
  { id: "home.find",               label: "Find...",                         shortcut: "Ctrl+F",        editing: false },
  { id: "home.replace",            label: "Replace...",                      shortcut: "Ctrl+H",        editing: true  },
  { id: "home.go-to",              label: "Go To...",                        shortcut: "Ctrl+G",        editing: false },
  { id: "home.go-to-special",      label: "Go To Special...",                shortcut: null,            editing: false },
  { id: "home.select-formulas",    label: "Formulas",                        shortcut: null,            editing: false },
  { id: "home.select-comments",    label: "Notes",                           shortcut: null,            editing: false },
  { id: "home.select-conditional", label: "Conditional Formatting",          shortcut: null,            editing: false },
  { id: "home.select-constants",   label: "Constants",                       shortcut: null,            editing: false },
  { id: "home.select-validation",  label: "Data Validation",                 shortcut: null,            editing: false },
  // File / Edit shortcuts
  { id: "file.save",               label: "Save",                            shortcut: "Ctrl+S",        editing: true  },
  { id: "file.open",               label: "Open",                            shortcut: "Ctrl+O",        editing: true  },
  { id: "file.new",                label: "New workbook",                    shortcut: "Ctrl+N",        editing: true  },
  { id: "file.print",              label: "Print",                           shortcut: "Ctrl+P",        editing: true  },
  { id: "edit.undo",               label: "Undo",                            shortcut: "Ctrl+Z",        editing: true  },
  { id: "edit.redo",               label: "Redo",                            shortcut: "Ctrl+Y",        editing: true  },
  // Number formats
  { id: "home.format-general",     label: "General number format",           shortcut: null,            editing: true  },
  { id: "home.format-currency",    label: "Currency format",                 shortcut: null,            editing: true  },
  { id: "home.format-percent",     label: "Percentage format",               shortcut: null,            editing: true  },
  { id: "home.format-scientific",  label: "Scientific format",               shortcut: null,            editing: true  },
  { id: "home.format-date",        label: "Date format",                     shortcut: null,            editing: true  },
  { id: "home.format-time",        label: "Time format",                     shortcut: null,            editing: true  },
  { id: "home.format-number",      label: "Number format",                   shortcut: null,            editing: true  },
  { id: "edit.insert-date",        label: "Insert the current date",         shortcut: null,            editing: true  },
  { id: "edit.insert-time",        label: "Insert the current time",         shortcut: null,            editing: true  },
  { id: "formulas.show-formulas",  label: "Show Formulas",                   shortcut: null,            editing: true  },
  { id: "edit.select-column",      label: "Select the entire column",        shortcut: null,            editing: true  },
  { id: "edit.select-row",         label: "Select the entire row",           shortcut: null,            editing: true  },
  { id: "sheet.previous",          label: "Previous sheet",                  shortcut: "Ctrl+PageUp",   editing: false },
  { id: "sheet.next",              label: "Next sheet",                      shortcut: "Ctrl+PageDown", editing: false },
  { id: "insert.table",            label: "Create a table",                  shortcut: null,            editing: true  },
  { id: "insert.link",             label: "Insert link",                     shortcut: "Ctrl+K",        editing: true  },
  { id: "review.new-comment",      label: "New comment",                     shortcut: null,            editing: true  },
  { id: "formulas.calculate-now",  label: "Calculate Now",                   shortcut: null,            editing: true  },
  // Pptx/docx office commands (from ribbon/commands.js — editor ops)
  { id: "home.shape-fill",         label: "Shape fill",                      shortcut: null,            editing: true  },
  { id: "home.shape-outline",      label: "Shape outline",                   shortcut: null,            editing: true  },
  { id: "home.shape-dash",         label: "Dashed shape outline",            shortcut: null,            editing: true  },
  { id: "home.duplicate",          label: "Duplicate shape",                 shortcut: null,            editing: true  },
  { id: "home.delete",             label: "Delete shape",                    shortcut: null,            editing: true  },
  { id: "home.group",              label: "Group shapes",                    shortcut: null,            editing: true  },
  { id: "home.ungroup",            label: "Ungroup shapes",                  shortcut: null,            editing: true  },
  { id: "home.bring-forward",      label: "Bring Forward",                   shortcut: null,            editing: true  },
  { id: "home.send-backward",      label: "Send Backward",                   shortcut: null,            editing: true  },
  { id: "home.bring-to-front",     label: "Bring to Front",                  shortcut: null,            editing: true  },
  { id: "home.send-to-back",       label: "Send to Back",                    shortcut: null,            editing: true  },
  { id: "home.subscript",          label: "Subscript",                       shortcut: null,            editing: true  },
  { id: "home.superscript",        label: "Superscript",                     shortcut: null,            editing: true  },
  { id: "home.highlight",          label: "Text highlight",                  shortcut: null,            editing: true  },
  { id: "home.double-underline",   label: "Double underline",                shortcut: null,            editing: true  },
  { id: "home.double-strike",      label: "Double strikethrough",            shortcut: null,            editing: true  },
  { id: "home.caps",               label: "All caps",                        shortcut: null,            editing: true  },
  { id: "home.small-caps",         label: "Small caps",                      shortcut: null,            editing: true  },
  { id: "home.hidden",             label: "Hidden text",                     shortcut: null,            editing: true  },
  { id: "home.expand-spacing",     label: "Expand character spacing",        shortcut: null,            editing: true  },
  { id: "home.condense-spacing",   label: "Condense character spacing",      shortcut: null,            editing: true  },
  { id: "home.scale-up",           label: "Increase character scale",        shortcut: null,            editing: true  },
  { id: "home.scale-down",         label: "Decrease character scale",        shortcut: null,            editing: true  },
  { id: "home.bullets",            label: "Bulleted list",                   shortcut: null,            editing: true  },
  { id: "home.numbering",          label: "Numbered list",                   shortcut: null,            editing: true  },
  { id: "home.outdent",            label: "Decrease indent",                 shortcut: null,            editing: true  },
  { id: "home.indent",             label: "Increase indent",                 shortcut: null,            editing: true  },
  { id: "home.align-justify",      label: "Justify",                         shortcut: null,            editing: true  },
  { id: "home.line-spacing",       label: "Line spacing",                    shortcut: null,            editing: true  },
  { id: "home.space-before",       label: "Space before paragraph",          shortcut: null,            editing: true  },
  { id: "home.space-after",        label: "Space after paragraph",           shortcut: null,            editing: true  },
  { id: "home.hanging-indent",     label: "Hanging indent",                  shortcut: null,            editing: true  },
  { id: "home.first-line",         label: "First line indent",               shortcut: null,            editing: true  },
  { id: "home.indent-right",       label: "Increase right indent",           shortcut: null,            editing: true  },
  { id: "home.outdent-right",      label: "Decrease right indent",           shortcut: null,            editing: true  },
  { id: "home.shading",            label: "Shade paragraph",                 shortcut: null,            editing: true  },
  { id: "home.border-bottom",      label: "Bottom border",                   shortcut: null,            editing: true  },
  { id: "home.border-top",         label: "Top border",                      shortcut: null,            editing: true  },
  { id: "home.border-left",        label: "Left border",                     shortcut: null,            editing: true  },
  { id: "home.border-right",       label: "Right border",                    shortcut: null,            editing: true  },
  { id: "home.border-double",      label: "Double border",                   shortcut: null,            editing: true  },
  { id: "home.border-dash",        label: "Dashed border",                   shortcut: null,            editing: true  },
  { id: "home.border-dot",         label: "Dotted border",                   shortcut: null,            editing: true  },
  { id: "home.style-normal",       label: "Normal",                          shortcut: null,            editing: true  },
  { id: "home.style-title",        label: "Title",                           shortcut: null,            editing: true  },
  { id: "home.style-heading-1",    label: "Heading 1",                       shortcut: null,            editing: true  },
  { id: "home.style-heading-2",    label: "Heading 2",                       shortcut: null,            editing: true  },
  { id: "home.style-heading-3",    label: "Heading 3",                       shortcut: null,            editing: true  },
  { id: "insert.slide",            label: "New Slide",                       shortcut: null,            editing: true  },
  { id: "insert.duplicate-slide",  label: "Duplicate Slide",                 shortcut: null,            editing: true  },
  { id: "home.layout",             label: "Change slide layout",             shortcut: null,            editing: true  },
  { id: "home.delete-slide",       label: "Delete Slide",                    shortcut: null,            editing: true  },
  { id: "insert.move-slide-up",    label: "Move Slide Up",                   shortcut: null,            editing: true  },
  { id: "insert.move-slide-down",  label: "Move Slide Down",                 shortcut: null,            editing: true  },
  { id: "table.insert-row",        label: "Insert Row",                      shortcut: null,            editing: true  },
  { id: "table.delete-row",        label: "Delete Row",                      shortcut: null,            editing: true  },
  { id: "table.insert-column",     label: "Insert Column",                   shortcut: null,            editing: true  },
  { id: "table.delete-column",     label: "Delete Column",                   shortcut: null,            editing: true  },
  { id: "table.cell-text",         label: "Set cell text",                   shortcut: null,            editing: true  },
  { id: "table.delete",            label: "Delete table",                    shortcut: null,            editing: true  },
  { id: "insert.picture",          label: "Insert a picture",                shortcut: null,            editing: true  },
  { id: "insert.shape",            label: "Insert a shape",                  shortcut: null,            editing: true  },
  { id: "insert.text-box",         label: "Text Box",                        shortcut: null,            editing: true  },
  { id: "insert.page-break",       label: "Insert a page break",             shortcut: null,            editing: true  },
  { id: "home.landscape",          label: "Toggle landscape orientation",    shortcut: null,            editing: true  },
  { id: "home.narrow-margins",     label: "Narrow margins",                  shortcut: null,            editing: true  },
  { id: "home.moderate-margins",   label: "Moderate margins",                shortcut: null,            editing: true  },
  { id: "home.wide-margins",       label: "Wide margins",                    shortcut: null,            editing: true  },
  { id: "home.page-a4",            label: "A4 page size",                    shortcut: null,            editing: true  },
  { id: "home.page-legal",         label: "Legal page size",                 shortcut: null,            editing: true  },
  { id: "home.page-break-before",  label: "Page break before",               shortcut: null,            editing: true  },
];

/** Fast id → meta lookup for validation and commands list. */
const META_MAP = new Map(CHROME_META.map((m) => [m.id, m]));

/** Exported for unit testing (guard_js_command_ids). */
export const COMMAND_IDS = META_MAP;

function formatOf(el) {
  const n = el.localName;
  return n === "nitro-xlsx" ? "xlsx" : n === "nitro-docx" ? "docx" : n === "nitro-pptx" ? "pptx" : "";
}

function isReadOnly(el) {
  return el._readonly !== false;
}

/** Build an SVG from a path data string (no HTML injection; strict-CSP safe). */
function iconSvg(pathData) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  // Only the path `d` attribute is accepted; no HTML or script injection possible.
  path.setAttribute("d", String(pathData));
  svg.appendChild(path);
  return svg;
}

/** Remove a command control (button + parent split/stack if it empties) from the chrome element. */
function removeControl(ch, id) {
  const esc = id.replace(/([.+*?[\]{}()|^$\\])/g, "\\$1");
  for (const btn of ch.querySelectorAll(`[data-g="${esc}"]`)) {
    const split = btn.closest(".ribbon-split, .ribbon-split-large");
    if (split) { split.remove(); continue; }
    const stack = btn.closest(".ribbon-stack");
    btn.remove();
    if (stack && !stack.querySelector("button, input, select")) stack.remove();
  }
}

/** Inject host-added command buttons at the end of the ribbon panel (Rust chrome). */
function injectChromeButtons(ch, addList, ro) {
  if (!addList || !addList.length) return;
  const panel = ch.querySelector(".ribbon-panel");
  if (!panel) return;
  let hostGroup = panel.querySelector("[data-host-grp]");
  if (!hostGroup) {
    hostGroup = document.createElement("div");
    hostGroup.className = "ribbon-group";
    hostGroup.dataset.hostGrp = "";
    hostGroup.setAttribute("role", "group");
    hostGroup.setAttribute("aria-label", "Actions");
    panel.appendChild(hostGroup);
  }
  for (const cmd of addList) {
    const eid = String(cmd.id).replace(/([.+*?[\]{}()|^$\\])/g, "\\$1");
    if (hostGroup.querySelector(`[data-g="${eid}"]`)) continue;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "ribbon-small ribbon-labelled";
    btn.setAttribute("data-a", "cmd");
    btn.setAttribute("data-g", String(cmd.id));
    btn.setAttribute("aria-label", String(cmd.label));
    if (cmd.icon) btn.appendChild(iconSvg(cmd.icon));
    const span = document.createElement("span");
    span.textContent = String(cmd.label);
    btn.appendChild(span);
    if (ro && cmd.editing !== false) btn.disabled = true;
    hostGroup.appendChild(btn);
  }
}

/** Re-apply ribbon config to the Rust chrome element (called by MutationObserver). */
function applyRibbonToChrome(ch, cfg, el) {
  if (!cfg) return;
  for (const id of cfg.hide) removeControl(ch, id);
  injectChromeButtons(ch, cfg.add, isReadOnly(el));
}

/** Build the commands array for el.commands. */
function buildCommands(el) {
  const ro = isReadOnly(el);
  const hostAdded = (el._ribbonCfg ? el._ribbonCfg.add : []).map((a) => ({
    id: a.id,
    label: a.label,
    shortcut: null,
    enabled: !(ro && a.editing !== false),
  }));
  const builtin = CHROME_META.map((m) => ({
    id: m.id,
    label: m.label,
    shortcut: m.shortcut || null,
    enabled: !m.editing || !ro,
  }));
  return [...builtin, ...hostAdded];
}

/**
 * Attach the ribbon API to a ShellViewerElement:
 *   el.ribbon = { hide, add }
 *   el.execCommand(id) → boolean
 *   el.commands → array
 *   fires cancelable `nitro-command` CustomEvent before every command dispatch
 */
export function attachRibbon(el) {
  // ---- nitro-command event wrapper ----
  // Wraps shellAction to intercept all "cmd" dispatches (ribbon clicks, shortcuts, execCommand).
  const origShellAction = el.shellAction.bind(el);
  el.shellAction = async function shellActionWithRibbonHook(op, arg) {
    if (op === "cmd") {
      const id = arg;
      const ev = new CustomEvent("nitro-command", {
        cancelable: true,
        bubbles: true,
        composed: true,
        detail: { id, format: formatOf(el) },
      });
      if (!el.dispatchEvent(ev)) return {};
      // Host-added commands fire nitro-command only; Rust has no handler for them.
      const cfg = el._ribbonCfg;
      if (cfg && cfg.add.some((a) => a.id === id)) return {};
    }
    return origShellAction(op, arg);
  };

  // ---- MutationObserver on _ch to re-apply hide/add after every chrome update ----
  const ch = el._ch;
  if (ch) {
    let busy = false;
    const mo = new MutationObserver(() => {
      if (busy || !el._ribbonCfg) return;
      busy = true;
      applyRibbonToChrome(ch, el._ribbonCfg, el);
      busy = false;
    });
    mo.observe(ch, { childList: true });
    el._ribbonMo = mo;
  }

  // ---- ribbon property ----
  // Pick up any pre-load assignment (host may set el.ribbon before loadBuffer).
  const pending = Object.prototype.hasOwnProperty.call(el, "ribbon") ? el.ribbon : undefined;
  delete el.ribbon;
  el._ribbonCfg = null;
  Object.defineProperty(el, "ribbon", {
    configurable: true,
    get() { return this._ribbonCfg; },
    set(v) {
      if (!v) { this._ribbonCfg = null; return; }
      this._ribbonCfg = {
        hide: new Set(Array.isArray(v.hide) ? v.hide.map(String) : []),
        add: Array.isArray(v.add) ? v.add : [],
      };
      // Apply immediately if chrome is already populated.
      if (el._ch && el._ch.firstChild) applyRibbonToChrome(el._ch, this._ribbonCfg, el);
      // Also inject into the embedded toolbar if it's already mounted.
      if (el._emb && this._ribbonCfg.add.length) {
        import("./embedded/index.js").then((m) => m.addEmbeddedHostBtns(el, this._ribbonCfg.add));
      }
    },
  });
  if (pending !== undefined) el.ribbon = pending;

  // ---- el.commands ----
  Object.defineProperty(el, "commands", {
    configurable: true,
    get() { return buildCommands(el); },
  });

  // ---- el.execCommand(id, args?) ----
  el.execCommand = function execCommand(id) {
    const cfg = el._ribbonCfg;
    const isHostAdded = cfg && cfg.add.some((a) => a.id === id);
    if (!META_MAP.has(id) && !isHostAdded) {
      const valid = [...META_MAP.keys(), ...(cfg ? cfg.add.map((a) => a.id) : [])];
      console.error(
        `nitro-xml: execCommand: unknown id "${id}". Valid ids:\n  ${valid.join(", ")}`,
      );
      return false;
    }
    el.shellAction("cmd", id);
    return true;
  };
}
