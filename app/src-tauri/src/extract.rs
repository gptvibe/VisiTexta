use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;

pub const TEMPLATE_INVOICE_RECEIPT: &str = "invoice_receipt";
pub const TEMPLATE_TABLE_TO_CSV: &str = "table_to_csv";
pub const TEMPLATE_MEETING_WHITEBOARD: &str = "meeting_whiteboard";
pub const TEMPLATE_CONTRACT_KEY_POINTS: &str = "contract_key_points";

#[derive(Debug, Clone, Serialize)]
pub struct ExtractTemplateDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub helper: &'static str,
    pub csv_hint: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ExtractOptions<'a> {
    pub template_id: &'a str,
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
struct Candidate {
    value: String,
    page_number: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructuredExtract {
    pub template_id: String,
    pub template_label: String,
    pub source_page_count: usize,
    pub summary: Vec<SourcedText>,
    pub fields: Vec<ExtractField>,
    pub rows: Vec<ExtractRow>,
    pub verification: Vec<SourcedText>,
    pub csv_export: CsvExport,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcedText {
    pub text: String,
    pub source_pages: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractField {
    pub key: String,
    pub label: String,
    pub value: Option<String>,
    pub source_pages: Vec<usize>,
    pub needs_verification: bool,
    pub verification_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractRow {
    pub cells: Vec<ExtractCell>,
    pub source_pages: Vec<usize>,
    pub needs_verification: bool,
    pub verification_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractCell {
    pub column: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CsvExport {
    pub mode: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn default_extract_template_id() -> &'static str {
    TEMPLATE_INVOICE_RECEIPT
}

pub fn extract_templates() -> Vec<ExtractTemplateDefinition> {
    vec![
        ExtractTemplateDefinition {
            id: TEMPLATE_INVOICE_RECEIPT,
            label: "Invoice / Receipt",
            description:
                "Pull out vendor, dates, totals, payment details, and likely line items for office and field paperwork.",
            helper: "Best for invoices, receipts, purchase slips, and expense paperwork.",
            csv_hint: "Exports line items to CSV when rows are detected; otherwise falls back to field/value CSV.",
        },
        ExtractTemplateDefinition {
            id: TEMPLATE_TABLE_TO_CSV,
            label: "Table to CSV",
            description:
                "Detect column-style rows and preserve them as markdown plus structured table data.",
            helper: "Best for tabular scans, printed schedules, reports, and pasted spreadsheet photos.",
            csv_hint: "Exports the detected table rows directly to CSV.",
        },
        ExtractTemplateDefinition {
            id: TEMPLATE_MEETING_WHITEBOARD,
            label: "Meeting Photo / Whiteboard",
            description:
                "Turn rough meeting photos into readable notes, action items, and follow-ups.",
            helper: "Best for whiteboards, brainstorm walls, sprint boards, and meeting snapshots.",
            csv_hint: "Exports action items as CSV when task rows are detected; otherwise uses field/value CSV.",
        },
        ExtractTemplateDefinition {
            id: TEMPLATE_CONTRACT_KEY_POINTS,
            label: "Contract Key Points",
            description:
                "Summarize the clauses that usually matter first for review and handoff.",
            helper: "Best for agreements, statements of work, vendor terms, and policy extracts.",
            csv_hint: "Exports key fields and clauses as field/value CSV for review.",
        },
    ]
}

pub fn extract_template_definition(id: &str) -> ExtractTemplateDefinition {
    extract_templates()
        .into_iter()
        .find(|template| template.id == id)
        .unwrap_or_else(|| {
            extract_templates()
                .into_iter()
                .find(|template| template.id == default_extract_template_id())
                .expect("default extract template must exist")
        })
}

pub fn build_extract_markdown(page_texts: &[String], options: ExtractOptions<'_>) -> String {
    let template = extract_template_definition(options.template_id);
    let lines = collect_lines(page_texts);
    let paragraphs = collect_paragraphs(page_texts);

    if lines.is_empty() && paragraphs.is_empty() {
        return String::new();
    }

    let structured = match template.id {
        TEMPLATE_TABLE_TO_CSV => build_table_extract(&template, page_texts, &lines, &paragraphs),
        TEMPLATE_MEETING_WHITEBOARD => {
            build_meeting_extract(&template, page_texts, &lines, &paragraphs)
        }
        TEMPLATE_CONTRACT_KEY_POINTS => {
            build_contract_extract(&template, page_texts, &lines, &paragraphs)
        }
        _ => build_invoice_extract(&template, page_texts, &lines, &paragraphs),
    };

    render_extract_markdown(&template, &structured, options.custom_override)
}

fn build_invoice_extract(
    template: &ExtractTemplateDefinition,
    page_texts: &[String],
    lines: &[PageLine],
    _paragraphs: &[PageParagraph],
) -> StructuredExtract {
    let source_page_count = page_texts.len().max(1);
    let mut verification = Vec::new();
    let mut fields = Vec::new();
    let mut summary = Vec::new();

    let document_type = if lines
        .iter()
        .any(|line| line.text.to_ascii_lowercase().contains("receipt"))
    {
        "Receipt"
    } else {
        "Invoice"
    };

    let vendor_candidates = first_page_heading_candidates(lines, 5)
        .into_iter()
        .filter(|candidate| {
            let lower = candidate.value.to_ascii_lowercase();
            !lower.contains("invoice")
                && !lower.contains("receipt")
                && !lower.contains("statement")
                && !lower.contains("page ")
        })
        .collect::<Vec<_>>();
    let vendor = add_field(
        &mut fields,
        &mut verification,
        "vendor",
        "Vendor / Merchant",
        vendor_candidates,
        Some("Confirm the seller or issuer name manually."),
    );

    let document_id = add_field(
        &mut fields,
        &mut verification,
        "document_number",
        "Invoice / Receipt Number",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:invoice|receipt|order)\s*(?:number|no\.?|#)?\b[:\s#-]*([A-Z0-9][A-Z0-9\-\/]{2,})"
            )
            .unwrap(),
            1,
        ),
        Some("The document number was not found confidently."),
    );

    let date = add_field(
        &mut fields,
        &mut verification,
        "document_date",
        "Document Date",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:date|invoice date|receipt date|issued on)\b[:\s-]*([A-Za-z]{3,9}\s+\d{1,2},?\s+\d{2,4}|\d{1,4}[/-]\d{1,2}[/-]\d{1,4})"
            )
            .unwrap(),
            1,
        ),
        Some("The issue or purchase date may need manual checking."),
    );

    add_field(
        &mut fields,
        &mut verification,
        "due_date",
        "Due Date",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:due date|payment due|due)\b[:\s-]*([A-Za-z]{3,9}\s+\d{1,2},?\s+\d{2,4}|\d{1,4}[/-]\d{1,2}[/-]\d{1,4})"
            )
            .unwrap(),
            1,
        ),
        None,
    );

    add_field(
        &mut fields,
        &mut verification,
        "subtotal",
        "Subtotal",
        find_capture_candidates(
            lines,
            &Regex::new(r"(?i)\bsubtotal\b[^0-9A-Za-z]{0,6}([$€£]?\s?\d[\d,]*\.?\d{0,2})")
                .unwrap(),
            1,
        ),
        None,
    );

    add_field(
        &mut fields,
        &mut verification,
        "tax",
        "Tax / VAT",
        find_capture_candidates(
            lines,
            &Regex::new(r"(?i)\b(?:tax|vat|gst)\b[^0-9A-Za-z]{0,6}([$€£]?\s?\d[\d,]*\.?\d{0,2})")
                .unwrap(),
            1,
        ),
        None,
    );

    let total = add_field(
        &mut fields,
        &mut verification,
        "total",
        "Total",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:grand total|amount due|balance due|total due|total)\b[^0-9A-Za-z]{0,6}([$€£]?\s?\d[\d,]*\.?\d{0,2})"
            )
            .unwrap(),
            1,
        ),
        Some("The final total should be checked against the original document."),
    );

    add_field(
        &mut fields,
        &mut verification,
        "payment_method",
        "Payment Method",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:payment method|paid by|method)\b[:\s-]*([A-Za-z][A-Za-z0-9 /-]{2,})"
            )
            .unwrap(),
            1,
        ),
        None,
    );

    summary.push(sourced_text(
        match (&vendor, &total) {
            (Some(vendor), Some(total)) => format!("{document_type} from {vendor} with a detected total of {total}."),
            (Some(vendor), None) => format!("{document_type} from {vendor}."),
            (None, Some(total)) => format!("{document_type} with a detected total of {total}."),
            (None, None) => format!("{document_type} details were extracted, but several fields still need checking."),
        },
        combine_pages_from_options([field_pages(&fields, "vendor"), field_pages(&fields, "total")]),
    ));

    if let Some(date) = &date {
        summary.push(sourced_text(
            format!("Detected document date: {date}."),
            field_pages(&fields, "document_date"),
        ));
    }

    if let Some(document_id) = &document_id {
        summary.push(sourced_text(
            format!("Reference number: {document_id}."),
            field_pages(&fields, "document_number"),
        ));
    }

    let rows = detect_invoice_rows(lines, &mut verification);
    let csv_export = if !rows.is_empty() {
        rows_to_csv("line_items", &rows)
    } else {
        fields_to_csv("fields", &fields)
    };

    if rows.is_empty() {
        verification.push(sourced_text(
            "No clear line-item table was detected. If this invoice contains items, compare the original page manually."
                .to_string(),
            vec![1],
        ));
    }

    StructuredExtract {
        template_id: template.id.to_string(),
        template_label: template.label.to_string(),
        source_page_count,
        summary,
        fields,
        rows,
        verification: dedupe_sourced_text(verification),
        csv_export,
    }
}

