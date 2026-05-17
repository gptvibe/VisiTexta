using System.Diagnostics;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using App.Core.Formatting;
using App.Core.Workflow;
using App.Models;

var options = new JsonSerializerOptions(JsonSerializerDefaults.Web)
{
    PropertyNameCaseInsensitive = true,
    Converters = { new JsonStringEnumConverter() }
};

try
{
    string? line;
    while ((line = await Console.In.ReadLineAsync()) is not null)
    {
        if (string.IsNullOrWhiteSpace(line))
        {
            continue;
        }

        using var document = JsonDocument.Parse(line);
        var command = document.RootElement.TryGetProperty("command", out var commandElement)
            ? commandElement.GetString()
            : null;

        if (command == "shutdown")
        {
            return;
        }

        if (command == "cancel")
        {
            Emit(new { @event = "warning", job_id = GetString(document.RootElement, "job_id"), code = "canceled", message = "Cancellation requested." });
            return;
        }

        if (command != "ocr_job")
        {
            Emit(new { @event = "error", job_id = GetString(document.RootElement, "job_id") ?? string.Empty, code = "unknown_command", message = "Unsupported worker command.", recoverable = false });
            continue;
        }

        var job = OcrWorkerJob.FromJson(document.RootElement, options);
        await RunJobAsync(job);
    }
}
catch (Exception ex)
{
    Emit(new { @event = "error", job_id = string.Empty, code = "worker_crash", message = ex.Message, recoverable = false });
    Environment.ExitCode = 1;
}

static async Task RunJobAsync(OcrWorkerJob job)
{
    Emit(new { @event = "job_started", job_id = job.JobId, source_path = job.SourcePath, message = "Preparing document" });

    if (!File.Exists(job.SourcePath))
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "source_missing", message = "The selected source file could not be found.", recoverable = false });
        return;
    }

    Directory.CreateDirectory(Path.GetDirectoryName(job.OutputMarkdownPath)!);
    Directory.CreateDirectory(job.TempDirectory);
    Directory.CreateDirectory(job.LogDirectory);

    var totalPages = job.SourcePath.EndsWith(".pdf", StringComparison.OrdinalIgnoreCase) ? 1 : 1;
    Emit(new { @event = "progress", job_id = job.JobId, stage = "rendering", percent = 5, page_number = 1, total_pages = totalPages, rendered_pages = 0, recognized_pages = 0, message = "Preparing page 1 of 1" });
    Emit(new { @event = "page_started", job_id = job.JobId, page_number = 1, total_pages = totalPages, message = "Reading page 1 of 1" });

    var rawText = await RecognizeAsync(job);
    foreach (var delta in Chunk(rawText, 180))
    {
        Emit(new { @event = "text_delta", job_id = job.JobId, page_number = 1, delta });
        await Task.Delay(12);
    }

    var pageMarkdown = $"## Page 1\n\n{rawText.Trim()}\n";
    Emit(new { @event = "page_done", job_id = job.JobId, page_number = 1, total_pages = totalPages, page_markdown = pageMarkdown });
    Emit(new { @event = "progress", job_id = job.JobId, stage = "formatting", percent = 80, page_number = 1, total_pages = totalPages, rendered_pages = 1, recognized_pages = 1, message = "Formatting OCR output" });

    var finalMarkdown = job.Mode switch
    {
        OcrWorkflowMode.Notes => NotesProcessor.BuildNotes([pageMarkdown], job.StudyBoost, job.PromptOverride),
        OcrWorkflowMode.Extract => ExtractProcessor.BuildExtract([pageMarkdown], job.ExtractTemplateId, job.PromptOverride),
        _ => MarkdownFormatter.CleanMarkdown(pageMarkdown)
    };

    if (string.IsNullOrWhiteSpace(finalMarkdown))
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "empty_ocr", message = "OCR produced no text.", recoverable = true });
        return;
    }

    Emit(new { @event = "progress", job_id = job.JobId, stage = "writing", percent = 92, page_number = 1, total_pages = totalPages, rendered_pages = 1, recognized_pages = 1, message = "Saving Markdown" });
    await File.WriteAllTextAsync(job.OutputMarkdownPath, finalMarkdown, Encoding.UTF8);
    Emit(new { @event = "done", job_id = job.JobId, status = "done", pages = totalPages, output_markdown_path = job.OutputMarkdownPath, elapsed_ms = 0, warnings = Array.Empty<string>() });
}

