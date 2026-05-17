using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
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
    var stopwatch = Stopwatch.StartNew();
    Emit(new { @event = "job_started", job_id = job.JobId, source_path = job.SourcePath, message = "Preparing document" });

    if (!File.Exists(job.SourcePath))
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "source_missing", message = "The selected source file could not be found.", recoverable = false });
        return;
    }

    Directory.CreateDirectory(Path.GetDirectoryName(job.OutputMarkdownPath)!);
    Directory.CreateDirectory(job.TempDirectory);
    Directory.CreateDirectory(job.LogDirectory);

    var cliCandidates = ResolveLlamaCliCandidates(job).ToList();
    if (cliCandidates.Count == 0)
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "local_runtime_missing", message = "No local llama OCR runtime was found. Use the bundled portable release or add llama-mtmd-cli.exe under bin.", recoverable = true });
        return;
    }

    if (string.IsNullOrWhiteSpace(job.ModelPath) || !File.Exists(job.ModelPath))
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "model_missing", message = "No ready local OCR model was found. Download the recommended GLM-OCR model from the Models page.", recoverable = true });
        return;
    }

    if (string.IsNullOrWhiteSpace(job.MmprojPath) || !File.Exists(job.MmprojPath))
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "mmproj_missing", message = "The selected OCR model is missing its companion mmproj file. Redownload the model profile from the Models page.", recoverable = true });
        return;
    }

    IReadOnlyList<RenderedPage> pages;
    if (job.SourcePath.EndsWith(".pdf", StringComparison.OrdinalIgnoreCase))
    {
        var pdfiumPath = ResolvePdfiumPath(job);
        if (string.IsNullOrWhiteSpace(pdfiumPath))
        {
            Emit(new { @event = "error", job_id = job.JobId, code = "pdfium_missing", message = "PDF OCR needs pdfium.dll under bin. Use the bundled portable release or open Diagnostics for searched paths.", recoverable = true });
            return;
        }

        try
        {
            pages = PdfiumRenderer.Render(job, pdfiumPath, Emit);
        }
        catch (Exception ex)
        {
            Emit(new { @event = "error", job_id = job.JobId, code = "pdf_render_failed", message = "PDF rendering failed: " + ex.Message, recoverable = true });
            return;
        }
    }
    else if (IsSupportedImage(job.SourcePath))
    {
        pages = [new RenderedPage(1, 1, job.SourcePath, job.SourcePath)];
        Emit(new { @event = "progress", job_id = job.JobId, stage = "rendering", percent = 5, page_number = 1, total_pages = 1, rendered_pages = 1, recognized_pages = 0, message = "Prepared image input" });
    }
    else
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "unsupported_source", message = "VisiTexta Native supports PNG, JPG, JPEG, and PDF inputs.", recoverable = true });
        return;
    }

    if (pages.Count == 0)
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "empty_document", message = "No pages were found to OCR.", recoverable = true });
        return;
    }

    var totalPages = pages.Count;
    var pageMarkdown = new List<string>(totalPages);
    var recognizedPages = 0;
    foreach (var page in pages)
    {
        var pagePercent = 10 + (recognizedPages * 70.0 / totalPages);
        Emit(new { @event = "page_started", job_id = job.JobId, page_number = page.PageNumber, total_pages = totalPages, percent = pagePercent, message = $"Reading page {page.PageNumber} of {totalPages}" });
        Emit(new { @event = "progress", job_id = job.JobId, stage = "ocr", percent = pagePercent, page_number = page.PageNumber, total_pages = totalPages, rendered_pages = totalPages, recognized_pages = recognizedPages, message = $"Running local OCR on page {page.PageNumber} of {totalPages}" });

        string rawText;
        try
        {
            rawText = await RecognizeImageAsync(cliCandidates, job, page.ImagePath);
        }
        catch (Exception ex)
        {
            Emit(new { @event = "error", job_id = job.JobId, code = "runtime_failed", message = "Local OCR runtime failed: " + ex.Message, recoverable = true });
            return;
        }

        foreach (var delta in Chunk(rawText, 180))
        {
            Emit(new { @event = "text_delta", job_id = job.JobId, page_number = page.PageNumber, delta });
            await Task.Delay(12);
        }

        var markdown = $"## Page {page.PageNumber}\n\n{rawText.Trim()}\n";
        pageMarkdown.Add(markdown);
        recognizedPages++;
        var donePercent = 10 + (recognizedPages * 70.0 / totalPages);
        Emit(new { @event = "page_done", job_id = job.JobId, page_number = page.PageNumber, total_pages = totalPages, page_markdown = markdown, preview_image_path = page.PreviewImagePath });
        Emit(new { @event = "progress", job_id = job.JobId, stage = "ocr", percent = donePercent, page_number = page.PageNumber, total_pages = totalPages, rendered_pages = totalPages, recognized_pages = recognizedPages, message = $"Finished page {page.PageNumber} of {totalPages}" });
    }

    Emit(new { @event = "progress", job_id = job.JobId, stage = "formatting", percent = 84, page_number = totalPages, total_pages = totalPages, rendered_pages = totalPages, recognized_pages = recognizedPages, message = "Formatting OCR output" });

    var finalMarkdown = job.Mode switch
    {
        OcrWorkflowMode.Notes => NotesProcessor.BuildNotes(pageMarkdown, job.StudyBoost, job.PromptOverride),
        OcrWorkflowMode.Extract => ExtractProcessor.BuildExtract(pageMarkdown, job.ExtractTemplateId, job.PromptOverride),
        _ => MarkdownFormatter.CleanMarkdown(string.Join("\n\n", pageMarkdown))
    };

    if (string.IsNullOrWhiteSpace(finalMarkdown))
    {
        Emit(new { @event = "error", job_id = job.JobId, code = "empty_ocr", message = "OCR produced no text.", recoverable = true });
        return;
    }

    Emit(new { @event = "progress", job_id = job.JobId, stage = "writing", percent = 94, page_number = totalPages, total_pages = totalPages, rendered_pages = totalPages, recognized_pages = recognizedPages, message = "Saving Markdown" });
    await File.WriteAllTextAsync(job.OutputMarkdownPath, finalMarkdown, Encoding.UTF8);
    Emit(new { @event = "done", job_id = job.JobId, status = "done", pages = totalPages, output_markdown_path = job.OutputMarkdownPath, elapsed_ms = stopwatch.ElapsedMilliseconds, warnings = Array.Empty<string>() });
}

