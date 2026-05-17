using System.Text.Json;
using App.Models;

namespace App.Inference.Worker;

public static class OcrWorkerProtocol
{
    public static OcrWorkerEvent ParseEvent(string line)
    {
        using var document = JsonDocument.Parse(line);
        var root = document.RootElement;
        var eventName = GetString(root, "event") ?? string.Empty;
        return new OcrWorkerEvent
        {
            Event = eventName,
            JobId = GetString(root, "job_id") ?? string.Empty,
            Status = ParseStatus(GetString(root, "status")),
            Stage = GetString(root, "stage"),
            Percent = GetDouble(root, "percent"),
            PageNumber = GetInt(root, "page_number"),
            TotalPages = GetInt(root, "total_pages"),
            RenderedPages = GetInt(root, "rendered_pages"),
            RecognizedPages = GetInt(root, "recognized_pages"),
            Message = GetString(root, "message"),
            Code = GetString(root, "code"),
            Delta = GetString(root, "delta"),
            PageMarkdown = GetString(root, "page_markdown"),
            PreviewImagePath = GetString(root, "preview_image_path"),
            OutputMarkdownPath = GetString(root, "output_markdown_path"),
            Pages = GetInt(root, "pages"),
            Recoverable = GetBool(root, "recoverable")
        };
    }

    public static string BuildCancelCommand(string jobId)
    {
        return JsonSerializer.Serialize(new Dictionary<string, object?>
        {
            ["command"] = "cancel",
            ["job_id"] = jobId
        });
    }

    public static string BuildShutdownCommand()
    {
        return "{\"command\":\"shutdown\"}";
    }

    private static OcrJobStatus? ParseStatus(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        return Enum.TryParse<OcrJobStatus>(value.Replace("_", string.Empty, StringComparison.Ordinal), ignoreCase: true, out var status)
            ? status
            : null;
    }

    private static string? GetString(JsonElement element, string name)
    {
        return element.TryGetProperty(name, out var property) ? property.GetString() : null;
    }

    private static int? GetInt(JsonElement element, string name)
    {
        return element.TryGetProperty(name, out var property) && property.TryGetInt32(out var value) ? value : null;
    }

    private static double? GetDouble(JsonElement element, string name)
    {
        return element.TryGetProperty(name, out var property) && property.TryGetDouble(out var value) ? value : null;
    }

    private static bool? GetBool(JsonElement element, string name)
    {
        return element.TryGetProperty(name, out var property) && (property.ValueKind is JsonValueKind.True or JsonValueKind.False) ? property.GetBoolean() : null;
    }
}