static async Task<string> RecognizeAsync(OcrWorkerJob job)
{
    if (job.SourcePath.EndsWith(".pdf", StringComparison.OrdinalIgnoreCase))
    {
        Emit(new { @event = "warning", job_id = job.JobId, code = "pdfium_pending", message = "PDFium rendering is scaffolded for native v1, but this worker build has not yet enabled page rendering." });
        return $"Local OCR worker accepted PDF '{Path.GetFileName(job.SourcePath)}'. PDFium rendering is the next implementation slice.";
    }

    var cliPath = job.FallbackCliPaths.Concat(job.PreferredServerPaths)
        .FirstOrDefault(path => File.Exists(path) && Path.GetFileName(path).StartsWith("llama", StringComparison.OrdinalIgnoreCase));
    if (!string.IsNullOrWhiteSpace(cliPath) && File.Exists(job.ModelPath ?? string.Empty))
    {
        try
        {
            var text = await RunLlamaCliAsync(cliPath, job);
            if (!string.IsNullOrWhiteSpace(text))
            {
                return text;
            }
        }
        catch (Exception ex)
        {
            Emit(new { @event = "warning", job_id = job.JobId, code = "runtime_fallback", message = "Local llama runtime failed: " + ex.Message });
        }
    }

    Emit(new { @event = "warning", job_id = job.JobId, code = "runtime_pending", message = "No ready local OCR runtime/model was found. Returning protocol placeholder output." });
    return $"Local OCR worker accepted image '{Path.GetFileName(job.SourcePath)}'. Download an OCR model and add llama runtime files under bin to enable full local OCR.";
}

static async Task<string> RunLlamaCliAsync(string cliPath, OcrWorkerJob job)
{
    var prompt = string.IsNullOrWhiteSpace(job.PromptOverride)
        ? "Transcribe the visible document text as markdown."
        : job.PromptOverride;
    using var process = new Process();
    process.StartInfo = new ProcessStartInfo
    {
        FileName = cliPath,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        UseShellExecute = false,
        CreateNoWindow = true
    };
    process.StartInfo.ArgumentList.Add("-m");
    process.StartInfo.ArgumentList.Add(job.ModelPath!);
    process.StartInfo.ArgumentList.Add("--image");
    process.StartInfo.ArgumentList.Add(job.SourcePath);
    process.StartInfo.ArgumentList.Add("-p");
    process.StartInfo.ArgumentList.Add(prompt);
    process.StartInfo.ArgumentList.Add("-n");
    process.StartInfo.ArgumentList.Add("2048");
    process.StartInfo.ArgumentList.Add("--temp");
    process.StartInfo.ArgumentList.Add("0");
    process.StartInfo.ArgumentList.Add("--ctx-size");
    process.StartInfo.ArgumentList.Add("8192");
    process.StartInfo.ArgumentList.Add("--threads");
    process.StartInfo.ArgumentList.Add(Math.Max(1, job.Threads).ToString());
    if (!string.IsNullOrWhiteSpace(job.MmprojPath))
    {
        process.StartInfo.ArgumentList.Add("--mmproj");
        process.StartInfo.ArgumentList.Add(job.MmprojPath);
    }

    process.Start();
    var stdoutTask = process.StandardOutput.ReadToEndAsync();
    var stderrTask = process.StandardError.ReadToEndAsync();
    await process.WaitForExitAsync();
    var stdout = await stdoutTask;
    var stderr = await stderrTask;
    if (process.ExitCode != 0)
    {
        throw new InvalidOperationException(string.IsNullOrWhiteSpace(stderr) ? "llama runtime exited with an error." : stderr.Trim());
    }

    return Sanitize(stdout);
}

static string Sanitize(string text)
{
    var lines = text.Replace("\r\n", "\n", StringComparison.Ordinal).Replace('\r', '\n').Split('\n');
    var kept = lines
        .Select(line => line.TrimEnd())
        .Where(line => !string.IsNullOrWhiteSpace(line))
        .Where(line => !line.StartsWith("llama_", StringComparison.OrdinalIgnoreCase))
        .Where(line => !line.StartsWith("build:", StringComparison.OrdinalIgnoreCase))
        .Where(line => !line.StartsWith("model:", StringComparison.OrdinalIgnoreCase));
    return string.Join('\n', kept).Trim();
}

