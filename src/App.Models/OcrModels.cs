namespace App.Models;

public sealed record OcrJobOptions
{
    public string SourcePath { get; init; } = string.Empty;

    public OcrWorkflowMode WorkflowMode { get; init; } = OcrWorkflowMode.ExactOcr;

    public OcrExportFormat ExportFormat { get; init; } = OcrExportFormat.Markdown;

    public string? PromptOverride { get; init; }

    public bool StudyBoost { get; init; }

    public string ExtractTemplateId { get; init; } = "invoice_receipt";

    public int Dpi { get; init; } = 300;

    public int MaxOcrDimension { get; init; } = 1600;

    public RuntimeProfile RuntimeProfile { get; init; } = RuntimeProfile.CpuCompatible;

    public string? ModelProfileId { get; init; }

    public string? ModelFile { get; init; }
}

public sealed record OcrJobProgress
{
    public string JobId { get; init; } = string.Empty;

    public OcrJobStatus Status { get; init; }

    public double Percent { get; init; }

    public string Message { get; init; } = string.Empty;

    public int? PageNumber { get; init; }

    public int? TotalPages { get; init; }

    public int? RenderedPages { get; init; }

    public int? RecognizedPages { get; init; }
}

public sealed record OcrPageResult
{
    public int PageNumber { get; init; }

    public int TotalPages { get; init; }

    public string Markdown { get; init; } = string.Empty;

    public string? PreviewImagePath { get; init; }
}

public sealed record OcrJobResult
{
    public string JobId { get; init; } = string.Empty;

    public string SourcePath { get; init; } = string.Empty;

    public string SourceName => Path.GetFileName(SourcePath);

    public string? OutputPath { get; init; }

    public OcrWorkflowMode WorkflowMode { get; init; }

    public OcrJobStatus Status { get; init; }

    public int Pages { get; init; }

    public string Markdown { get; init; } = string.Empty;

    public string? Error { get; init; }

    public IReadOnlyList<string> Warnings { get; init; } = [];
}

public sealed record OcrWorkerEvent
{
    public string Event { get; init; } = string.Empty;

    public string JobId { get; init; } = string.Empty;

    public OcrJobStatus? Status { get; init; }

    public string? Stage { get; init; }

    public double? Percent { get; init; }

    public int? PageNumber { get; init; }

    public int? TotalPages { get; init; }

    public int? RenderedPages { get; init; }

    public int? RecognizedPages { get; init; }

    public string? Message { get; init; }

    public string? Code { get; init; }

    public string? Delta { get; init; }

    public string? PageMarkdown { get; init; }

    public string? PreviewImagePath { get; init; }

    public string? OutputMarkdownPath { get; init; }

    public int? Pages { get; init; }

    public bool? Recoverable { get; init; }
}

public sealed record OcrHistoryItem
{
    public string Id { get; init; } = Guid.NewGuid().ToString("N");

    public DateTimeOffset CreatedAt { get; init; } = DateTimeOffset.Now;

    public DateTimeOffset UpdatedAt { get; init; } = DateTimeOffset.Now;

    public string SourcePath { get; init; } = string.Empty;

    public string SourceName { get; init; } = string.Empty;

    public OcrWorkflowMode WorkflowMode { get; init; }

    public string? ModelProfileId { get; init; }

    public string? ModelFile { get; init; }

    public RuntimeProfile RuntimeProfile { get; init; }

    public string? EffectiveRuntimeLabel { get; init; }

    public int Pages { get; init; }

    public OcrJobStatus Status { get; init; }

    public string? OutputPath { get; init; }

    public string? Error { get; init; }

    public IReadOnlyList<string> Warnings { get; init; } = [];

    public OcrJobOptions RetryOptions { get; init; } = new();
}