fn build_table_extract(
    template: &ExtractTemplateDefinition,
    page_texts: &[String],
    lines: &[PageLine],
    _paragraphs: &[PageParagraph],
) -> StructuredExtract {
    let source_page_count = page_texts.len().max(1);
    let mut verification = Vec::new();
    let mut fields = Vec::new();
    let mut summary = Vec::new();
    let detected = detect_table_rows(lines);
    let rows = detected.rows;
    let columns_text = if detected.columns.is_empty() {
        None
    } else {
        Some(detected.columns.join(", "))
    };

    add_static_field(
        &mut fields,
        "columns",
        "Detected Columns",
        columns_text.clone(),
        detected.source_pages.clone(),
        columns_text.is_none(),
        columns_text
            .is_none()
            .then_some("Column headers were not clear enough to detect confidently.".to_string()),
    );

    add_static_field(
        &mut fields,
        "row_count",
        "Detected Rows",
        Some(rows.len().to_string()),
        detected.source_pages.clone(),
        rows.is_empty(),
        rows.is_empty()
            .then_some("No consistent table rows were detected. Review the scan manually.".to_string()),
    );

    if !rows.is_empty() {
        summary.push(sourced_text(
            format!(
                "Detected a table with {} rows and {} columns.",
                rows.len(),
                detected.columns.len()
            ),
            detected.source_pages.clone(),
        ));
    } else {
        summary.push(sourced_text(
            "A clean table was not detected. The document may need a cleaner scan or manual correction."
                .to_string(),
            vec![1],
        ));
    }

    if detected.inconsistent_rows > 0 {
        verification.push(sourced_text(
            format!(
                "{} row(s) had inconsistent column counts and may need manual cleanup before importing.",
                detected.inconsistent_rows
            ),
            detected.source_pages.clone(),
        ));
    }

    let csv_export = if !rows.is_empty() {
        rows_to_csv("table", &rows)
    } else {
        fields_to_csv("fields", &fields)
    };

    StructuredExtract {
        template_id: template.id.to_string(),
        template_label: template.label.to_string(),
        source_page_count,
        summary,
        fields,
        rows,
        verification: dedupe_sourced_text(verification),
        csv_export,
    }
}

