using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;
using App.Core.Contracts;
using App.Models;

namespace App.Inference.Worker;

public sealed class OcrWorkerClient : IOcrWorkerClient
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web)
    {
        Converters = { new JsonStringEnumConverter() }
    };

    private readonly IStoragePathService _paths;
    private readonly ISettingsService _settings;
    private readonly IModelRegistryService _models;
    private readonly IRuntimeDetectionService _runtimeDetection;
    private readonly IHistoryService _history;

    public OcrWorkerClient(
        IStoragePathService paths,
        ISettingsService settings,
        IModelRegistryService models,
        IRuntimeDetectionService runtimeDetection,
        IHistoryService history)
    {
        _paths = paths;
        _settings = settings;
        _models = models;
        _runtimeDetection = runtimeDetection;
        _history = history;
    }

    public async Task<OcrJobResult> RunAsync(
        OcrJobOptions options,
        IProgress<OcrJobProgress>? progress = null,
        IProgress<OcrPageResult>? pageProgress = null,
        IProgress<string>? textDeltas = null,
        CancellationToken cancellationToken = default)
    {
        if (!File.Exists(options.SourcePath))
        {
            throw new FileNotFoundException("The selected image or PDF could not be found.", options.SourcePath);
        }

        var settings = await _settings.LoadAsync(cancellationToken);
        var effectiveOptions = MergeOptions(settings, options);
        var jobId = Guid.NewGuid().ToString();
        var jobTemp = _paths.CreateJobTempDirectory(jobId);
        var outputPath = _paths.GetDuplicateSafeOutputPath(effectiveOptions.SourcePath, OcrExportFormat.Markdown);
        var model = await _models.ResolveActiveModelAsync(settings, cancellationToken);
        var runtime = _runtimeDetection.GetRuntimeStatus(effectiveOptions.RuntimeProfile);
        var warnings = new List<string>();
        var pageMarkdown = new SortedDictionary<int, string>();

        var historyItem = new OcrHistoryItem
        {
            Id = jobId,
            SourcePath = effectiveOptions.SourcePath,
            SourceName = Path.GetFileName(effectiveOptions.SourcePath),
            WorkflowMode = effectiveOptions.WorkflowMode,
            ModelProfileId = model?.ProfileId ?? effectiveOptions.ModelProfileId,
            ModelFile = model?.FileName ?? effectiveOptions.ModelFile,
            RuntimeProfile = effectiveOptions.RuntimeProfile,
            EffectiveRuntimeLabel = runtime.EffectiveRuntimeLabel,
            Status = OcrJobStatus.Queued,
            OutputPath = outputPath,
            RetryOptions = effectiveOptions
        };
        await _history.SaveAsync(historyItem, cancellationToken);

        using var process = CreateProcess();
        try
        {
            process.Start();
        }
        catch (Exception ex)
        {
            throw new OcrWorkerException("The OCR worker could not be started.", ex);
        }

        using var registration = cancellationToken.Register(() =>
        {
            try
            {
                if (!process.HasExited)
                {
                    process.StandardInput.WriteLine(OcrWorkerProtocol.BuildCancelCommand(jobId));
                    process.Kill(entireProcessTree: true);
                }
            }
            catch
            {
                // Best-effort cancellation.
            }
        });

        var stderrTask = process.StandardError.ReadToEndAsync(cancellationToken);
        var command = BuildJobCommand(jobId, effectiveOptions, outputPath, jobTemp, model, runtime);
        await process.StandardInput.WriteLineAsync(command);
        await process.StandardInput.FlushAsync(cancellationToken);
        process.StandardInput.Close();

        OcrJobResult? result = null;
        while (true)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var line = await process.StandardOutput.ReadLineAsync(cancellationToken);
            if (line is null)
            {
                break;
            }

            if (string.IsNullOrWhiteSpace(line))
            {
                continue;
            }

            var workerEvent = OcrWorkerProtocol.ParseEvent(line);
            switch (workerEvent.Event)
            {
                case "job_started":
                    progress?.Report(new OcrJobProgress { JobId = jobId, Status = OcrJobStatus.Ocr, Percent = 0, Message = workerEvent.Message ?? "OCR started" });
                    break;
                case "progress":
                    progress?.Report(new OcrJobProgress
                    {
                        JobId = jobId,
                        Status = StatusFromStage(workerEvent.Stage) ?? workerEvent.Status ?? OcrJobStatus.Ocr,
                        Percent = workerEvent.Percent ?? 0,
                        Message = workerEvent.Message ?? "Working locally",
                        PageNumber = workerEvent.PageNumber,
                        TotalPages = workerEvent.TotalPages,
                        RenderedPages = workerEvent.RenderedPages,
                        RecognizedPages = workerEvent.RecognizedPages
                    });
                    break;
                case "page_started":
                    progress?.Report(new OcrJobProgress
                    {
                        JobId = jobId,
                        Status = OcrJobStatus.Ocr,
                        Percent = workerEvent.Percent ?? 0,
                        Message = workerEvent.Message ?? "Reading page",
                        PageNumber = workerEvent.PageNumber,
                        TotalPages = workerEvent.TotalPages
                    });
                    break;
                case "text_delta":
                    if (!string.IsNullOrEmpty(workerEvent.Delta))
                    {
                        textDeltas?.Report(workerEvent.Delta);
                    }
                    break;
                case "page_done":
                    if (workerEvent.PageNumber is not null && workerEvent.PageMarkdown is not null)
                    {
                        pageMarkdown[workerEvent.PageNumber.Value] = workerEvent.PageMarkdown;
                        pageProgress?.Report(new OcrPageResult
                        {
                            PageNumber = workerEvent.PageNumber.Value,
                            TotalPages = workerEvent.TotalPages ?? workerEvent.PageNumber.Value,
                            Markdown = workerEvent.PageMarkdown,
                            PreviewImagePath = workerEvent.PreviewImagePath
                        });
                    }
                    break;
                case "warning":
                    warnings.Add(workerEvent.Message ?? workerEvent.Code ?? "OCR worker warning");
                    break;
                case "error":
                    throw new OcrWorkerException(workerEvent.Message ?? "The OCR worker failed.");
                case "done":
                    var markdown = string.Join("\n\n", pageMarkdown.OrderBy(pair => pair.Key).Select(pair => pair.Value)).Trim();
                    if (File.Exists(outputPath))
                    {
                        markdown = await File.ReadAllTextAsync(outputPath, cancellationToken);
                    }

                    result = new OcrJobResult
                    {
                        JobId = jobId,
                        SourcePath = effectiveOptions.SourcePath,
                        OutputPath = workerEvent.OutputMarkdownPath ?? outputPath,
                        WorkflowMode = effectiveOptions.WorkflowMode,
                        Status = OcrJobStatus.Done,
                        Pages = workerEvent.Pages ?? pageMarkdown.Count,
                        Markdown = markdown,
                        Warnings = warnings
                    };
                    break;
            }
        }

        await process.WaitForExitAsync(cancellationToken);
        var stderr = await stderrTask;
        if (process.ExitCode != 0)
        {
            throw new OcrWorkerException(string.IsNullOrWhiteSpace(stderr) ? "The OCR worker exited with an error." : stderr.Trim());
        }

        result ??= new OcrJobResult
        {
            JobId = jobId,
            SourcePath = effectiveOptions.SourcePath,
            OutputPath = outputPath,
            WorkflowMode = effectiveOptions.WorkflowMode,
            Status = OcrJobStatus.Done,
            Pages = pageMarkdown.Count,
            Markdown = string.Join("\n\n", pageMarkdown.OrderBy(pair => pair.Key).Select(pair => pair.Value)).Trim(),
            Warnings = warnings
        };

        await _history.SaveAsync(historyItem with
        {
            Status = result.Status,
            OutputPath = result.OutputPath,
            Pages = result.Pages,
            Warnings = result.Warnings
        }, cancellationToken);
        return result;
    }

    private static OcrJobOptions MergeOptions(AppSettings settings, OcrJobOptions options)
    {
        return options with
        {
            WorkflowMode = options.WorkflowMode,
            ExportFormat = options.ExportFormat,
            RuntimeProfile = options.RuntimeProfile,
            Dpi = options.Dpi <= 0 ? settings.Dpi : options.Dpi,
            MaxOcrDimension = options.MaxOcrDimension <= 0 ? settings.MaxOcrDimension : options.MaxOcrDimension,
            StudyBoost = options.StudyBoost || settings.StudyBoost,
            ExtractTemplateId = string.IsNullOrWhiteSpace(options.ExtractTemplateId) ? settings.ExtractTemplateId : options.ExtractTemplateId,
            ModelProfileId = options.ModelProfileId ?? settings.ModelProfileId,
            ModelFile = options.ModelFile ?? settings.ModelFile
        };
    }

    private static OcrJobStatus? StatusFromStage(string? stage)
    {
        return stage?.ToLowerInvariant() switch
        {
            "rendering" => OcrJobStatus.Rendering,
            "ocr" => OcrJobStatus.Ocr,
            "formatting" => OcrJobStatus.Formatting,
            "writing" => OcrJobStatus.Writing,
            "done" => OcrJobStatus.Done,
            _ => null
        };
    }

    private Process CreateProcess()
    {
        var exe = LocateWorkerExecutable();
        var startInfo = new ProcessStartInfo
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
            WorkingDirectory = Path.GetDirectoryName(exe.Path) ?? AppContext.BaseDirectory
        };

        if (exe.IsDotnetProject)
        {
            startInfo.FileName = "dotnet";
            startInfo.Arguments = "run --project " + Quote(exe.Path);
        }
        else
        {
            startInfo.FileName = exe.Path;
        }

        return new Process { StartInfo = startInfo };
    }

    private static WorkerLocation LocateWorkerExecutable()
    {
        var candidates = new[]
        {
            Path.Combine(AppContext.BaseDirectory, "workers", "ocr-worker", "ocr-worker.exe"),
            Path.Combine(AppContext.BaseDirectory, "ocr-worker.exe"),
            Path.Combine(Environment.CurrentDirectory, "workers", "ocr-worker", "bin", "Debug", "net10.0", "ocr-worker.exe")
        };

        var exe = candidates.Select(Path.GetFullPath).FirstOrDefault(File.Exists);
        if (exe is not null)
        {
            return new WorkerLocation(exe, false);
        }

        var projectCandidates = new[]
        {
            Path.Combine(Environment.CurrentDirectory, "workers", "ocr-worker", "OcrWorker.csproj"),
            Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "workers", "ocr-worker", "OcrWorker.csproj")
        };
        var project = projectCandidates.Select(Path.GetFullPath).FirstOrDefault(File.Exists);
        if (project is not null)
        {
            return new WorkerLocation(project, true);
        }

        return new WorkerLocation(candidates[0], false);
    }

    private string BuildJobCommand(
        string jobId,
        OcrJobOptions options,
        string outputPath,
        string tempDir,
        LocalOcrModelInfo? model,
        RuntimeStatus runtime)
    {
        var payload = new Dictionary<string, object?>
        {
            ["command"] = "ocr_job",
            ["protocol_version"] = 1,
            ["job_id"] = jobId,
            ["source_path"] = options.SourcePath,
            ["output_markdown_path"] = outputPath,
            ["temp_dir"] = tempDir,
            ["log_dir"] = _paths.LogsDirectory,
            ["mode"] = options.WorkflowMode,
            ["prompt_override"] = options.PromptOverride,
            ["study_boost"] = options.StudyBoost,
            ["extract_template_id"] = options.ExtractTemplateId,
            ["dpi"] = options.Dpi,
            ["max_ocr_dimension"] = options.MaxOcrDimension,
            ["model"] = new
            {
                profile_id = model?.ProfileId ?? options.ModelProfileId,
                model_path = model?.FilePath,
                mmproj_path = model?.MmprojPath
            },
            ["runtime"] = new
            {
                profile = options.RuntimeProfile,
                preferred_server_paths = runtime.ServerRuntimes.Select(item => item.Path).ToArray(),
                fallback_cli_paths = runtime.CliRuntimes.Select(item => item.Path).ToArray()
            }
        };
        return JsonSerializer.Serialize(payload, JsonOptions);
    }

    private static string Quote(string value)
    {
        return "\"" + value.Replace("\"", "\\\"", StringComparison.Ordinal) + "\"";
    }

    private sealed record WorkerLocation(string Path, bool IsDotnetProject);
}
