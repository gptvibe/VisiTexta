use crate::errors::Result;
use crate::storage;
use regex::Regex;
use std::path::Path;

const PAGE_WIDTH: f32 = 612.0;
const PAGE_HEIGHT: f32 = 792.0;
const MARGIN: f32 = 54.0;
const FONT_SIZE: f32 = 11.0;
const LINE_HEIGHT: f32 = 15.0;
const MAX_CHARS_PER_LINE: usize = 88;

pub fn save_printable_pdf(title: &str, content: &str, dest_path: &Path) -> Result<()> {
    let text = markdown_to_print_text(title, content);
    let lines = wrap_text(&text, MAX_CHARS_PER_LINE);
    let lines_per_page = ((PAGE_HEIGHT - (MARGIN * 2.0)) / LINE_HEIGHT).floor() as usize;
    let page_lines = lines_per_page.max(1);
    let pages: Vec<Vec<String>> = if lines.is_empty() {
        vec![vec![String::new()]]
    } else {
        lines
            .chunks(page_lines)
            .map(|chunk| chunk.to_vec())
            .collect()
    };

    let mut objects = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());

    let page_object_ids: Vec<usize> = (0..pages.len()).map(|index| 3 + (index * 2)).collect();
    let kids = page_object_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        pages.len(),
        kids
    ));

    for (index, page) in pages.iter().enumerate() {
        let page_object_id = 3 + (index * 2);
        let content_object_id = page_object_id + 1;
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 {} 0 R >> >> /Contents {} 0 R >>",
            PAGE_WIDTH,
            PAGE_HEIGHT,
            3 + (pages.len() * 2),
            content_object_id
        ));
        let stream = build_page_stream(page);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.as_bytes().len(),
            stream
        ));
    }

    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }

    let xref_offset = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    bytes.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );

    storage::atomic_write(dest_path, &bytes)?;
    Ok(())
}

fn build_page_stream(lines: &[String]) -> String {
    let mut stream = format!("BT\n/F1 {} Tf\n", FONT_SIZE);
    let mut y = PAGE_HEIGHT - MARGIN;
    for line in lines {
        stream.push_str(&format!(
            "1 0 0 1 {} {} Tm ({}) Tj\n",
            MARGIN,
            y,
            escape_pdf_text(line)
        ));
        y -= LINE_HEIGHT;
    }
    stream.push_str("ET");
    stream
}

fn escape_pdf_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn markdown_to_print_text(title: &str, content: &str) -> String {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let mut in_code_block = false;
    let mut lines: Vec<String> = Vec::new();

    for raw_line in normalized.lines() {
        let trimmed = raw_line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if trimmed.starts_with("<!--") {
            continue;
        }

        if trimmed.is_empty() {
            if !matches!(lines.last(), Some(last) if last.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }

        let plain_line = if in_code_block {
            strip_inline_markdown(trimmed)
        } else {
            format_markdown_line(trimmed)
        };

        if plain_line.is_empty() {
            continue;
        }

        lines.push(plain_line);
    }

    let body = lines.join("\n").trim().to_string();
    if body.is_empty() {
        return title.to_string();
    }

    let title_line = strip_inline_markdown(title.trim());
    if body.lines().next() == Some(title_line.as_str()) {
        body
    } else {
        format!("{title_line}\n\n{body}")
    }
}

fn format_markdown_line(line: &str) -> String {
    if let Some(content) = line.strip_prefix("# ") {
        return strip_inline_markdown(content).to_uppercase();
    }

    if let Some(content) = line.strip_prefix("## ") {
        return strip_inline_markdown(content);
    }

    if let Some(content) = line.strip_prefix("### ") {
        return strip_inline_markdown(content);
    }

    if let Some(content) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return format!("- {}", strip_inline_markdown(content));
    }

    let ordered_list = Regex::new(r"^(?P<number>\d+)\.\s+(?P<body>.+)$").unwrap();
    if let Some(captures) = ordered_list.captures(line) {
        let number = captures.name("number").unwrap().as_str();
        let body = captures.name("body").unwrap().as_str();
        return format!("{number}. {}", strip_inline_markdown(body));
    }

    strip_inline_markdown(line)
}

fn strip_inline_markdown(text: &str) -> String {
    let link_re = Regex::new(r"\[(?P<label>[^\]]+)\]\([^)]+\)").unwrap();
    let comment_re = Regex::new(r"<!--.*?-->").unwrap();

    let without_links = link_re.replace_all(text, "$label");
    let without_comments = comment_re.replace_all(&without_links, "");

    without_comments
        .replace("**", "")
        .replace("__", "")
        .replace('*', "")
        .replace('_', "")
        .replace('`', "")
        .trim()
        .to_string()
}

fn wrap_text(input: &str, width: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            wrapped.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let pending_len = if current.is_empty() {
                word.len()
            } else {
                current.len() + 1 + word.len()
            };

            if pending_len > width && !current.is_empty() {
                wrapped.push(current.clone());
                current.clear();
            }

            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }

        if !current.is_empty() {
            wrapped.push(current);
        }
    }
    wrapped
}