fn build_meeting_extract(
    template: &ExtractTemplateDefinition,
    page_texts: &[String],
    lines: &[PageLine],
    paragraphs: &[PageParagraph],
) -> StructuredExtract {
    let source_page_count = page_texts.len().max(1);
    let mut verification = Vec::new();
    let mut fields = Vec::new();
    let mut summary = Vec::new();

    let title_candidate = first_page_heading_candidates(lines, 4)
        .into_iter()
        .find(|candidate| candidate.value.split_whitespace().count() <= 8);

    add_static_field(
        &mut fields,
        "meeting_title",
        "Meeting Title",
        title_candidate.as_ref().map(|candidate| candidate.value.clone()),
        title_candidate
            .as_ref()
            .map(|candidate| vec![candidate.page_number])
            .unwrap_or_else(|| vec![1]),
        title_candidate.is_none(),
        title_candidate
            .is_none()
            .then_some("The whiteboard title was not obvious from the scan.".to_string()),
    );

    let action_items = detect_action_rows(lines);
    let action_pages = action_items
        .iter()
        .flat_map(|row| row.source_pages.iter().copied())
        .collect::<Vec<_>>();
    add_static_field(
        &mut fields,
        "action_item_count",
        "Action Items",
        Some(action_items.len().to_string()),
        action_pages.clone(),
        action_items.is_empty(),
        action_items
            .is_empty()
            .then_some("No clear action items were detected. Check the source image manually.".to_string()),
    );

    for paragraph in paragraphs
        .iter()
        .filter(|paragraph| paragraph.text.chars().filter(|ch| ch.is_alphanumeric()).count() >= 20)
        .take(4)
    {
        summary.push(sourced_text(
            truncate(&paragraph.text, 180),
            vec![paragraph.page_number],
        ));
    }

    if summary.is_empty() {
        summary.push(sourced_text(
            "Meeting notes were detected, but the photo likely needs manual review for names, dates, and assignments."
                .to_string(),
            vec![1],
        ));
    }

    if action_items.is_empty() {
        verification.push(sourced_text(
            "This whiteboard did not yield reliable action-item rows. Verify owners and due dates manually."
                .to_string(),
            vec![1],
        ));
    } else {
        let ambiguous_actions = action_items
            .iter()
            .filter(|row| row.needs_verification)
            .count();
        if ambiguous_actions > 0 {
            verification.push(sourced_text(
                format!(
                    "{ambiguous_actions} action item(s) are missing an obvious owner or due date."
                ),
                action_pages.clone(),
            ));
        }
    }

    let csv_export = if !action_items.is_empty() {
        rows_to_csv("action_items", &action_items)
    } else {
        fields_to_csv("fields", &fields)
    };

    StructuredExtract {
        template_id: template.id.to_string(),
        template_label: template.label.to_string(),
        source_page_count,
        summary,
        fields,
        rows: action_items,
        verification: dedupe_sourced_text(verification),
        csv_export,
    }
}

