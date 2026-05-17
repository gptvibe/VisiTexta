using System.Text.RegularExpressions;

namespace App.Core.Workflow;

public static partial class NotesProcessor
{
    public static string BuildNotes(IReadOnlyList<string> pageMarkdown, bool studyBoost, string? customOverride)
    {
        var lines = pageMarkdown
            .SelectMany((page, index) => PageContent(page)
                .Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(text => new PageLine(index + 1, text)))
            .Where(line => !string.IsNullOrWhiteSpace(line.Text))
            .ToList();

        if (lines.Count == 0)
        {
            return string.Empty;
        }

        var title = lines.FirstOrDefault(line => line.Text.Length <= 120 && !line.Text.EndsWith(".", StringComparison.Ordinal))?.Text
            ?? "Study Notes";
        var sections = new List<string> { $"# {title}" };

        if (!string.IsNullOrWhiteSpace(customOverride))
        {
            sections.Add($"_Advanced override: {customOverride.Replace('\n', ' ')}_");
        }

        var headings = lines
            .Where(line => line.Text != title && line.Text.Length <= 80 && !line.Text.EndsWith(".", StringComparison.Ordinal))
            .DistinctBy(line => line.Text, StringComparer.OrdinalIgnoreCase)
            .Take(8)
            .ToList();
        if (headings.Count > 0)
        {
            sections.Add("## Headings");
            sections.AddRange(headings.Select(line => $"- {line.Text} _(Source: p. {line.PageNumber})_"));
        }

        var paragraphs = pageMarkdown
            .SelectMany((page, index) => PageContent(page)
                .Split("\n\n", StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(text => new PageLine(index + 1, text.Replace('\n', ' '))))
            .Where(line => line.Text.Count(char.IsLetterOrDigit) >= 20)
            .Take(8)
            .ToList();

        if (paragraphs.Count > 0)
        {
            sections.Add("## Key Points");
            sections.AddRange(paragraphs.Select(line => $"- {Truncate(line.Text, 180)} _(Source: p. {line.PageNumber})_"));
        }

        var glossary = lines
            .Select(line => (line, Match: GlossaryRegex().Match(line.Text)))
            .Where(item => item.Match.Success)
            .Take(8)
            .Select(item => $"- **{item.Match.Groups["term"].Value.Trim()}**: {Truncate(item.Match.Groups["definition"].Value.Trim(), 140)} _(Source: p. {item.line.PageNumber})_")
            .ToList();
        if (glossary.Count > 0)
        {
            sections.Add("## Glossary");
            sections.AddRange(glossary);
        }

        sections.Add("## Review Questions");
        sections.Add($"- What are the most important ideas in {title}? _(Source: p. 1)_");
        if (studyBoost)
        {
            sections.Add("## Study Boost");
            sections.Add("- Re-read the source pages, then recall the headings without looking.");
        }

        return string.Join("\n\n", sections).Trim();
    }

    private static string PageContent(string pageMarkdown)
    {
        return string.Join('\n', pageMarkdown
            .Split('\n')
            .Where(line => !line.TrimStart().StartsWith("<!--", StringComparison.Ordinal))
            .Where(line => !PageHeadingRegex().IsMatch(line.Trim())));
    }

    private static string Truncate(string value, int maxLength)
    {
        return value.Length <= maxLength ? value : value[..maxLength].TrimEnd() + "...";
    }

    private sealed record PageLine(int PageNumber, string Text);

    [GeneratedRegex(@"^##\s+Page\s+\d+\s*$", RegexOptions.IgnoreCase)]
    private static partial Regex PageHeadingRegex();

    [GeneratedRegex(@"^(?<term>[A-Za-z][A-Za-z0-9 ()/\-]{1,40}):\s+(?<definition>.+)$")]
    private static partial Regex GlossaryRegex();
}
