namespace App.Models;

public sealed record AppSettings
{
    public AppThemePreference Theme { get; init; } = AppThemePreference.System;

    public RuntimeProfile RuntimeProfile { get; init; } = RuntimeProfile.CpuCompatible;

    public OcrWorkflowMode DefaultWorkflowMode { get; init; } = OcrWorkflowMode.ExactOcr;

    public OcrExportFormat DefaultExportFormat { get; init; } = OcrExportFormat.Markdown;

    public string ExtractTemplateId { get; init; } = "invoice_receipt";

    public string? ModelProfileId { get; init; }

    public string? ModelFile { get; init; }

    public int Dpi { get; init; } = 300;

    public int Threads { get; init; } = Math.Max(1, Environment.ProcessorCount - 1);

    public int MaxOcrDimension { get; init; } = 1600;

    public bool IdleModelPrewarm { get; init; } = true;

    public bool StudyBoost { get; init; }

    public bool HasCompletedOnboarding { get; init; }

    public string PrivacyNotice { get; init; } = "OCR and document processing stay on this device. Network access is only used for model downloads.";
}