fn build_contract_extract(
    template: &ExtractTemplateDefinition,
    page_texts: &[String],
    lines: &[PageLine],
    paragraphs: &[PageParagraph],
) -> StructuredExtract {
    let source_page_count = page_texts.len().max(1);
    let mut verification = Vec::new();
    let mut fields = Vec::new();
    let mut summary = Vec::new();

    let title_candidate = first_page_heading_candidates(lines, 5)
        .into_iter()
        .find(|candidate| candidate.value.split_whitespace().count() <= 12);
    add_static_field(
        &mut fields,
        "document_title",
        "Document Title",
        title_candidate.as_ref().map(|candidate| candidate.value.clone()),
        title_candidate
            .as_ref()
            .map(|candidate| vec![candidate.page_number])
            .unwrap_or_else(|| vec![1]),
        title_candidate.is_none(),
        title_candidate
            .is_none()
            .then_some("The contract title may need manual checking.".to_string()),
    );

    add_field(
        &mut fields,
        &mut verification,
        "parties",
        "Parties",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:between|among|by and between|parties?)\b[:\s-]*([A-Za-z0-9 ,.&()/-]{6,120})"
            )
            .unwrap(),
            1,
        ),
        Some("The named parties were not extracted confidently."),
    );

    add_field(
        &mut fields,
        &mut verification,
        "effective_date",
        "Effective Date",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:effective date|effective as of|commencement date)\b[:\s-]*([A-Za-z]{3,9}\s+\d{1,2},?\s+\d{2,4}|\d{1,4}[/-]\d{1,2}[/-]\d{1,4})"
            )
            .unwrap(),
            1,
        ),
        Some("The effective date should be verified manually."),
    );

    add_field(
        &mut fields,
        &mut verification,
        "term",
        "Term",
        find_capture_candidates(
            lines,
            &Regex::new(r"(?i)\bterm\b[:\s-]*([A-Za-z0-9 ,.-]{4,80})").unwrap(),
            1,
        ),
        None,
    );

    add_field(
        &mut fields,
        &mut verification,
        "renewal",
        "Renewal",
        find_capture_candidates(
            lines,
            &Regex::new(r"(?i)\b(?:renewal|auto-renewal|renews?)\b[:\s-]*([A-Za-z0-9 ,.-]{4,80})")
                .unwrap(),
            1,
        ),
        None,
    );

    add_field(
        &mut fields,
        &mut verification,
        "termination_notice",
        "Termination / Notice",
        find_capture_candidates(
            lines,
            &Regex::new(
                r"(?i)\b(?:termination|notice period|terminate)\b[:\s-]*([A-Za-z0-9 ,.-]{4,100})"
            )
            .unwrap(),
            1,
        ),
        None,
    );

    add_field(
        &mut fields,
        &mut verification,
        "payment_terms",
        "Payment Terms",
        find_capture_candidates(
            lines,
            &Regex::new(r"(?i)\b(?:payment terms|fees|payment)\b[:\s-]*([A-Za-z0-9 $€£,./()-]{4,100})")
                .unwrap(),
            1,
        ),
        None,
    );

    add_field(
        &mut fields,
        &mut verification,
        "governing_law",
        "Governing Law",
        find_capture_candidates(
            lines,
            &Regex::new(r"(?i)\bgoverning law\b[:\s-]*([A-Za-z ,.-]{3,60})").unwrap(),
            1,
        ),
        None,
    );

    for paragraph in paragraphs
        .iter()
        .filter(|paragraph| {
            let lower = paragraph.text.to_ascii_lowercase();
            lower.contains("shall")
                || lower.contains("must")
                || lower.contains("agrees to")
                || lower.contains("will")
        })
        .take(6)
    {
        summary.push(sourced_text(
            truncate(&paragraph.text, 180),
            vec![paragraph.page_number],
        ));
    }

    if summary.is_empty() {
        summary.push(sourced_text(
            "Key clauses were extracted, but the contract should still be reviewed against the source pages."
                .to_string(),
            vec![1],
        ));
    }

    let csv_export = fields_to_csv("fields", &fields);

    StructuredExtract {
        template_id: template.id.to_string(),
        template_label: template.label.to_string(),
        source_page_count,
        summary,
        fields,
        rows: Vec::new(),
        verification: dedupe_sourced_text(verification),
        csv_export,
    }
}

struct DetectedTable {
    columns: Vec<String>,
    rows: Vec<ExtractRow>,
    inconsistent_rows: usize,
    source_pages: Vec<usize>,
}