static IEnumerable<string> Chunk(string text, int size)
{
    for (var index = 0; index < text.Length; index += size)
    {
        yield return text.Substring(index, Math.Min(size, text.Length - index));
    }
}

static void Emit(object payload)
{
    Console.WriteLine(JsonSerializer.Serialize(payload, new JsonSerializerOptions(JsonSerializerDefaults.Web) { Converters = { new JsonStringEnumConverter() } }));
    Console.Out.Flush();
}

static string? GetString(JsonElement root, string property)
{
    return root.TryGetProperty(property, out var value) ? value.GetString() : null;
}

internal sealed record OcrWorkerJob
{
    public string JobId { get; init; } = string.Empty;

    public string SourcePath { get; init; } = string.Empty;

    public string OutputMarkdownPath { get; init; } = string.Empty;

    public string TempDirectory { get; init; } = string.Empty;

    public string LogDirectory { get; init; } = string.Empty;

    public OcrWorkflowMode Mode { get; init; } = OcrWorkflowMode.ExactOcr;

    public string? PromptOverride { get; init; }

    public bool StudyBoost { get; init; }

    public string ExtractTemplateId { get; init; } = "invoice_receipt";

    public int Threads { get; init; } = Math.Max(1, Environment.ProcessorCount - 1);

    public string? ModelPath { get; init; }

    public string? MmprojPath { get; init; }

    public IReadOnlyList<string> PreferredServerPaths { get; init; } = [];

    public IReadOnlyList<string> FallbackCliPaths { get; init; } = [];

    public static OcrWorkerJob FromJson(JsonElement root, JsonSerializerOptions options)
    {
        var mode = OcrWorkflowMode.ExactOcr;
        if (root.TryGetProperty("mode", out var modeElement))
        {
            if (modeElement.ValueKind == JsonValueKind.String)
            {
                Enum.TryParse(modeElement.GetString(), ignoreCase: true, out mode);
            }
            else
            {
                mode = modeElement.Deserialize<OcrWorkflowMode>(options);
            }
        }

        var model = root.TryGetProperty("model", out var modelElement) ? modelElement : default;
        var runtime = root.TryGetProperty("runtime", out var runtimeElement) ? runtimeElement : default;
        return new OcrWorkerJob
        {
            JobId = GetRequired(root, "job_id"),
            SourcePath = GetRequired(root, "source_path"),
            OutputMarkdownPath = GetRequired(root, "output_markdown_path"),
            TempDirectory = GetRequired(root, "temp_dir"),
            LogDirectory = GetRequired(root, "log_dir"),
            Mode = mode,
            PromptOverride = GetString(root, "prompt_override"),
            StudyBoost = root.TryGetProperty("study_boost", out var study) && study.GetBoolean(),
            ExtractTemplateId = GetString(root, "extract_template_id") ?? "invoice_receipt",
            ModelPath = model.ValueKind == JsonValueKind.Object ? GetString(model, "model_path") : null,
            MmprojPath = model.ValueKind == JsonValueKind.Object ? GetString(model, "mmproj_path") : null,
            PreferredServerPaths = runtime.ValueKind == JsonValueKind.Object ? GetStringArray(runtime, "preferred_server_paths") : [],
            FallbackCliPaths = runtime.ValueKind == JsonValueKind.Object ? GetStringArray(runtime, "fallback_cli_paths") : []
        };
    }

    private static string GetRequired(JsonElement root, string property)
    {
        return root.TryGetProperty(property, out var value) && !string.IsNullOrWhiteSpace(value.GetString())
            ? value.GetString()!
            : throw new InvalidOperationException($"Missing required job property: {property}");
    }

    private static string? GetString(JsonElement root, string property)
    {
        return root.TryGetProperty(property, out var value) ? value.GetString() : null;
    }

    private static IReadOnlyList<string> GetStringArray(JsonElement root, string property)
    {
        if (!root.TryGetProperty(property, out var value) || value.ValueKind != JsonValueKind.Array)
        {
            return [];
        }

        return value.EnumerateArray().Select(item => item.GetString()).Where(item => !string.IsNullOrWhiteSpace(item)).Select(item => item!).ToList();
    }
}
