use crate::completion_engine::requires_semantic_verification;

#[test]
fn plain_write_directive_naming_report_py_is_not_semantic() {
    let objective =
        "Use the write_file tool to create report.py that prints the value of n read from data.json.";
    assert!(!requires_semantic_verification(objective));
}

#[test]
fn bare_report_word_is_still_semantic() {
    assert!(requires_semantic_verification("Analyze the report and explain the findings"));
}

#[test]
fn semantic_verb_next_to_a_file_name_still_counts() {
    assert!(requires_semantic_verification("Summarize the contents of notes.md"));
}

#[test]
fn backticked_file_token_with_trailing_punctuation_is_stripped() {
    assert!(!requires_semantic_verification("Create `report.py`, then stop."));
}
