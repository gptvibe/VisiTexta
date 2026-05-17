using App.Models;

namespace App.Core.Output;

public static class OutputPathHelper
{
    public static string GetExtension(OcrExportFormat format)
    {
        return format switch
        {
            OcrExportFormat.Markdown => "md",
            OcrExportFormat.Text => "txt",
            OcrExportFormat.Pdf => "pdf",
            OcrExportFormat.Json => "json",
            OcrExportFormat.Csv => "csv",
            _ => "txt"
        };
    }

    public static string GetDuplicateSafePath(string sourcePath, OcrExportFormat format)
    {
        if (string.IsNullOrWhiteSpace(sourcePath))
        {
            throw new ArgumentException("Source path is required.", nameof(sourcePath));
        }

        var directory = Path.GetDirectoryName(sourcePath);
        if (string.IsNullOrWhiteSpace(directory))
        {
            throw new ArgumentException("Source path must include a directory.", nameof(sourcePath));
        }

        var stem = Path.GetFileNameWithoutExtension(sourcePath);
        if (string.IsNullOrWhiteSpace(stem))
        {
            throw new ArgumentException("Source path must include a file name.", nameof(sourcePath));
        }

        var extension = GetExtension(format);
        var first = Path.Combine(directory, $"{stem}.ocr.{extension}");
        if (!File.Exists(first))
        {
            return first;
        }

        for (var attempt = 2; attempt < 10_000; attempt++)
        {
            var candidate = Path.Combine(directory, $"{stem} (ocr {attempt}).{extension}");
            if (!File.Exists(candidate))
            {
                return candidate;
            }
        }

        throw new IOException("Could not find a free output file name.");
    }
}
