using System.Text.RegularExpressions;

namespace App.Core.Formatting;

public static partial class MarkdownFormatter
{
    public static string CleanMarkdown(string input)
    {
        if (string.IsNullOrWhiteSpace(input))
        {
            return string.Empty;
        }

        var normalized = input.Replace("\r\n", "\n", StringComparison.Ordinal).Replace('\r', '\n');
        normalized = HyphenLineBreakRegex().Replace(normalized, string.Empty);
        normalized = QuoteRegex().Replace(normalized, "\"");
        normalized = BlankLineRegex().Replace(normalized, "\n\n");
        return normalized.Trim();
    }

    public static string ToPlainText(string markdown)
    {
        if (string.IsNullOrWhiteSpace(markdown))
        {
            return string.Empty;
        }

        var text = CodeFenceRegex().Replace(markdown, string.Empty);
        text = HeadingRegex().Replace(text, string.Empty);
        text = BulletRegex().Replace(text, string.Empty);
        text = LinkRegex().Replace(text, "$1");
        text = MarkdownMarksRegex().Replace(text, string.Empty);
        text = BlankLineRegex().Replace(text, "\n\n");
        return text.Trim();
    }

    [GeneratedRegex(@"-\n(?=\p{L})")]
    private static partial Regex HyphenLineBreakRegex();

    [GeneratedRegex("[\u201c\u201d]")]
    private static partial Regex QuoteRegex();

    [GeneratedRegex(@"\n{3,}")]
    private static partial Regex BlankLineRegex();

    [GeneratedRegex(@"```[\s\S]*?```")]
    private static partial Regex CodeFenceRegex();

    [GeneratedRegex(@"^#{1,6}\s+", RegexOptions.Multiline)]
    private static partial Regex HeadingRegex();

    [GeneratedRegex(@"^\s*(?:[-*+]|\d+\.)\s+", RegexOptions.Multiline)]
    private static partial Regex BulletRegex();

    [GeneratedRegex(@"\[(.*?)\]\(.*?\)")]
    private static partial Regex LinkRegex();

    [GeneratedRegex(@"[*_`>#-]")]
    private static partial Regex MarkdownMarksRegex();
}
