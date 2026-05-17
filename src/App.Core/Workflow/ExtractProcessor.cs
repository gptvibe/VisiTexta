using System.Text.Json;
using System.Text.RegularExpressions;

namespace App.Core.Workflow;

public static partial class ExtractProcessor
{
    public static string BuildExtract(IReadOnlyList<string> pageMarkdown, string templateId, string? customOverride)
    {
        var lines = pageMarkdown
            .SelectMany((page, index) => PageContent(page)
                .Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(text => new { Page = index + 1, Text = text }))
            .ToList();

        if (lines.Count == 0)
        {
            return string.Empty;
        }

        var fields = new List<Dictionary<string, object?>>
        {
            Field("document_type", "Document Type", GuessDocumentType(lines.Select(line => line.Text)), [1], true),
            Field("first_heading", "First Heading", lines.First().Text, [lines.First().Page], false)
        };

        var amounts = lines
            .Where(line => AmountRegex().IsMatch(line.Text))
            .Take(10)
            .Select(line => new Dictionary<string, object?>
            {
                ["cells"] = new[]
                {
                    new Dictionary<string, string> { ["column"] = "text", ["value"] = line.Text },
                    new Dictionary<string, string> { ["column"] = "page", ["value"] = line.Page.ToString() }
                },
                ["source_pages"] = new[] { line.Page },
                ["needs_verification"] = true,
                ["verification_note"] = "Confirm OCR amount manually."
            })
            .ToList();

        var structured = new Dictionary<string, object?>
        {
            ["template_id"] = templateId,
            ["template_label"] = TemplateLabel(templateId),
            ["source_page_count"] = pageMarkdown.Count,
            ["summary"] = lines.Take(5).Select(line => new { text = line.Text, source_pages = new[] { line.Page } }).ToArray(),
            ["fields"] = fields,
            ["rows"] = amounts,
            ["verification"] = new[] { new { text = "Review OCR against the source image before using extracted values.", source_pages = Enumerable.Range(1, Math.Max(1, pageMarkdown.Count)).ToArray() } },
            ["csv_export"] = new
            {
                mode = amounts.Count > 0 ? "rows" : "fields",
                columns = new[] { "field", "value", "page" },
                rows = fields.Select(field => new[] { field["label"]?.ToString() ?? string.Empty, field["value"]?.ToString() ?? string.Empty, string.Join(";", (int[])field["source_pages"]!) }).ToArray()
            }
        };

        var metadata = JsonSerializer.Serialize(structured, new JsonSerializerOptions { WriteIndented = false });
        var sections = new List<string>
        {
            $"<!-- visitexta-extract: {metadata} -->",
            $"# {TemplateLabel(templateId)}",
            "## Summary",
        };
        sections.AddRange(lines.Take(5).Select(line => $"- {line.Text} _(Source: p. {line.Page})_"));
        sections.Add("## Fields");
        sections.AddRange(fields.Select(field => $"- **{field["label"]}**: {field["value"] ?? "Needs review"} _(Source: p. {string.Join(", ", (int[])field["source_pages"]!)})_"));
        sections.Add("## Uncertainty / Verification");
        sections.Add("- Review OCR against the source image before using extracted values.");

        if (!string.IsNullOrWhiteSpace(customOverride))
        {
            sections.Add($"_Advanced override: {customOverride.Replace('\n', ' ')}_");
        }

        return string.Join("\n\n", sections).Trim();
    }

    private static Dictionary<string, object?> Field(string key, string label, string? value, int[] pages, bool verify)
    {
        return new()
        {
            ["key"] = key,
            ["label"] = label,
            ["value"] = value,
            ["source_pages"] = pages,
            ["needs_verification"] = verify,
            ["verification_note"] = verify ? "Confirm manually." : null
        };
    }

    private static string PageContent(string pageMarkdown)
    {
        return string.Join('\n', pageMarkdown
            .Split('\n')
            .Where(line => !line.TrimStart().StartsWith("<!--", StringComparison.Ordinal))
            .Where(line => !PageHeadingRegex().IsMatch(line.Trim())));
    }

    private static string GuessDocumentType(IEnumerable<string> lines)
    {
        var combined = string.Join(' ', lines).ToLowerInvariant();
        if (combined.Contains("receipt", StringComparison.Ordinal)) return "Receipt";
        if (combined.Contains("invoice", StringComparison.Ordinal)) return "Invoice";
        if (combined.Contains("agreement", StringComparison.Ordinal) || combined.Contains("contract", StringComparison.Ordinal)) return "Contract";
        return "Document";
    }

    private static string TemplateLabel(string templateId)
    {
        return templateId switch
        {
            "table_to_csv" => "Table to CSV",
            "meeting_whiteboard" => "Meeting Photo / Whiteboard",
            "contract_key_points" => "Contract Key Points",
            _ => "Invoice / Receipt"
        };
    }

    [GeneratedRegex(@"^##\s+Page\s+\d+\s*$", RegexOptions.IgnoreCase)]
    private static partial Regex PageHeadingRegex();

    [GeneratedRegex(@"(?:[$€£]\s*)?\d+(?:[,.]\d{2,})")]
    private static partial Regex AmountRegex();
}
