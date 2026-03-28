use crate::models;
use crate::runtime::RuntimeProfile;
use crate::settings::Settings;
use serde::Serialize;

pub const DEFAULT_PROMPT_TEXT: &str = "Extract all text from the image and return it as markdown.";
pub const DEFAULT_SYSTEM_PROMPT_TEXT: &str = "Transcribe the visible document text as markdown.";

#[derive(Debug, Clone, Serialize)]
pub struct ThemeOption {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeDefaults {
    pub default_theme: &'static str,
    pub options: Vec<ThemeOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProfileOption {
    pub id: RuntimeProfile,
    pub label: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeProfileDefaults {
    pub default_profile: RuntimeProfile,
    pub options: Vec<RuntimeProfileOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptDefaults {
    pub default_prompt: &'static str,
    pub system_prompt: &'static str,
    pub placeholder: &'static str,
    pub hint: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractionPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub dpi: u16,
    pub description: &'static str,
    pub meta: &'static str,
    pub tradeoff: &'static str,
    pub runtime_profile_override: Option<RuntimeProfile>,
    pub max_ocr_dimension: Option<u32>,
    pub lazy_preview_thumbnails: bool,
    pub disable_rich_preview_for_large_jobs: bool,
    pub large_job_page_threshold: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppDefaults {
    pub settings: Settings,
    pub theme: ThemeDefaults,
    pub runtime_profiles: RuntimeProfileDefaults,
    pub prompt: PromptDefaults,
    pub extraction_presets: Vec<ExtractionPreset>,
    pub recommended_model_profile_id: &'static str,
    pub recommended_model_label: &'static str,
    pub recommended_model_file: &'static str,
    pub recommended_model_repo: &'static str,
}

pub fn default_theme_id() -> &'static str {
    "system"
}

pub fn runtime_profile_options() -> Vec<RuntimeProfileOption> {
    vec![
        RuntimeProfileOption {
            id: RuntimeProfile::Auto,
            label: "Auto",
            description:
                "Auto prefers a bundled accelerated runtime when this PC appears compatible, then falls back to the CPU-compatible path.",
        },
        RuntimeProfileOption {
            id: RuntimeProfile::CpuCompatible,
            label: "CPU compatible",
            description:
                "CPU compatible is the safe default and uses the widest-compatibility local runtime build.",
        },
        RuntimeProfileOption {
            id: RuntimeProfile::AcceleratedIfAvailable,
            label: "Accelerated if available",
            description:
                "Accelerated if available tries bundled acceleration first, then falls back cleanly if it is missing or incompatible.",
        },
    ]
}

pub fn extraction_presets() -> Vec<ExtractionPreset> {
    vec![
        ExtractionPreset {
            id: "starter",
            label: "Starter",
            dpi: 200,
            description: "Low-end hardware profile for older CPUs, thin laptops, and memory-sensitive batches.",
            meta: "Lower DPI, CPU-safe, lighter previews",
            tradeoff: "Best when you need lower memory use and steadier responsiveness. Small print and rich page previews will be less detailed.",
            runtime_profile_override: Some(RuntimeProfile::CpuCompatible),
            max_ocr_dimension: Some(1200),
            lazy_preview_thumbnails: true,
            disable_rich_preview_for_large_jobs: true,
            large_job_page_threshold: Some(12),
        },
        ExtractionPreset {
            id: "recommended",
            label: "Recommended",
            dpi: 300,
            description: "Best default for most screenshots, scans, and PDFs.",
            meta: "Balanced speed and readability",
            tradeoff: "Balanced default. Uses your selected runtime and keeps rich preview behavior on normal jobs.",
            runtime_profile_override: None,
            max_ocr_dimension: None,
            lazy_preview_thumbnails: false,
            disable_rich_preview_for_large_jobs: false,
            large_job_page_threshold: None,
        },
        ExtractionPreset {
            id: "quality",
            label: "Higher quality",
            dpi: 360,
            description: "Sharper rendering for small print and dense documents.",
            meta: "Slower, but easier on tiny text",
            tradeoff: "Best for tiny text and dense scans. Uses more time and memory than the other presets.",
            runtime_profile_override: None,
            max_ocr_dimension: None,
            lazy_preview_thumbnails: false,
            disable_rich_preview_for_large_jobs: false,
            large_job_page_threshold: None,
        },
        ExtractionPreset {
            id: "faster",
            label: "Faster",
            dpi: 220,
            description: "Quickest option for clean documents and large batches.",
            meta: "Fastest turnaround",
            tradeoff: "Optimized for quick turnaround on clean pages. It keeps richer previews than Starter, but with less OCR detail than Recommended.",
            runtime_profile_override: None,
            max_ocr_dimension: None,
            lazy_preview_thumbnails: false,
            disable_rich_preview_for_large_jobs: false,
            large_job_page_threshold: None,
        },
    ]
}

pub fn app_defaults() -> AppDefaults {
    AppDefaults {
        settings: Settings::default(),
        theme: ThemeDefaults {
            default_theme: default_theme_id(),
            options: vec![
                ThemeOption {
                    id: "system",
                    label: "System",
                },
                ThemeOption {
                    id: "light",
                    label: "Light",
                },
                ThemeOption {
                    id: "dark",
                    label: "Dark",
                },
            ],
        },
        runtime_profiles: RuntimeProfileDefaults {
            default_profile: crate::runtime::default_runtime_profile(),
            options: runtime_profile_options(),
        },
        prompt: PromptDefaults {
            default_prompt: DEFAULT_PROMPT_TEXT,
            system_prompt: DEFAULT_SYSTEM_PROMPT_TEXT,
            placeholder: DEFAULT_PROMPT_TEXT,
            hint: "Optional. Leave blank for the standard OCR prompt.",
        },
        extraction_presets: extraction_presets(),
        recommended_model_profile_id: models::recommended_model_profile_id(),
        recommended_model_label: models::recommended_model_label(),
        recommended_model_file: models::recommended_model_file_name(),
        recommended_model_repo: models::recommended_model_repo(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_defaults_match_backend_recommendations() {
        let defaults = app_defaults();

        assert_eq!(defaults.theme.default_theme, "system");
        assert_eq!(
            defaults.runtime_profiles.default_profile,
            crate::runtime::default_runtime_profile()
        );
        assert_eq!(defaults.prompt.default_prompt, DEFAULT_PROMPT_TEXT);
        assert_eq!(defaults.settings.theme.as_deref(), Some(default_theme_id()));
        assert_eq!(defaults.theme.options.len(), 3);
        assert_eq!(
            defaults.recommended_model_profile_id,
            models::recommended_model_profile_id()
        );
    }

    #[test]
    fn extraction_presets_keep_expected_order() {
        let defaults = app_defaults();
        let preset_ids = defaults
            .extraction_presets
            .iter()
            .map(|preset| preset.id)
            .collect::<Vec<_>>();

        assert_eq!(
            preset_ids,
            vec!["starter", "recommended", "quality", "faster"]
        );
        assert_eq!(defaults.extraction_presets[1].dpi, 300);
        assert_eq!(
            defaults.extraction_presets[0].runtime_profile_override,
            Some(RuntimeProfile::CpuCompatible)
        );
    }
}