fn detect_table_rows(lines: &[PageLine]) -> DetectedTable {
    let mut best_rows: Vec<(Vec<String>, usize)> = Vec::new();
    let mut inconsistent_rows = 0;

    for window_start in 0..lines.len() {
        let Some(first_cells) = split_table_line(&lines[window_start].text) else {
            continue;
        };
        if first_cells.len() < 2 {
            continue;
        }

        let expected = first_cells.len();
        let mut candidate_rows = vec![(first_cells, lines[window_start].page_number)];
        let mut local_inconsistent = 0;

        for line in lines.iter().skip(window_start + 1) {
            let Some(cells) = split_table_line(&line.text) else {
                if candidate_rows.len() >= 3 {
                    break;
                }
                continue;
            };

            if cells.len() == expected {
                candidate_rows.push((cells, line.page_number));
            } else if (cells.len() as isize - expected as isize).abs() <= 1 {
                local_inconsistent += 1;
                if candidate_rows.len() >= 2 {
                    break;
                }
            } else if candidate_rows.len() >= 2 {
                break;
            }
        }

        if candidate_rows.len() > best_rows.len() {
            best_rows = candidate_rows;
            inconsistent_rows = local_inconsistent;
        }
    }

    if best_rows.is_empty() {
        return DetectedTable {
            columns: Vec::new(),
            rows: Vec::new(),
            inconsistent_rows: 0,
            source_pages: vec![1],
        };
    }

    let raw_columns = best_rows.first().map(|(cells, _)| cells.clone()).unwrap_or_default();
    let header_is_data = raw_columns
        .iter()
        .all(|cell| looks_amount(cell) || cell.chars().all(|ch| ch.is_ascii_digit()));
    let columns = if header_is_data {
        (0..raw_columns.len())
            .map(|index| format!("Column {}", index + 1))
            .collect::<Vec<_>>()
    } else {
        raw_columns
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let cleaned = value.trim().trim_matches(':');
                if cleaned.is_empty() {
                    format!("Column {}", index + 1)
                } else {
                    truncate(cleaned, 40)
                }
            })
            .collect::<Vec<_>>()
    };

    let data_rows = if header_is_data {
        best_rows
    } else {
        best_rows.into_iter().skip(1).collect()
    };

    let source_pages = data_rows
        .iter()
        .map(|(_, page_number)| *page_number)
        .collect::<Vec<_>>();

    let rows = data_rows
        .into_iter()
        .take(30)
        .map(|(cells, page_number)| ExtractRow {
            cells: columns
                .iter()
                .cloned()
                .zip(cells.into_iter())
                .map(|(column, value)| ExtractCell { column, value })
                .collect(),
            source_pages: vec![page_number],
            needs_verification: false,
            verification_note: None,
        })
        .collect::<Vec<_>>();

    DetectedTable {
        columns,
        rows,
        inconsistent_rows,
        source_pages: normalize_source_pages(&source_pages),
    }
}

fn detect_invoice_rows(lines: &[PageLine], verification: &mut Vec<SourcedText>) -> Vec<ExtractRow> {
    let amount_re = Regex::new(r"[$€£]?\s?\d[\d,]*\.?\d{0,2}$").unwrap();
    let mut rows = Vec::new();

    for line in lines {
        let lower = line.text.to_ascii_lowercase();
        if lower.contains("subtotal")
            || lower.contains("total")
            || lower.contains("tax")
            || lower.contains("vat")
        {
            continue;
        }

        let cells = split_table_line(&line.text).unwrap_or_else(|| {
            if amount_re.is_match(&line.text) {
                split_amount_tail(&line.text)
            } else {
                Vec::new()
            }
        });

        if cells.len() < 2 || !looks_amount(cells.last().unwrap_or(&String::new())) {
            continue;
        }

        let columns = match cells.len() {
            2 => vec!["Description".to_string(), "Amount".to_string()],
            3 => vec![
                "Description".to_string(),
                "Qty / Rate".to_string(),
                "Amount".to_string(),
            ],
            4 => vec![
                "Description".to_string(),
                "Qty".to_string(),
                "Unit Price".to_string(),
                "Amount".to_string(),
            ],
            _ => (0..cells.len())
                .map(|index| format!("Column {}", index + 1))
                .collect::<Vec<_>>(),
        };

        let verification_note = if cells
            .first()
            .map(|value| value.len() <= 2 || value.chars().all(|ch| ch.is_ascii_digit()))
            .unwrap_or(false)
        {
            Some("Item description may have been split incorrectly.".to_string())
        } else {
            None
        };

        rows.push(ExtractRow {
            cells: columns
                .into_iter()
                .zip(cells.into_iter())
                .map(|(column, value)| ExtractCell {
                    column,
                    value,
                })
                .collect(),
            source_pages: vec![line.page_number],
            needs_verification: verification_note.is_some(),
            verification_note,
        });
    }

    rows.truncate(12);

    if rows.iter().any(|row| row.needs_verification) {
        verification.push(sourced_text(
            "Some line items were reconstructed heuristically and should be checked against the original receipt."
                .to_string(),
            rows.iter()
                .flat_map(|row| row.source_pages.iter().copied())
                .collect::<Vec<_>>(),
        ));
    }

    rows
}

