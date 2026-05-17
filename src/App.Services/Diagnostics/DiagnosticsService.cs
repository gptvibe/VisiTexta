using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using App.Core.Contracts;
using App.Models;

namespace App.Services.Diagnostics;

public sealed class DiagnosticsService : IDiagnosticsService
{
    private readonly IStoragePathService _paths;
    private readonly IRuntimeDetectionService _runtimeDetection;
    private readonly List<string> _lastErrors = [];

    public DiagnosticsService(IStoragePathService paths, IRuntimeDetectionService runtimeDetection)
    {
        _paths = paths;
        _runtimeDetection = runtimeDetection;
    }

    public Task<DiagnosticSnapshot> CreateSnapshotAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var snapshot = new DiagnosticSnapshot
        {
            AppVersion = Assembly.GetEntryAssembly()?.GetName().Version?.ToString() ?? "development",
            WindowsVersion = Environment.OSVersion.ToString(),
            DotNetVersion = Environment.Version.ToString(),
            ProcessArchitecture = RuntimeInformation.ProcessArchitecture.ToString(),
            Storage = _paths.GetStorageInfo(),
            Runtime = _runtimeDetection.GetRuntimeStatus(RuntimeProfile.CpuCompatible),
            LastErrors = _lastErrors.ToList()
        };
        return Task.FromResult(snapshot);
    }

    public async Task<string> CreateReportAsync(CancellationToken cancellationToken = default)
    {
        var snapshot = await CreateSnapshotAsync(cancellationToken);
        var builder = new StringBuilder();
        builder.AppendLine("VisiTexta Native diagnostics");
        builder.AppendLine($"Generated: {DateTimeOffset.Now:O}");
        builder.AppendLine($"App version: {snapshot.AppVersion}");
        builder.AppendLine($"OS: {snapshot.WindowsVersion}");
        builder.AppendLine($".NET: {snapshot.DotNetVersion}");
        builder.AppendLine($"Process: {snapshot.ProcessArchitecture}");
        builder.AppendLine();
        builder.AppendLine("Storage:");
        builder.AppendLine($"- Mode: {snapshot.Storage.Mode}");
        builder.AppendLine($"- Root: {snapshot.Storage.RootPath}");
        builder.AppendLine($"- Settings: {snapshot.Storage.SettingsPath}");
        builder.AppendLine($"- History: {snapshot.Storage.HistoryPath}");
        builder.AppendLine($"- Models: {snapshot.Storage.ModelsPath}");
        builder.AppendLine($"- Temp: {snapshot.Storage.TempPath}");
        builder.AppendLine($"- Logs: {snapshot.Storage.LogsPath}");
        builder.AppendLine();
        builder.AppendLine("Runtime:");
        builder.AppendLine($"- Usable: {snapshot.Runtime.UsableRuntime}");
        builder.AppendLine($"- Effective: {snapshot.Runtime.EffectiveRuntimeLabel}");
        builder.AppendLine($"- PDFium: {(snapshot.Runtime.PdfiumAvailable ? snapshot.Runtime.PdfiumPath : "missing")}");
        builder.AppendLine($"- Summary: {snapshot.Runtime.Summary}");
        if (snapshot.LastErrors.Count > 0)
        {
            builder.AppendLine();
            builder.AppendLine("Last errors:");
            foreach (var error in snapshot.LastErrors)
            {
                builder.AppendLine($"- {error}");
            }
        }

        return builder.ToString();
    }

    public void RecordError(string message)
    {
        if (string.IsNullOrWhiteSpace(message))
        {
            return;
        }

        _lastErrors.Insert(0, $"{DateTimeOffset.Now:O} {message}");
        if (_lastErrors.Count > 20)
        {
            _lastErrors.RemoveRange(20, _lastErrors.Count - 20);
        }
    }
}
