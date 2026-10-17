use omarchy_t3_client::prompt_recall::recall;
#[test]
fn native_sent_context_recall_preserves_typed_text_and_removes_only_trailing_context() {
    assert_eq!(recall("Ultrathink:\nInvestigate this\n\n<terminal_context>\n- Terminal 1 lines 12-13:\n  12 | git status\n</terminal_context>\n\n<element_context>\n- <button>:\n  url: https://example.com\n</element_context>"),"Investigate this");
    let block = "<review_comment filePath=\"src/app.ts\">Keep this configurable.</review_comment>";
    assert_eq!(
        recall(&format!("Please update this.\n\n{block}")),
        "Please update this."
    );
    let typed = format!("Before\n\n{block}\n\nAfter");
    assert_eq!(recall(&format!("{typed}\n\n{block}")), typed);
    let malformed="Prompt\n<preview_annotation>\nouter literal\n<preview_annotation>\ninner\n</preview_annotation>";
    assert_eq!(recall(malformed), malformed);
    assert_eq!(recall("Fix this\n\n<preview_annotation>\nPage: Example\n<element_context>\n- <button>:\n  html: Save\n</element_context>\n</preview_annotation>"),"Fix this");
    assert_eq!(
        recall("Ordinary\n<terminal_context>\nnot a generated header\n</terminal_context>"),
        "Ordinary\n<terminal_context>\nnot a generated header\n</terminal_context>"
    );
}
#[test]
fn native_terminal_label_boundaries_and_canonical_references_are_exact() {
    for (typed, expected) in [
        ("Look at @terminal-1:4 please", "Look at please"),
        (
            "@terminal-1:4 typed twice: @terminal-1:4\n    indented  code",
            "typed twice: @terminal-1:4\n    indented  code",
        ),
        (
            "see @terminal-1:40 and @terminal-1:4-12 then @terminal-1:4",
            "see @terminal-1:40 and @terminal-1:4-12 then",
        ),
        ("email@terminal-1:4", "email@terminal-1:4"),
    ] {
        assert_eq!(recall(&format!("{typed}\n\n<terminal_context>\n- Terminal 1 line 4:\n  4 | ls\n</terminal_context>")),expected)
    }
    assert_eq!(recall("Look at [Terminal](t3-context://v1/terminal/term-1) please\n    indented  code\n![image](t3-context://v1/image/img-1)"),"Look at please\n    indented  code");
    assert_eq!(
        recall("Keep [literal](t3-context://v1/invalid?/id) and [external](https://example.com)"),
        "Keep [literal](t3-context://v1/invalid?/id) and [external](https://example.com)"
    );
    for message in ["   ","[User attached one or more files without additional text. Respond using the conversation context and the attached files.]","PLEASE IMPLEMENT THIS PLAN:\n# Plan\nSteps"]{assert_eq!(recall(message),"")}
}
