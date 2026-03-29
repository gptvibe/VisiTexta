use crate::extract::{build_extract_markdown, default_extract_template_id, ExtractOptions};
use crate::formatting::clean_markdown;
use crate::study::{build_study_notes, StudyOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    ExactOcr,
    Notes,
    Extract,
}

impl Default for WorkflowMode {
    fn default() -> Self {
        Self::ExactOcr
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeExportOption {
    pub id: &'static str,
    pub label: &'static str,
    pub extension: &'static str,
    pub description: &'static str,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeDefinition {
    pub id: WorkflowMode,
    pub label: &'static str,
    pub short_label: &'static str,
    pub description: &'static str,
    pub helper: &'static str,
    pub result_label: &'static str,
    pub empty_state_copy: &'static str,
    pub copy_action_label: &'static str,
    pub save_action_label: &'static str,
    pub advanced_panel_copy: &'static str,
    pub prompt_label: &'static str,
    pub prompt_hint: &'static str,
    pub prompt_placeholder: &'static str,
    pub default_prompt: &'static str,
    pub available_exports: Vec<ModeExportOption>,
}

#[derive(Debug, Clone)]
pub struct ResolvedWorkflowPrompt {
    pub prompt: String,
    pub used_custom_override: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PostProcessOptions<'a> {
    pub study_boost: bool,
    pub custom_override: Option<&'a str>,
    pub extract_template_id: Option<&'a str>,
}

pub fn default_workflow_mode() -> WorkflowMode {
    WorkflowMode::ExactOcr
}

pub fn workflow_modes() -> Vec<ModeDefinition> {
    vec![
        ModeDefinition {
            id: WorkflowMode::ExactOcr,
            label: "Exact OCR",
            short_label: "Exact",
            description: "Preserve the current OCR-to-markdown behavior for faithful transcription.",
            helper: "Best when layout fidelity matters and you want the same OCR-first markdown flow VisiTexta already uses.",
            result_label: "Markdown",
            empty_state_copy: "Markdown will appear here when text is ready.",
            copy_action_label: "Copy markdown",
            save_action_label: "Export markdown",
            advanced_panel_copy: "Use Exact OCR when you want a faithful transcription. Advanced instructions stay optional and act as an override.",
            prompt_label: "Custom OCR override",
            prompt_hint: "Optional. Leave blank to keep Exact OCR's current transcription behavior.",
            prompt_placeholder: "Extract all text from the image and return it as markdown.",
            default_prompt: "Extract all text from the image and return it as markdown.",
            available_exports: vec![
                ModeExportOption {
                    id: "markdown",
                    label: "Markdown",
                    extension: "md",
                    description: "Faithful OCR markdown output.",
                    primary: true,
                },
                ModeExportOption {
                    id: "text",
                    label: "Plain text",
                    extension: "txt",
                    description: "A plain-text copy without markdown syntax.",
                    primary: false,
                },
            ],
        },
        ModeDefinition {
            id: WorkflowMode::Notes,
            label: "Notes",
            short_label: "Notes",
            description: "Turn the document into concise markdown notes for quick reading.",
            helper: "Best for lectures, handouts, and PDFs you want summarized into useful headings and bullets.",
            result_label: "Notes",
            empty_state_copy: "Notes will appear here when the document summary is ready.",
            copy_action_label: "Copy notes",
            save_action_label: "Export notes",
            advanced_panel_copy: "Notes mode keeps the OCR pipeline local, then shapes the result into concise note-friendly markdown.",
            prompt_label: "Custom notes override",
            prompt_hint: "Optional. Leave blank to use the default notes prompt for concise headings and bullets.",
            prompt_placeholder: "Turn this document into concise markdown notes with short headings and bullet points. Keep important names, dates, numbers, and action items.",
            default_prompt: "Turn this document into concise markdown notes with short headings and bullet points. Keep important names, dates, numbers, and action items.",
            available_exports: vec![
                ModeExportOption {
                    id: "markdown",
                    label: "Markdown notes",
                    extension: "md",
                    description: "Structured notes in markdown.",
                    primary: true,
                },
                ModeExportOption {
                    id: "text",
                    label: "Plain text notes",
                    extension: "txt",
                    description: "Readable notes without markdown syntax.",
                    primary: false,
                },
                ModeExportOption {
                    id: "pdf",
                    label: "Note PDF",
                    extension: "pdf",
                    description: "Searchable text-based PDF notes.",
                    primary: false,
                },
                ModeExportOption {
                    id: "csv",
                    label: "Flashcard / Anki CSV",
                    extension: "csv",
                    description: "Question/answer cards for study review.",
                    primary: false,
                },
            ],
        },
        ModeDefinition {
            id: WorkflowMode::Extract,
            label: "Extract",
            short_label: "Extract",
            description: "Pull out key details, fields, dates, numbers, and action items.",
            helper: "Best for invoices, forms, receipts, and documents where you want the important details separated from the rest.",
            result_label: "Extract",
            empty_state_copy: "The extracted details will appear here when processing finishes.",
            copy_action_label: "Copy extract",
            save_action_label: "Export extract",
            advanced_panel_copy: "Extract mode prioritizes key fields and important details instead of preserving the full transcript line by line.",
            prompt_label: "Custom extract override",
            prompt_hint: "Optional. Leave blank to use the default extraction prompt for structured key details.",
            prompt_placeholder: "Extract the important fields, entities, dates, amounts, identifiers, and action items from this document. Return a structured markdown extract with short sections and bullets.",
            default_prompt: "Extract the important fields, entities, dates, amounts, identifiers, and action items from this document. Return a structured markdown extract with short sections and bullets.",
            available_exports: vec![
                ModeExportOption {
                    id: "markdown",
                    label: "Markdown extract",
                    extension: "md",
                    description: "Structured extraction in markdown.",
                    primary: true,
                },
                ModeExportOption {
                    id: "json",
                    label: "JSON extract",
                    extension: "json",
                    description: "Structured export with fields, rows, source pages, and verification notes.",
                    primary: false,
                },
                ModeExportOption {
                    id: "csv",
                    label: "CSV extract",
                    extension: "csv",
                    description: "Rows when detected, otherwise a field/value review CSV.",
                    primary: false,
                },
                ModeExportOption {
                    id: "text",
                    label: "Plain text extract",
                    extension: "txt",
                    description: "A plain-text extraction export.",
                    primary: false,
                },
            ],
        },
    ]
}

pub fn workflow_mode_definition(mode: WorkflowMode) -> ModeDefinition {
    workflow_modes()
        .into_iter()
        .find(|candidate| candidate.id == mode)
        .expect("workflow mode definition must exist")
}

pub fn resolve_prompt(mode: WorkflowMode, prompt_override: Option<&str>) -> ResolvedWorkflowPrompt {
    let prompt_override = prompt_override
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match prompt_override {
        Some(prompt) => ResolvedWorkflowPrompt {
            prompt: prompt.to_string(),
            used_custom_override: true,
        },
        None => ResolvedWorkflowPrompt {
            prompt: workflow_mode_definition(mode).default_prompt.to_string(),
            used_custom_override: false,
        },
    }
}

pub fn post_process(
    mode: WorkflowMode,
    page_texts: &[String],
    options: PostProcessOptions<'_>,
) -> String {
    match mode {
        WorkflowMode::ExactOcr => clean_markdown(&page_texts.join("\n")),
        WorkflowMode::Notes => build_study_notes(
            page_texts,
            StudyOptions {
                study_boost: options.study_boost,
                custom_override: options.custom_override,
            },
        ),
        WorkflowMode::Extract => build_extract_markdown(
            page_texts,
            ExtractOptions {
                template_id: options
                    .extract_template_id
                    .unwrap_or(default_extract_template_id()),
                custom_override: options.custom_override,
            },
        ),
    }
}
