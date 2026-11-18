// SPDX-License-Identifier: MIT
use nitro_xml_docx::{edit::host::DocxHost, save_docx, DocxEditor};
use serde_json::json;
const NOTES: &[u8] = include_bytes!("../../../../demo/samples/notes.docx");

#[test]
fn replace_body_and_table_save_undo() {
    // Catches: replacement flattening italic body text, skipping tables, or bypassing undo/save.
    let mut editor = DocxEditor::from_host(DocxHost::open(NOTES).unwrap());
    editor.set_read_only(false);
    let original = editor.session.host.blocks.clone();
    editor
        .dispatch_command(
            "search.replaceAll",
            &json!({"query":"Q3", "replacement":"Q4"}),
        )
        .unwrap();
    assert_eq!(editor.commands.search.last_replaced, 2);
    let saved = save_docx(&editor.session.host, NOTES).unwrap();
    let reopened = DocxHost::open(&saved).unwrap();
    assert!(!serde_json::to_string(&reopened.blocks)
        .unwrap()
        .contains("Q3"));
    assert!(reopened.blocks[1]["runs"][0]["italic"].as_bool().unwrap());
    editor.dispatch_command("edit.undo", &json!({})).unwrap();
    assert_eq!(editor.session.host.blocks, original);
    assert_eq!(save_docx(&editor.session.host, NOTES).unwrap(), NOTES);
    editor.dispatch_command("edit.redo", &json!({})).unwrap();
    assert_eq!(save_docx(&editor.session.host, NOTES).unwrap(), saved);
}

#[test]
fn unicode_and_mixed_runs() {
    // Catches: folded UTF-8 offsets panicking, overlapping matches, or formatting leaking across runs.
    let mut host = DocxHost::open(NOTES).unwrap();
    host.blocks = vec![json!({"type":"paragraph","id":"p0","runs":[
        {"text":"İ é ","italic":true}, {"text":"fo","bold":true}, {"text":"o foo food","color":"123456"}
    ]})];
    let mut editor = DocxEditor::from_host(host);
    editor.set_read_only(false);
    editor
        .dispatch_command(
            "search.replaceAll",
            &json!({"query":"foo","replacement":"X","wholeWord":true}),
        )
        .unwrap();
    assert_eq!(editor.commands.search.last_replaced, 2);
    let runs = editor.session.host.blocks[0]["runs"].as_array().unwrap();
    assert_eq!(runs[0]["text"], "İ é ");
    assert_eq!(runs[1]["text"], "X");
    assert_eq!(runs[1]["bold"], true);
    assert_eq!(runs[2]["text"], " X food");
    assert_eq!(runs[2]["color"], "123456");
    editor
        .dispatch_command("search.replaceAll", &json!({"query":"é","replacement":"E"}))
        .unwrap();
    assert_eq!(editor.commands.search.last_replaced, 1);
}