static async Task<string> RecognizeImageAsync(IReadOnlyList<string> cliCandidates, OcrWorkerJob job, string imagePath)
{
    var errors = new List<string>();
    foreach (var cliPath in cliCandidates)
    {
        try
        {
            var text = await RunLlamaCliAsync(cliPath, job, imagePath);
            if (!string.IsNullOrWhiteSpace(text))
            {
                return text;
            }

            errors.Add(Path.GetFileName(cliPath) + " produced no OCR text.");
        }
        catch (Exception ex)
        {
            errors.Add(Path.GetFileName(cliPath) + ": " + ex.Message);
            if (cliCandidates.Count > 1)
            {
                Emit(new { @event = "warning", job_id = job.JobId, code = "runtime_fallback", message = Path.GetFileName(cliPath) + " failed; trying the next local runtime." });
            }
        }
    }

    throw new InvalidOperationException(errors.Count == 0 ? "No local llama OCR runtime was available." : string.Join(" | ", errors));
}

static async Task<string> RunLlamaCliAsync(string cliPath, OcrWorkerJob job, string imagePath)
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
        CreateNoWindow = true,
        WorkingDirectory = Path.GetDirectoryName(cliPath) ?? Environment.CurrentDirectory
    };
    process.StartInfo.ArgumentList.Add("-m");
    process.StartInfo.ArgumentList.Add(job.ModelPath!);
    process.StartInfo.ArgumentList.Add("--image");
    process.StartInfo.ArgumentList.Add(imagePath);
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
        throw new InvalidOperationException(string.IsNullOrWhiteSpace(stderr) ? "llama runtime exited with an error." : CompactRuntimeError(stderr));
    }

    return Sanitize(stdout);
}

