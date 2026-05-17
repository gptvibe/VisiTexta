using App.Models;

namespace App.Core.Contracts;

public interface IExportService
{
    string GetDefaultExtension(OcrExportFormat format);

    IReadOnlyList<OcrExportFormat> GetAvailableFormats(OcrWorkflowMode mode);

    Task<string> SavePrimaryMarkdownAsync(string sourcePath, string markdown, CancellationToken cancellationToken = default);

    Task ExportAsync(
        OcrWorkflowMode mode,
        OcrExportFormat format,
        string destinationPath,
        string markdown,
        CancellationToken cancellationToken = default);
}