fn detect_action_rows(lines: &[PageLine]) -> Vec<ExtractRow> {
    let date_re = Regex::new(
        r"(?i)\b([A-Za-z]{3,9}\s+\d{1,2},?\s+\d{2,4}|\d{1,4}[/-]\d{1,2}[/-]\d{1,4}|tomorrow|today|next week)\b",
    )
    .unwrap();
    let owner_re = Regex::new(r"@([A-Za-z][A-Za-z0-9._-]{1,30})").unwrap();
    let action_re = Regex::new(
        r"(?i)\b(follow up|send|review|call|prepare|draft|update|confirm|check|finish|schedule|share)\b",
    )
    .unwrap();

    let mut rows = Vec::new();
    for line in lines {
        let trimmed = line.text.trim_start_matches(['-', '*', '•', '[', ']', ' ']).trim();
        if trimmed.len() < 6 {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if !action_re.is_match(&lower)
            && !lower.contains("todo")
            && !lower.contains("next steps")
            && !trimmed.starts_with("Action")
        {
            continue;
        }

        let due = date_re
            .captures(trimmed)
            .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()));
        let owner = owner_re
            .captures(trimmed)
            .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
            .or_else(|| {
                trimmed
                    .split_once(':')
                    .filter(|(prefix, _)| prefix.split_whitespace().count() <= 3)
                    .map(|(prefix, _)| prefix.trim().to_string())
            });

        let cells = vec![
            ExtractCell {
                column: "Task".to_string(),
                value: truncate(trimmed, 140),
            },
            ExtractCell {
                column: "Owner".to_string(),
                value: owner.clone().unwrap_or_default(),
            },
            ExtractCell {
                column: "Due".to_string(),
                value: due.clone().unwrap_or_default(),
            },
            ExtractCell {
                column: "Notes".to_string(),
                value: if owner.is_some() || due.is_some() {
                    String::new()
                } else {
                    "Owner or due date not obvious from the photo.".to_string()
                },
            },
        ];

        rows.push(ExtractRow {
            cells,
            source_pages: vec![line.page_number],
            needs_verification: owner.is_none() || due.is_none(),
            verification_note: (owner.is_none() || due.is_none())
                .then_some("Owner or due date may need manual checking.".to_string()),
        });
    }

    rows.truncate(12);
    rows
}

fn rows_to_csv(mode: &str, rows: &[ExtractRow]) -> CsvExport {
    let columns = rows
        .first()
        .map(|row| {
            let mut headers = row
                .cells
                .iter()
                .map(|cell| cell.column.clone())
                .collect::<Vec<_>>();
            headers.push("Source Pages".to_string());
            headers.push("Needs Verification".to_string());
            headers.push("Verification Note".to_string());
            headers
        })
        .unwrap_or_default();

    let rows = rows
        .iter()
        .map(|row| {
            let mut record = row
                .cells
                .iter()
                .map(|cell| cell.value.clone())
                .collect::<Vec<_>>();
            record.push(format_page_labels(&row.source_pages));
            record.push(if row.needs_verification {
                "yes".to_string()
            } else {
                "no".to_string()
            });
            record.push(row.verification_note.clone().unwrap_or_default());
            record
        })
        .collect::<Vec<_>>();

    CsvExport {
        mode: mode.to_string(),
        columns,
        rows,
    }
}

fn fields_to_csv(mode: &str, fields: &[ExtractField]) -> CsvExport {
    CsvExport {
        mode: mode.to_string(),
        columns: vec![
            "Field".to_string(),
            "Value".to_string(),
            "Source Pages".to_string(),
            "Needs Verification".to_string(),
            "Verification Note".to_string(),
        ],
        rows: fields
            .iter()
            .map(|field| {
                vec![
                    field.label.clone(),
                    field.value.clone().unwrap_or_default(),
                    format_page_labels(&field.source_pages),
                    if field.needs_verification {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    },
                    field.verification_note.clone().unwrap_or_default(),
                ]
            })
            .collect(),
    }
}

