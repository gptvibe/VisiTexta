using App.Models;

namespace App.Core.Workflow;

public sealed record WorkflowModeDefinition
{
    public OcrWorkflowMode Mode { get; init; }

    public string Label { get; init; } = string.Empty;

    public string Description { get; init; } = string.Empty;

    public IReadOnlyList<OcrExportFormat> ExportFormats { get; init; } = [];
}

public static class WorkflowDefinitions
{
    public static IReadOnlyList<WorkflowModeDefinition> All { get; } =
    [
        new()
        {
            Mode = OcrWorkflowMode.ExactOcr,
            Label = "Exact OCR",
            Description = "Faithful local OCR transcription to Markdown.",
            ExportFormats = [OcrExportFormat.Markdown, OcrExportFormat.Text]
        },
        new()
        {
            Mode = OcrWorkflowMode.Notes,
            Label = "Notes",
            Description = "Local OCR shaped into source-linked study notes.",
            ExportFormats = [OcrExportFormat.Markdown, OcrExportFormat.Text, OcrExportFormat.Pdf, OcrExportFormat.Csv]
        },
        new()
        {
            Mode = OcrWorkflowMode.Extract,
            Label = "Extract",
            Description = "Local OCR shaped into structured fields, rows, and verification notes.",
            ExportFormats = [OcrExportFormat.Markdown, OcrExportFormat.Json, OcrExportFormat.Csv, OcrExportFormat.Text]
        }
    ];

    public static IReadOnlyList<OcrExportFormat> FormatsFor(OcrWorkflowMode mode)
    {
        return All.First(item => item.Mode == mode).ExportFormats;
    }
}