static IEnumerable<string> ResolveLlamaCliCandidates(OcrWorkerJob job)
{
    var preferredNames = new[]
    {
        "llama-mtmd-cli",
        "llama-llava-cli",
        "llama-qwen2vl-cli",
        "llama-minicpmv-cli",
        "llama-gemma3-cli",
        "llama-cli"
    };

    return job.FallbackCliPaths.Concat(job.PreferredServerPaths)
        .Where(path => !string.IsNullOrWhiteSpace(path) && File.Exists(path))
        .Where(path => preferredNames.Contains(Path.GetFileNameWithoutExtension(path), StringComparer.OrdinalIgnoreCase))
        .Distinct(StringComparer.OrdinalIgnoreCase)
        .OrderBy(path =>
        {
            var name = Path.GetFileNameWithoutExtension(path);
            var index = Array.FindIndex(preferredNames, item => item.Equals(name, StringComparison.OrdinalIgnoreCase));
            return index < 0 ? preferredNames.Length : index;
        })
        .ThenBy(path => path.Length);
}

static bool IsSupportedImage(string path)
{
    var extension = Path.GetExtension(path);
    return extension.Equals(".png", StringComparison.OrdinalIgnoreCase)
        || extension.Equals(".jpg", StringComparison.OrdinalIgnoreCase)
        || extension.Equals(".jpeg", StringComparison.OrdinalIgnoreCase);
}

