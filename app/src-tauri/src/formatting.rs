use regex::Regex;

/// Lightweight formatter that mimics the “clean Markdown” step when LLM is unavailable.
pub fn clean_markdown(input: &str) -> String {
    let dehyphenated = dehyphenate(input);
    let smart_quotes_fixed = fix_quotes(&dehyphenated);
    let trimmed = collapse_blank_lines(&smart_quotes_fixed);
    trimmed.trim().to_string()
}

fn dehyphenate(text: &str) -> String {
    let re = Regex::new(r"(?m)(\w+)-\r?\n(\w+)").unwrap();
    re.replace_all(text, "$1$2").to_string()
}

fn fix_quotes(text: &str) -> String {
    text.replace('“', "\"")
        .replace('”', "\"")
        .replace('‘', "'")
        .replace('’', "'")
}

fn collapse_blank_lines(text: &str) -> String {
    let re = Regex::new(r"\n{3,}").unwrap();
    re.replace_all(text, "\n\n").to_string()
}
