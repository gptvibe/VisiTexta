use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
pub struct StudyOptions<'a> {
    pub study_boost: bool,
    pub custom_override: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct PageLine {
    text: String,
    page_number: usize,
}

#[derive(Debug, Clone)]
struct PageParagraph {
    text: String,
    page_number: usize,
}

#[derive(Debug, Clone)]
struct NoteItem {
    text: String,
    source_pages: Vec<usize>,
}

#[derive(Debug, Clone)]
struct GlossaryItem {
    term: String,
    definition: String,
    source_pages: Vec<usize>,
}

pub fn build_study_notes(page_texts: &[String], options: StudyOptions<'_>) -> String {
    let lines = collect_lines(page_texts);
    let paragraphs = collect_paragraphs(page_texts);

    if paragraphs.is_empty() && lines.is_empty() {
        return String::new();
    }

    let (title, title_pages) = pick_title(&lines)
        .map(|line| (line.text.clone(), vec![line.page_number]))
        .unwrap_or_else(|| {
            let fallback_pages = page_texts
                .first()
                .map(|_| vec![1])
                .unwrap_or_default();
            ("Study Notes".to_string(), fallback_pages)
        });
    let headings = collect_headings(&lines, &title);
    let key_points = collect_key_points(&paragraphs);
    let glossary = collect_glossary(&lines, &paragraphs);
    let formulas = collect_formulas(&lines);
    let examples = collect_examples(&paragraphs);
    let review_questions =
        collect_review_questions(&title, &title_pages, &headings, &glossary, &key_points);

    let mut sections = vec![format!("# {title}")];
    if let Some(source_links) = format_source_suffix(&title_pages) {
        sections.push(source_links);
    }

    if let Some(override_note) = options
        .custom_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!(
            "_Advanced override: {}_",
            override_note.replace('\n', " ")
        ));
    }

    if !headings.is_empty() {
        sections.push("## Headings".into());
        sections.extend(headings.iter().map(format_note_item));
    }

    if !key_points.is_empty() {
        sections.push("## Key Points".into());
        sections.extend(key_points.iter().map(format_note_item));
    }

    if !glossary.is_empty() {
        sections.push("## Glossary".into());
        sections.extend(glossary.iter().map(format_glossary_item));
    }

    if !formulas.is_empty() {
        sections.push("## Formulas".into());
        sections.extend(formulas.iter().map(|formula| {
            format_note_item(&NoteItem {
                text: format!("`{}`", formula.text),
                source_pages: formula.source_pages.clone(),
            })
        }));
    }

    if !examples.is_empty() {
        sections.push("## Examples".into());
        sections.extend(examples.iter().map(format_note_item));
    }

    sections.push("## Review Questions".into());
    sections.extend(review_questions.iter().map(format_note_item));

    if options.study_boost {
        let memory_checks = collect_memory_checks(&glossary, &formulas, &headings, &key_points);
        if !memory_checks.is_empty() {
            sections.push("## Study Boost".into());
            sections.extend(memory_checks.iter().map(format_note_item));
        }
    }

    sections.join("\n\n").trim().to_string()
}