static string? ResolvePdfiumPath(OcrWorkerJob job)
{
    if (!string.IsNullOrWhiteSpace(job.PdfiumPath) && File.Exists(job.PdfiumPath))
    {
        return job.PdfiumPath;
    }

    var runtimeDirectories = job.FallbackCliPaths.Concat(job.PreferredServerPaths)
        .Where(path => !string.IsNullOrWhiteSpace(path))
        .Select(Path.GetDirectoryName)
        .Where(path => !string.IsNullOrWhiteSpace(path))
        .Select(path => path!);

    var baseDirectory = AppContext.BaseDirectory;
    var currentDirectory = Environment.CurrentDirectory;
    var roots = runtimeDirectories.Concat([
        Path.Combine(baseDirectory, "bin"),
        Path.Combine(baseDirectory, "resources", "bin"),
        baseDirectory,
        Path.GetFullPath(Path.Combine(baseDirectory, "..", "..", "bin")),
        Path.Combine(currentDirectory, "bin"),
        Path.Combine(currentDirectory, "resources", "bin"),
        Path.Combine(currentDirectory, "app", "src-tauri", "bin")
    ]);

    return roots
        .Distinct(StringComparer.OrdinalIgnoreCase)
        .Select(root => Path.Combine(root, "pdfium.dll"))
        .FirstOrDefault(File.Exists);
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

static string CompactRuntimeError(string text)
{
    var lines = text.Replace("\r\n", "\n", StringComparison.Ordinal).Replace('\r', '\n').Split('\n')
        .Select(line => line.Trim())
        .Where(line => !string.IsNullOrWhiteSpace(line))
        .Where(line => !line.StartsWith("load_backend:", StringComparison.OrdinalIgnoreCase))
        .Where(line => !line.StartsWith("build:", StringComparison.OrdinalIgnoreCase))
        .Where(line => !line.StartsWith("common_init_result:", StringComparison.OrdinalIgnoreCase))
        .Where(line => !line.StartsWith("llama_params_fit:", StringComparison.OrdinalIgnoreCase))
        .Distinct(StringComparer.OrdinalIgnoreCase)
        .ToList();

    var relevant = lines
        .Where(line =>
            line.Contains("error", StringComparison.OrdinalIgnoreCase)
            || line.Contains("failed", StringComparison.OrdinalIgnoreCase)
            || line.Contains("invalid", StringComparison.OrdinalIgnoreCase)
            || line.Contains("missing", StringComparison.OrdinalIgnoreCase))
        .Take(6)
        .ToList();

    if (relevant.Count == 0)
    {
        relevant = lines.TakeLast(4).ToList();
    }

    var compact = string.Join(" | ", relevant);
    return compact.Length <= 1200 ? compact : compact[..1200] + "...";
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

internal sealed record RenderedPage(int PageNumber, int TotalPages, string ImagePath, string? PreviewImagePath);

internal static class PdfiumRenderer
{
    private const int FpdfAnnot = 0x01;
    private const int MaxWorkerRenderDimension = 4096;

    public static IReadOnlyList<RenderedPage> Render(OcrWorkerJob job, string pdfiumPath, Action<object> emit)
    {
        PdfiumNative.Load(pdfiumPath);
        PdfiumNative.FPDF_InitLibrary();
        try
        {
            var safePdfPath = Path.Combine(job.TempDirectory, "source.pdf");
            File.Copy(job.SourcePath, safePdfPath, overwrite: true);

            var document = PdfiumNative.FPDF_LoadDocument(safePdfPath, null);
            if (document == IntPtr.Zero)
            {
                throw new InvalidOperationException("PDFium could not open the document.");
            }

            try
            {
                var pageCount = PdfiumNative.FPDF_GetPageCount(document);
                if (pageCount <= 0)
                {
                    return [];
                }

                var rendered = new List<RenderedPage>(pageCount);
                for (var index = 0; index < pageCount; index++)
                {
                    var pageNumber = index + 1;
                    emit(new { @event = "progress", job_id = job.JobId, stage = "rendering", percent = Math.Min(5 + (index * 45.0 / pageCount), 50), page_number = pageNumber, total_pages = pageCount, rendered_pages = index, recognized_pages = 0, message = $"Rendering page {pageNumber} of {pageCount}" });

                    var outputPath = Path.Combine(job.TempDirectory, $"page-{pageNumber:0000}.png");
                    RenderPage(document, index, outputPath, job.Dpi, job.MaxOcrDimension);
                    rendered.Add(new RenderedPage(pageNumber, pageCount, outputPath, outputPath));

                    emit(new { @event = "progress", job_id = job.JobId, stage = "rendering", percent = Math.Min(5 + (pageNumber * 45.0 / pageCount), 50), page_number = pageNumber, total_pages = pageCount, rendered_pages = pageNumber, recognized_pages = 0, message = $"Rendered page {pageNumber} of {pageCount}" });
                }

                return rendered;
            }
            finally
            {
                PdfiumNative.FPDF_CloseDocument(document);
            }
        }
        finally
        {
            PdfiumNative.FPDF_DestroyLibrary();
        }
    }

    private static void RenderPage(IntPtr document, int pageIndex, string outputPath, int dpi, int maxDimension)
    {
        var page = PdfiumNative.FPDF_LoadPage(document, pageIndex);
        if (page == IntPtr.Zero)
        {
            throw new InvalidOperationException($"PDFium could not load page {pageIndex + 1}.");
        }

        try
        {
            var widthPoints = PdfiumNative.FPDF_GetPageWidth(page);
            var heightPoints = PdfiumNative.FPDF_GetPageHeight(page);
            if (widthPoints <= 0 || heightPoints <= 0)
            {
                throw new InvalidOperationException($"PDF page {pageIndex + 1} has invalid dimensions.");
            }

            var scale = Math.Max(72, dpi) / 72.0;
            var width = Math.Max(1, (int)Math.Round(widthPoints * scale));
            var height = Math.Max(1, (int)Math.Round(heightPoints * scale));
            var effectiveMax = Math.Clamp(maxDimension <= 0 ? 1600 : maxDimension, 256, MaxWorkerRenderDimension);
            var longest = Math.Max(width, height);
            if (longest > effectiveMax)
            {
                var ratio = effectiveMax / (double)longest;
                width = Math.Max(1, (int)Math.Round(width * ratio));
                height = Math.Max(1, (int)Math.Round(height * ratio));
            }

            var pdfBitmap = PdfiumNative.FPDFBitmap_Create(width, height, alpha: 0);
            if (pdfBitmap == IntPtr.Zero)
            {
                throw new InvalidOperationException("PDFium could not allocate a page bitmap.");
            }

            try
            {
                PdfiumNative.FPDFBitmap_FillRect(pdfBitmap, 0, 0, width, height, 0xFFFFFFFF);
                PdfiumNative.FPDF_RenderPageBitmap(pdfBitmap, page, 0, 0, width, height, rotate: 0, flags: FpdfAnnot);
                SavePdfiumBitmapAsPng(pdfBitmap, outputPath, width, height);
            }
            finally
            {
                PdfiumNative.FPDFBitmap_Destroy(pdfBitmap);
            }
        }
        finally
        {
            PdfiumNative.FPDF_ClosePage(page);
        }
    }

    private static void SavePdfiumBitmapAsPng(IntPtr pdfBitmap, string outputPath, int width, int height)
    {
        var buffer = PdfiumNative.FPDFBitmap_GetBuffer(pdfBitmap);
        var sourceStride = PdfiumNative.FPDFBitmap_GetStride(pdfBitmap);
        if (buffer == IntPtr.Zero || sourceStride <= 0)
        {
            throw new InvalidOperationException("PDFium returned an empty page bitmap.");
        }

        using var bitmap = new Bitmap(width, height, PixelFormat.Format32bppRgb);
        var bounds = new Rectangle(0, 0, width, height);
        var data = bitmap.LockBits(bounds, ImageLockMode.WriteOnly, PixelFormat.Format32bppRgb);
        try
        {
            var rowBytes = width * 4;
            var row = new byte[rowBytes];
            var destinationStride = Math.Abs(data.Stride);
            for (var y = 0; y < height; y++)
            {
                Marshal.Copy(IntPtr.Add(buffer, y * sourceStride), row, 0, rowBytes);
                var destinationRow = data.Stride < 0
                    ? IntPtr.Add(data.Scan0, (height - 1 - y) * destinationStride)
                    : IntPtr.Add(data.Scan0, y * destinationStride);
                Marshal.Copy(row, 0, destinationRow, rowBytes);
            }
        }
        finally
        {
            bitmap.UnlockBits(data);
        }

        bitmap.Save(outputPath, ImageFormat.Png);
    }
}

internal static partial class PdfiumNative
{
    private static IntPtr _libraryHandle;

    public static void Load(string path)
    {
        if (_libraryHandle != IntPtr.Zero)
        {
            return;
        }

        _libraryHandle = NativeLibrary.Load(path);
    }

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDF_InitLibrary();

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDF_DestroyLibrary();

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr FPDF_LoadDocument(string filePath, string? password);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDF_CloseDocument(IntPtr document);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern int FPDF_GetPageCount(IntPtr document);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr FPDF_LoadPage(IntPtr document, int pageIndex);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDF_ClosePage(IntPtr page);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern double FPDF_GetPageWidth(IntPtr page);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern double FPDF_GetPageHeight(IntPtr page);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr FPDFBitmap_Create(int width, int height, int alpha);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDFBitmap_Destroy(IntPtr bitmap);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr FPDFBitmap_GetBuffer(IntPtr bitmap);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern int FPDFBitmap_GetStride(IntPtr bitmap);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDFBitmap_FillRect(IntPtr bitmap, int left, int top, int width, int height, uint color);

    [DllImport("pdfium.dll", CallingConvention = CallingConvention.Cdecl)]
    public static extern void FPDF_RenderPageBitmap(IntPtr bitmap, IntPtr page, int startX, int startY, int sizeX, int sizeY, int rotate, int flags);
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

    public int Dpi { get; init; } = 300;

    public int MaxOcrDimension { get; init; } = 1600;

    public int Threads { get; init; } = Math.Max(1, Environment.ProcessorCount - 1);

    public string? ModelPath { get; init; }

    public string? MmprojPath { get; init; }

    public string? PdfiumPath { get; init; }

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
            Dpi = GetInt(root, "dpi") ?? 300,
            MaxOcrDimension = GetInt(root, "max_ocr_dimension") ?? 1600,
            ModelPath = model.ValueKind == JsonValueKind.Object ? GetString(model, "model_path") : null,
            MmprojPath = model.ValueKind == JsonValueKind.Object ? GetString(model, "mmproj_path") : null,
            PdfiumPath = runtime.ValueKind == JsonValueKind.Object ? GetString(runtime, "pdfium_path") : null,
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

    private static int? GetInt(JsonElement root, string property)
    {
        return root.TryGetProperty(property, out var value) && value.TryGetInt32(out var result) ? result : null;
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
