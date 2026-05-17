using System.Text;
using System.Text.Json;
using App.Core.Contracts;
using App.Core.Formatting;
using App.Core.Output;
using App.Core.Workflow;
using App.Models;

namespace App.Services.Export;

public sealed class ExportService : IExportService
{
    private readonly IStoragePathService _paths;

    public ExportService(IStoragePathService paths)
    {
        _paths = paths;
    }

    public string GetDefaultExtension(OcrExportFormat format)
    {
        return OutputPathHelper.GetExtension(format);
    }

    public IReadOnlyList<OcrExportFormat> GetAvailableFormats(OcrWorkflowMode mode)
    {
        return WorkflowDefinitions.FormatsFor(mode);
    }

    public async Task<string> SavePrimaryMarkdownAsync(string sourcePath, string markdown, CancellationToken cancellationToken = default)
    {
        var outputPath = _paths.GetDuplicateSafeOutputPath(sourcePath, OcrExportFormat.Markdown);
        await WriteAtomicAsync(outputPath, markdown, cancellationToken);
        return outputPath;
    }

    public async Task ExportAsync(
        OcrWorkflowMode mode,
        OcrExportFormat format,
        string destinationPath,
        string markdown,
        CancellationToken cancellationToken = default)
    {
        var content = BuildExportContent(mode, format, markdown);
        await WriteAtomicAsync(destinationPath, content, cancellationToken);
    }

    private static string BuildExportContent(OcrWorkflowMode mode, OcrExportFormat format, string markdown)
    {
        return format switch
        {
            OcrExportFormat.Markdown => markdown,
            OcrExportFormat.Text => MarkdownFormatter.ToPlainText(markdown),
            OcrExportFormat.Csv => mode == OcrWorkflowMode.Notes ? NotesToCsv(markdown) : ExtractToCsv(markdown),
            OcrExportFormat.Json => ExtractToJson(markdown),
            OcrExportFormat.Pdf => BuildSimpleTextPdf(MarkdownFormatter.ToPlainText(markdown)),
            _ => markdown
        };
    }

    private static async Task WriteAtomicAsync(string path, string content, CancellationToken cancellationToken)
    {
        var directory = Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var tempPath = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        await File.WriteAllTextAsync(tempPath, content, Encoding.UTF8, cancellationToken);
        if (File.Exists(path))
        {
            File.Replace(tempPath, path, null);
        }
        else
        {
            File.Move(tempPath, path);
        }
    }

    private static string NotesToCsv(string markdown)
    {
        var rows = new List<string[]> { new[] { "Front", "Back", "Deck", "Tags" } };
        var title = markdown.Split('\n').FirstOrDefault(line => line.StartsWith("# ", StringComparison.Ordinal))?.TrimStart('#', ' ').Trim();
        title = string.IsNullOrWhiteSpace(title) ? "VisiTexta Notes" : title;
        foreach (var line in markdown.Split('\n').Where(line => line.TrimStart().StartsWith("- ", StringComparison.Ordinal)))
        {
            var text = StripSource(line.Trim()[2..]);
            if (!string.IsNullOrWhiteSpace(text))
            {
                rows.Add(new[] { $"Review: {title}", text, title, "visitexta" });
            }
        }

        if (rows.Count == 1)
        {
            rows.Add(new[] { $"Summary of {title}", MarkdownFormatter.ToPlainText(markdown), title, "summary" });
        }

        return string.Join(Environment.NewLine, rows.Select(row => string.Join(",", row.Select(EscapeCsv))));
    }

    private static string ExtractToCsv(string markdown)
    {
        var metadata = TryReadExtractMetadata(markdown);
        if (metadata is not null && metadata.RootElement.TryGetProperty("csv_export", out var csvExport))
        {
            var rows = new List<string>();
            if (csvExport.TryGetProperty("columns", out var columns))
            {
                rows.Add(string.Join(",", columns.EnumerateArray().Select(column => EscapeCsv(column.GetString() ?? string.Empty))));
            }

            if (csvExport.TryGetProperty("rows", out var dataRows))
            {
                rows.AddRange(dataRows.EnumerateArray().Select(row => string.Join(",", row.EnumerateArray().Select(cell => EscapeCsv(cell.GetString() ?? string.Empty)))));
            }

            if (rows.Count > 0)
            {
                return string.Join(Environment.NewLine, rows);
            }
        }

        return NotesToCsv(markdown);
    }

    private static string ExtractToJson(string markdown)
    {
        var metadata = TryReadExtractMetadata(markdown);
        if (metadata is not null)
        {
            return JsonSerializer.Serialize(metadata.RootElement, new JsonSerializerOptions { WriteIndented = true });
        }

        return JsonSerializer.Serialize(new { markdown, plain_text = MarkdownFormatter.ToPlainText(markdown) }, new JsonSerializerOptions { WriteIndented = true });
    }

    private static JsonDocument? TryReadExtractMetadata(string markdown)
    {
        const string marker = "<!-- visitexta-extract:";
        var start = markdown.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
        if (start < 0)
        {
            return null;
        }

        start += marker.Length;
        var end = markdown.IndexOf("-->", start, StringComparison.OrdinalIgnoreCase);
        if (end < 0)
        {
            return null;
        }

        var json = markdown[start..end].Trim();
        try
        {
            return JsonDocument.Parse(json);
        }
        catch
        {
            return null;
        }
    }

    private static string StripSource(string text)
    {
        var index = text.IndexOf("_(Source:", StringComparison.OrdinalIgnoreCase);
        return index >= 0 ? text[..index].Trim() : text.Trim();
    }

    private static string EscapeCsv(string value)
    {
        return "\"" + value.Replace("\"", "\"\"", StringComparison.Ordinal) + "\"";
    }

    private static string BuildSimpleTextPdf(string text)
    {
        // v1 scaffold: keep the export service contract in place. The packaging milestone
        // replaces this with a proper searchable PDF writer.
        return text;
    }
}