fn render_extract_markdown(
    template: &ExtractTemplateDefinition,
    structured: &StructuredExtract,
    custom_override: Option<&str>,
) -> String {
    let metadata_json = serde_json::to_string(structured)
        .unwrap_or_else(|_| "{}".to_string())
        .replace("--", "\\u002d\\u002d");

    let mut sections = vec![
        format!("<!-- visitexta-extract: {metadata_json} -->"),
        format!("# {}", template.label),
    ];

    if let Some(override_note) = custom_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!(
            "_Advanced override: {}_",
            override_note.replace('\n', " ")
        ));
    }

    sections.push(format!("_{}_", template.helper));

    if !structured.summary.is_empty() {
        sections.push("## Summary".to_string());
        sections.extend(structured.summary.iter().map(format_sourced_bullet));
    }

    if !structured.fields.is_empty() {
        sections.push("## Key Fields".to_string());
        sections.extend(structured.fields.iter().map(format_extract_field));
    }

    if !structured.rows.is_empty() {
        let heading = match template.id {
            TEMPLATE_INVOICE_RECEIPT => "## Line Items",
            TEMPLATE_MEETING_WHITEBOARD => "## Action Items",
            TEMPLATE_TABLE_TO_CSV => "## Table Rows",
            _ => "## Rows",
        };
        sections.push(heading.to_string());
        sections.push(render_rows_markdown(&structured.rows));
    }

    sections.push("## Structured JSON".to_string());
    sections.push(
        "- Export `JSON extract` for structured fields, rows, verification notes, and source pages."
            .to_string(),
    );
    sections.push(format!("- {}", template.csv_hint));

    sections.push("## Uncertainty / Verification".to_string());
    if structured.verification.is_empty() {
        sections.push(
            "- No obvious conflicts were detected, but important fields should still be checked against the source document."
                .to_string(),
        );
    } else {
        sections.extend(structured.verification.iter().map(format_sourced_bullet));
    }

    sections.join("\n\n").trim().to_string()
}

