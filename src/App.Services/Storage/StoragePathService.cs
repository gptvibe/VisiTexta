using App.Core.Contracts;
using App.Core.Output;
using App.Models;

namespace App.Services.Storage;

public sealed class StoragePathService : IStoragePathService
{
    private const string AppDirectoryName = "VisiTexta";
    private const string PortableDataDirectoryName = "portable-data";
    private const string PortableMarkerFileName = "visitexta-portable.txt";

    public StoragePathService(string? executableDirectory = null, string? installedRoot = null)
    {
        ExecutableDirectory = executableDirectory ?? AppContext.BaseDirectory;
        var localRoot = installedRoot ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), AppDirectoryName);
        Mode = ResolveMode(ExecutableDirectory);
        RootDirectory = Mode == StorageMode.Portable
            ? Path.Combine(ExecutableDirectory, PortableDataDirectoryName)
            : localRoot;
        SettingsPath = Path.Combine(RootDirectory, "settings.json");
        HistoryDirectory = Path.Combine(RootDirectory, "history");
        ModelsDirectory = Path.Combine(RootDirectory, "models");
        DownloadsDirectory = Path.Combine(RootDirectory, "downloads");
        TempDirectory = Path.Combine(RootDirectory, "temp");
        LogsDirectory = Path.Combine(RootDirectory, "logs");
        DiagnosticsDirectory = Path.Combine(RootDirectory, "diagnostics");
        PastedInputsDirectory = Path.Combine(RootDirectory, "pasted-inputs");
    }

    public string ExecutableDirectory { get; }

    public StorageMode Mode { get; }

    public string RootDirectory { get; }

    public string SettingsPath { get; }

    public string HistoryDirectory { get; }

    public string ModelsDirectory { get; }

    public string DownloadsDirectory { get; }

    public string TempDirectory { get; }

    public string LogsDirectory { get; }

    public string DiagnosticsDirectory { get; }

    public string PastedInputsDirectory { get; }

    public void EnsureCreated()
    {
        Directory.CreateDirectory(RootDirectory);
        Directory.CreateDirectory(HistoryDirectory);
        Directory.CreateDirectory(ModelsDirectory);
        Directory.CreateDirectory(DownloadsDirectory);
        Directory.CreateDirectory(TempDirectory);
        Directory.CreateDirectory(LogsDirectory);
        Directory.CreateDirectory(DiagnosticsDirectory);
        Directory.CreateDirectory(PastedInputsDirectory);
    }

    public StorageInfo GetStorageInfo()
    {
        return new StorageInfo
        {
            Mode = Mode,
            RootPath = RootDirectory,
            SettingsPath = SettingsPath,
            HistoryPath = HistoryDirectory,
            ModelsPath = ModelsDirectory,
            DownloadsPath = DownloadsDirectory,
            TempPath = TempDirectory,
            LogsPath = LogsDirectory,
            DiagnosticsPath = DiagnosticsDirectory,
            PastedInputsPath = PastedInputsDirectory
        };
    }

    public string CreateJobTempDirectory(string jobId)
    {
        EnsureCreated();
        var safeId = SanitizeFileName(string.IsNullOrWhiteSpace(jobId) ? Guid.NewGuid().ToString("N") : jobId);
        var path = Path.Combine(TempDirectory, "job-" + safeId);
        Directory.CreateDirectory(path);
        return path;
    }

    public string GetDuplicateSafeOutputPath(string sourcePath, OcrExportFormat format)
    {
        return OutputPathHelper.GetDuplicateSafePath(sourcePath, format);
    }

    private static StorageMode ResolveMode(string executableDirectory)
    {
        return Directory.Exists(Path.Combine(executableDirectory, PortableDataDirectoryName))
            || File.Exists(Path.Combine(executableDirectory, PortableMarkerFileName))
            ? StorageMode.Portable
            : StorageMode.Installed;
    }

    private static string SanitizeFileName(string value)
    {
        foreach (var invalid in Path.GetInvalidFileNameChars())
        {
            value = value.Replace(invalid, '-');
        }

        return value;
    }
}
