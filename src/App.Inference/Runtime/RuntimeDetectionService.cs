using App.Core.Contracts;
using App.Models;

namespace App.Inference.Runtime;

public sealed class RuntimeDetectionService : IRuntimeDetectionService
{
    public RuntimeStatus GetRuntimeStatus(RuntimeProfile profile)
    {
        var roots = RuntimeRoots().Distinct(StringComparer.OrdinalIgnoreCase).ToList();
        var allRuntimes = roots
            .Where(Directory.Exists)
            .SelectMany(root => Directory.EnumerateFiles(root, "*.exe", SearchOption.AllDirectories))
            .Select(ClassifyRuntime)
            .Where(runtime => runtime is not null)
            .Select(runtime => runtime!)
            .OrderBy(runtime => runtime.Backend == RuntimeBackend.CpuCompatible ? 0 : 1)
            .ThenBy(runtime => runtime.Path.Length)
            .ToList();

        var serverRuntimes = allRuntimes.Where(runtime => runtime.IsServer).ToList();
        var cliRuntimes = allRuntimes.Where(runtime => !runtime.IsServer).ToList();
        var cpuAvailable = allRuntimes.Any(runtime => runtime.Backend == RuntimeBackend.CpuCompatible);
        var acceleratedAvailable = allRuntimes.Any(runtime => runtime.Backend != RuntimeBackend.CpuCompatible);
        var selected = SelectRuntimes(profile, allRuntimes);
        var pdfiumPath = roots.Select(root => Path.Combine(root, "pdfium.dll")).FirstOrDefault(File.Exists);

        var effective = selected.FirstOrDefault();
        var usable = selected.Any();
        return new RuntimeStatus
        {
            SelectedProfile = profile,
            UsableRuntime = usable,
            CpuRuntimeAvailable = cpuAvailable,
            AcceleratedRuntimeAvailable = acceleratedAvailable,
            PdfiumAvailable = pdfiumPath is not null,
            PdfiumPath = pdfiumPath,
            EffectiveRuntimeLabel = effective?.Label ?? "No usable runtime",
            ServerRuntimes = selected.Where(runtime => runtime.IsServer).ToList(),
            CliRuntimes = selected.Where(runtime => !runtime.IsServer).ToList(),
            Summary = BuildSummary(profile, usable, cpuAvailable, acceleratedAvailable, pdfiumPath is not null)
        };
    }

    private static IReadOnlyList<RuntimeExecutableInfo> SelectRuntimes(RuntimeProfile profile, IReadOnlyList<RuntimeExecutableInfo> all)
    {
        var cpu = all.Where(runtime => runtime.Backend == RuntimeBackend.CpuCompatible);
        var accelerated = all.Where(runtime => runtime.Backend != RuntimeBackend.CpuCompatible);

        return profile switch
        {
            RuntimeProfile.CpuCompatible => cpu.ToList(),
            RuntimeProfile.Auto => accelerated.Concat(cpu).ToList(),
            RuntimeProfile.AcceleratedIfAvailable => accelerated.Concat(cpu).ToList(),
            _ => all.ToList()
        };
    }

    private static RuntimeExecutableInfo? ClassifyRuntime(string path)
    {
        var name = Path.GetFileNameWithoutExtension(path).ToLowerInvariant();
        var isServer = name.StartsWith("llama-server", StringComparison.Ordinal);
        var isCli = name.StartsWith("llama-mtmd-cli", StringComparison.Ordinal) || name.StartsWith("llama-cli", StringComparison.Ordinal);
        if (!isServer && !isCli)
        {
            return null;
        }

        var loweredPath = path.ToLowerInvariant();
        var backend = RuntimeBackend.CpuCompatible;
        if (loweredPath.Contains("cuda", StringComparison.Ordinal) || loweredPath.Contains("cublas", StringComparison.Ordinal))
        {
            backend = RuntimeBackend.Cuda;
        }
        else if (loweredPath.Contains("directml", StringComparison.Ordinal) || loweredPath.Contains("\\dml\\", StringComparison.Ordinal))
        {
            backend = RuntimeBackend.DirectMl;
        }
        else if (loweredPath.Contains("vulkan", StringComparison.Ordinal))
        {
            backend = RuntimeBackend.Vulkan;
        }
        else if (loweredPath.Contains("accelerated", StringComparison.Ordinal) || loweredPath.Contains("\\gpu\\", StringComparison.Ordinal))
        {
            backend = RuntimeBackend.GenericAccelerated;
        }

        return new RuntimeExecutableInfo
        {
            Path = path,
            Backend = backend,
            Label = backend switch
            {
                RuntimeBackend.Cuda => "accelerated CUDA",
                RuntimeBackend.DirectMl => "accelerated DirectML",
                RuntimeBackend.Vulkan => "accelerated Vulkan",
                RuntimeBackend.GenericAccelerated => "accelerated runtime",
                _ => "CPU compatible"
            },
            IsServer = isServer
        };
    }

    private static IEnumerable<string> RuntimeRoots()
    {
        var baseDir = AppContext.BaseDirectory;
        yield return Path.Combine(baseDir, "bin");
        yield return Path.Combine(baseDir, "resources", "bin");
        yield return baseDir;

        var cwd = Environment.CurrentDirectory;
        yield return Path.Combine(cwd, "bin");
        yield return Path.Combine(cwd, "resources", "bin");
        yield return Path.Combine(cwd, "app", "src-tauri", "bin");
    }

    private static string BuildSummary(RuntimeProfile profile, bool usable, bool cpuAvailable, bool acceleratedAvailable, bool pdfiumAvailable)
    {
        if (!usable)
        {
            return "No local llama OCR runtime was found. Add llama-server.exe or llama-mtmd-cli.exe under bin or resources/bin.";
        }

        var runtimeText = profile switch
        {
            RuntimeProfile.CpuCompatible => cpuAvailable ? "CPU-compatible runtime is selected." : "CPU-compatible runtime was requested, but only accelerated runtimes were found.",
            RuntimeProfile.Auto => acceleratedAvailable ? "Auto can try an accelerated runtime and keep CPU fallback when available." : "Auto is using the CPU-compatible runtime.",
            RuntimeProfile.AcceleratedIfAvailable => acceleratedAvailable ? "Accelerated runtime will be tried first with CPU fallback when available." : "No accelerated runtime was found, so CPU-compatible runtime is used.",
            _ => "Runtime detected."
        };

        return pdfiumAvailable ? runtimeText : runtimeText + " PDFium is missing, so PDF OCR will not be ready.";
    }
}