fn render_rows_markdown(rows: &[ExtractRow]) -> String {
    rows.iter()
        .map(|row| {
            let body = row
                .cells
                .iter()
                .map(|cell| format!("**{}**: {}", cell.column, cell.value))
                .collect::<Vec<_>>()
                .join("; ");
            let mut parts = vec![format!("- {body}")];
            if let Some(source_suffix) = format_source_suffix(&row.source_pages) {
                parts.push(source_suffix);
            }
            if let Some(note) = &row.verification_note {
                parts.push(format!("_(Check: {})_", note));
            } else if row.needs_verification {
                parts.push("_(Check manually.)_".to_string());
            }
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_extract_field(field: &ExtractField) -> String {
    let mut parts = vec![format!(
        "- **{}**: {}",
        field.label,
        field.value.as_deref().unwrap_or("Not found")
    )];

    if let Some(source_suffix) = format_source_suffix(&field.source_pages) {
        parts.push(source_suffix);
    }

    if let Some(note) = &field.verification_note {
        parts.push(format!("_(Check: {})_", note));
    } else if field.needs_verification {
        parts.push("_(Check manually.)_".to_string());
    }

    parts.join(" ")
}

fn format_sourced_bullet(item: &SourcedText) -> String {
    match format_source_suffix(&item.source_pages) {
        Some(source_suffix) => format!("- {} {}", item.text, source_suffix),
        None => format!("- {}", item.text),
    }
}

fn sourced_text(text: String, source_pages: Vec<usize>) -> SourcedText {
    SourcedText {
        text,
        source_pages: normalize_source_pages(&source_pages),
    }
}

fn add_field(
    fields: &mut Vec<ExtractField>,
    verification: &mut Vec<SourcedText>,
    key: &str,
    label: &str,
    candidates: Vec<Candidate>,
    missing_note: Option<&str>,
) -> Option<String> {
    let distinct = distinct_candidates(candidates);
    let source_pages = normalize_source_pages(
        &distinct
            .iter()
            .map(|candidate| candidate.page_number)
            .collect::<Vec<_>>(),
    );
    let value = distinct.first().map(|candidate| candidate.value.clone());
    let needs_verification = distinct.len() > 1
        || value.as_deref().map(looks_noisy).unwrap_or(false)
        || value.is_none();

    let verification_note = if distinct.len() > 1 {
        Some(format!(
            "Multiple candidates were found: {}.",
            distinct
                .iter()
                .map(|candidate| candidate.value.clone())
                .collect::<Vec<_>>()
                .join("; ")
        ))
    } else if value.as_deref().map(looks_noisy).unwrap_or(false) {
        Some("The extracted text looks noisy and should be checked manually.".to_string())
    } else {
        missing_note.map(ToOwned::to_owned).filter(|_| value.is_none())
    };

    if let Some(note) = &verification_note {
        verification.push(sourced_text(
            format!("{label}: {note}"),
            if source_pages.is_empty() {
                vec![1]
            } else {
                source_pages.clone()
            },
        ));
    }

    fields.push(ExtractField {
        key: key.to_string(),
        label: label.to_string(),
        value: value.clone(),
        source_pages,
        needs_verification,
        verification_note,
    });

    value
}

fn add_static_field(
    fields: &mut Vec<ExtractField>,
    key: &str,
    label: &str,
    value: Option<String>,
    source_pages: Vec<usize>,
    needs_verification: bool,
    verification_note: Option<String>,
) {
    fields.push(ExtractField {
        key: key.to_string(),
        label: label.to_string(),
        value,
        source_pages: normalize_source_pages(&source_pages),
        needs_verification,
        verification_note,
    });
}

fn find_capture_candidates(lines: &[PageLine], pattern: &Regex, capture_index: usize) -> Vec<Candidate> {
    lines
        .iter()
        .filter_map(|line| {
            pattern
                .captures(&line.text)
                .and_then(|captures| captures.get(capture_index))
                .map(|capture| Candidate {
                    value: capture.as_str().trim().trim_matches(':').to_string(),
                    page_number: line.page_number,
                })
        })
        .collect()
}

fn first_page_heading_candidates(lines: &[PageLine], limit: usize) -> Vec<Candidate> {
    lines
        .iter()
        .filter(|line| line.page_number == 1)
        .filter(|line| line.text.len() <= 80)
        .filter(|line| !line.text.ends_with('.'))
        .take(limit)
        .map(|line| Candidate {
            value: line.text.clone(),
            page_number: line.page_number,
        })
        .collect()
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

fn split_table_line(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim().trim_matches('|').trim();
    if trimmed.is_empty() {
        return None;
    }

    let pipe_parts = trimmed
        .split('|')
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if pipe_parts.len() >= 2 {
        return Some(pipe_parts);
    }

    let tab_parts = trimmed
        .split('\t')
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if tab_parts.len() >= 2 {
        return Some(tab_parts);
    }

    let multi_space = Regex::new(r"\s{2,}").unwrap();
    let space_parts = multi_space
        .split(trimmed)
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    (space_parts.len() >= 2).then_some(space_parts)
}

fn split_amount_tail(line: &str) -> Vec<String> {
    let amount_re = Regex::new(r"(?P<body>.+?)\s+(?P<amount>[$€£]?\s?\d[\d,]*\.?\d{0,2})$").unwrap();
    amount_re
        .captures(line.trim())
        .map(|captures| {
            vec![
                captures.name("body").unwrap().as_str().trim().to_string(),
                captures.name("amount").unwrap().as_str().trim().to_string(),
            ]
        })
        .unwrap_or_default()
}

fn distinct_candidates(candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.value.to_ascii_lowercase()))
        .collect()
}

fn format_source_suffix(source_pages: &[usize]) -> Option<String> {
    let normalized_pages = normalize_source_pages(source_pages);
    if normalized_pages.is_empty() {
        return None;
    }

    Some(format!(
        "_(Source: {})_",
        normalized_pages
            .iter()
            .map(|page_number| format!("[p. {}](#source-page-{})", page_number, page_number))
            .collect::<Vec<_>>()
            .join(", ")
    ))
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

fn combine_pages_from_options<const N: usize>(page_groups: [Vec<usize>; N]) -> Vec<usize> {
    normalize_source_pages(
        &page_groups
            .into_iter()
            .flat_map(|pages| pages.into_iter())
            .collect::<Vec<_>>(),
    )
}

fn field_pages(fields: &[ExtractField], key: &str) -> Vec<usize> {
    fields
        .iter()
        .find(|field| field.key == key)
        .map(|field| field.source_pages.clone())
        .unwrap_or_default()
}

fn format_page_labels(source_pages: &[usize]) -> String {
    normalize_source_pages(source_pages)
        .iter()
        .map(|page_number| format!("p. {page_number}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn looks_amount(value: &str) -> bool {
    Regex::new(r"^\$?€?£?\s?\d[\d,]*\.?\d{0,2}$")
        .unwrap()
        .is_match(value.trim())
}

fn looks_noisy(value: &str) -> bool {
    let punctuation = value
        .chars()
        .filter(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
        .count();
    punctuation > value.len().saturating_div(3) || value.contains('?')
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

fn dedupe_sourced_text(items: Vec<SourcedText>) -> Vec<SourcedText> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.text.to_ascii_lowercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoice_extract_includes_hidden_metadata_and_verification_section() {
        let markdown = build_extract_markdown(
            &[r#"## Page 1

ACME Supplies
Invoice Number: INV-1002
Date: 2026-03-14
Total: $42.50
Paper clips    12    $42.50
"#
            .to_string()],
            ExtractOptions {
                template_id: TEMPLATE_INVOICE_RECEIPT,
                custom_override: None,
            },
        );

        assert!(markdown.contains("<!-- visitexta-extract:"));
        assert!(markdown.contains("## Uncertainty / Verification"));
        assert!(markdown.contains("[p. 1](#source-page-1)"));
    }

    #[test]
    fn table_template_detects_simple_pipe_rows() {
        let page = r#"## Page 1

Name | Hours | Rate
Sam | 3 | 120
Lee | 2 | 140
"#
        .to_string();
        let structured = build_table_extract(
            &extract_template_definition(TEMPLATE_TABLE_TO_CSV),
            &[page.clone()],
            &collect_lines(&[page]),
            &[],
        );

        assert_eq!(structured.rows.len(), 2);
        assert_eq!(structured.csv_export.mode, "table");
    }
}
