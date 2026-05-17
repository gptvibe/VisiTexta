namespace App.Models;

public enum AppThemePreference
{
    System,
    Light,
    Dark
}

public enum StorageMode
{
    Portable,
    Installed
}

public enum RuntimeProfile
{
    Auto,
    CpuCompatible,
    AcceleratedIfAvailable
}

public enum RuntimeBackend
{
    CpuCompatible,
    Cuda,
    DirectMl,
    Vulkan,
    GenericAccelerated
}

public enum OcrWorkflowMode
{
    ExactOcr,
    Notes,
    Extract
}

public enum OcrExportFormat
{
    Markdown,
    Text,
    Pdf,
    Json,
    Csv
}

public enum OcrJobStatus
{
    Queued,
    Rendering,
    Ocr,
    Formatting,
    Writing,
    Done,
    Failed,
    Canceled
}

public enum ModelSupportTier
{
    Recommended,
    Tested,
    Legacy,
    Experimental
}

public enum ModelInstallSource
{
    Registry,
    Heuristic,
    Legacy,
    Custom
}

public enum ModelDownloadStatus
{
    NotDownloaded,
    Partial,
    Downloading,
    Verifying,
    Downloaded,
    Invalid,
    Error
}