fn page_content(page_markdown: &str) -> String {
    page_markdown
        .lines()
        .filter(|line| !line.trim_start().starts_with("<!--"))
        .filter(|line| {
            let trimmed = line.trim();
            !(trimmed.starts_with("## Page ")
                && trimmed["## Page ".len()..]
                    .chars()
                    .all(|ch| ch.is_ascii_digit()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_lines(page_texts: &[String]) -> Vec<PageLine> {
    page_texts
        .iter()
        .enumerate()
        .flat_map(|(index, page_markdown)| {
            let page_number = index + 1;
            page_content(page_markdown)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .into_iter()
                .map(move |text| PageLine { text, page_number })
        })
        .collect()
}

fn collect_paragraphs(page_texts: &[String]) -> Vec<PageParagraph> {
    page_texts
        .iter()
        .enumerate()
        .flat_map(|(index, page_markdown)| {
            let page_number = index + 1;
            page_content(page_markdown)
                .split("\n\n")
                .map(|paragraph| paragraph.replace('\n', " "))
                .map(|paragraph| paragraph.trim().to_string())
                .filter(|paragraph| !paragraph.is_empty())
                .collect::<Vec<_>>()
                .into_iter()
                .map(move |text| PageParagraph { text, page_number })
        })
        .collect()
}

fn pick_title(lines: &[PageLine]) -> Option<&PageLine> {
    lines
        .iter()
        .find(|line| {
            line.text.len() <= 120
                && !line.text.ends_with('.')
                && line.text.chars().any(char::is_alphabetic)
        })
}

fn collect_headings(lines: &[PageLine], title: &str) -> Vec<NoteItem> {
    let mut seen = HashSet::new();
    lines
        .iter()
        .filter(|line| line.text.as_str() != title)
        .filter(|line| line.text.len() <= 80)
        .filter(|line| !line.text.ends_with('.'))
        .filter(|line| line.text.split_whitespace().count() <= 10)
        .filter(|line| seen.insert(line.text.to_ascii_lowercase()))
        .take(8)
        .map(|line| NoteItem {
            text: line.text.clone(),
            source_pages: vec![line.page_number],
        })
        .collect()
}

fn collect_key_points(paragraphs: &[PageParagraph]) -> Vec<NoteItem> {
    paragraphs
        .iter()
        .filter(|paragraph| paragraph.text.chars().filter(|ch| ch.is_alphanumeric()).count() >= 20)
        .take(8)
        .map(|paragraph| NoteItem {
            text: truncate(&paragraph.text, 180),
            source_pages: vec![paragraph.page_number],
        })
        .collect()
}

fn collect_glossary(lines: &[PageLine], paragraphs: &[PageParagraph]) -> Vec<GlossaryItem> {
    let definition_re =
        Regex::new(r"^(?P<term>[A-Za-z][A-Za-z0-9 ()/\-]{1,40}):\s+(?P<definition>.+)$").unwrap();
    let mut glossary = Vec::new();
    let mut seen = HashSet::new();

    for line in lines {
        if let Some(captures) = definition_re.captures(&line.text) {
            let term = captures.name("term").unwrap().as_str().trim().to_string();
            let definition = truncate(captures.name("definition").unwrap().as_str().trim(), 140);
            if seen.insert(term.to_ascii_lowercase()) {
                glossary.push(GlossaryItem {
                    term,
                    definition,
                    source_pages: vec![line.page_number],
                });
            }
        }
    }

    if glossary.len() >= 4 {
        return glossary.into_iter().take(8).collect();
    }

    for paragraph in paragraphs {
        for token in paragraph.text.split_whitespace() {
            let cleaned = token.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '-');
            if cleaned.len() >= 3
                && cleaned.len() <= 16
                && cleaned
                    .chars()
                    .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-')
            {
                let term = cleaned.to_string();
                if seen.insert(term.to_ascii_lowercase()) {
                    glossary.push(GlossaryItem {
                        term,
                        definition: truncate(&paragraph.text, 140),
                        source_pages: vec![paragraph.page_number],
                    });
                }
            }
        }
    }

    glossary.into_iter().take(8).collect()
}

fn collect_formulas(lines: &[PageLine]) -> Vec<NoteItem> {
    let mut seen = HashSet::new();
    lines
        .iter()
        .filter(|line| {
            line.text.contains('=')
                || line.text.contains('^')
                || line.text.contains('÷')
                || line.text.contains('×')
                || line.text.contains(" + ")
                || line.text.contains(" - ")
        })
        .filter(|line| seen.insert(line.text.to_ascii_lowercase()))
        .take(8)
        .map(|line| NoteItem {
            text: line.text.clone(),
            source_pages: vec![line.page_number],
        })
        .collect()
}

fn collect_examples(paragraphs: &[PageParagraph]) -> Vec<NoteItem> {
    paragraphs
        .iter()
        .filter(|paragraph| {
            let lower = paragraph.text.to_ascii_lowercase();
            lower.contains("example")
                || lower.contains("for instance")
                || lower.contains("e.g.")
                || lower.contains("such as")
        })
        .take(6)
        .map(|paragraph| NoteItem {
            text: truncate(&paragraph.text, 180),
            source_pages: vec![paragraph.page_number],
        })
        .collect()
}

fn collect_review_questions(
    title: &str,
    title_pages: &[usize],
    headings: &[NoteItem],
    glossary: &[GlossaryItem],
    key_points: &[NoteItem],
) -> Vec<NoteItem> {
    let mut questions = Vec::new();

    questions.push(NoteItem {
        text: format!("What is the main idea of {title}?"),
        source_pages: title_pages.to_vec(),
    });

    for heading in headings.iter().take(4) {
        questions.push(NoteItem {
            text: format!("How would you explain {} in your own words?", heading.text),
            source_pages: heading.source_pages.clone(),
        });
    }

    for item in glossary.iter().take(4) {
        questions.push(NoteItem {
            text: format!("What does {} mean, and why is it important?", item.term),
            source_pages: item.source_pages.clone(),
        });
    }

    for point in key_points.iter().take(3) {
        questions.push(NoteItem {
            text: format!("Why does this matter: {}?", truncate(&point.text, 90)),
            source_pages: point.source_pages.clone(),
        });
    }

    questions
}

fn collect_memory_checks(
    glossary: &[GlossaryItem],
    formulas: &[NoteItem],
    headings: &[NoteItem],
    key_points: &[NoteItem],
) -> Vec<NoteItem> {
    let mut checks = Vec::new();

    for item in glossary.iter().take(4) {
        checks.push(NoteItem {
            text: format!(
                "Flashcard front: {}; back: {}",
                item.term,
                truncate(&item.definition, 90)
            ),
            source_pages: item.source_pages.clone(),
        });
    }

    for formula in formulas.iter().take(3) {
        checks.push(NoteItem {
            text: format!(
                "Practice when to use `{}` and what each symbol means.",
                formula.text
            ),
            source_pages: formula.source_pages.clone(),
        });
    }

    for heading in headings.iter().take(3) {
        checks.push(NoteItem {
            text: format!("Summarize {} from memory in 2 sentences.", heading.text),
            source_pages: heading.source_pages.clone(),
        });
    }

    for point in key_points.iter().take(2) {
        checks.push(NoteItem {
            text: format!(
                "Create a quick self-test from this point: {}",
                truncate(&point.text, 100)
            ),
            source_pages: point.source_pages.clone(),
        });
    }

    checks
}

fn format_glossary_item(item: &GlossaryItem) -> String {
    match format_source_suffix(&item.source_pages) {
        Some(source_suffix) => {
            format!("- **{}**: {} {}", item.term, item.definition, source_suffix)
        }
        None => format!("- **{}**: {}", item.term, item.definition),
    }
}

fn format_note_item(item: &NoteItem) -> String {
    match format_source_suffix(&item.source_pages) {
        Some(source_suffix) => format!("- {} {}", item.text, source_suffix),
        None => format!("- {}", item.text),
    }
}

fn format_source_suffix(source_pages: &[usize]) -> Option<String> {
    let normalized_pages = normalize_source_pages(source_pages);
    if normalized_pages.is_empty() {
        return None;
    }

    let links = normalized_pages
        .iter()
        .map(|page_number| format!("[p. {}](#source-page-{})", page_number, page_number))
        .collect::<Vec<_>>()
        .join(", ");

    Some(format!("_(Source: {links})_"))
}

fn normalize_source_pages(source_pages: &[usize]) -> Vec<usize> {
    let mut pages = source_pages
        .iter()
        .copied()
        .filter(|page_number| *page_number > 0)
        .collect::<Vec<_>>();
    pages.sort_unstable();
    pages.dedup();
    pages
}

fn truncate(input: &str, max_len: usize) -> String {
    let trimmed = input.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }

    let mut result = String::new();
    for ch in trimmed.chars().take(max_len.saturating_sub(1)) {
        result.push(ch);
    }
    result.push('…');
    result
}
