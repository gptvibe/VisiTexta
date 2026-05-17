namespace App.Models;

public sealed record StorageInfo
{
    public StorageMode Mode { get; init; }

    public string RootPath { get; init; } = string.Empty;

    public string SettingsPath { get; init; } = string.Empty;

    public string HistoryPath { get; init; } = string.Empty;

    public string ModelsPath { get; init; } = string.Empty;

    public string DownloadsPath { get; init; } = string.Empty;

    public string TempPath { get; init; } = string.Empty;

    public string LogsPath { get; init; } = string.Empty;

    public string DiagnosticsPath { get; init; } = string.Empty;

    public string PastedInputsPath { get; init; } = string.Empty;

    public string OutputDescription { get; init; } = "OCR outputs are written next to the source file with duplicate-safe names.";
}
