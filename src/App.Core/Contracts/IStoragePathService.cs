using App.Models;

namespace App.Core.Contracts;

public interface IStoragePathService
{
    string RootDirectory { get; }

    string SettingsPath { get; }

    string HistoryDirectory { get; }

    string ModelsDirectory { get; }

    string DownloadsDirectory { get; }

    string TempDirectory { get; }

    string LogsDirectory { get; }

    string DiagnosticsDirectory { get; }

    string PastedInputsDirectory { get; }

    StorageInfo GetStorageInfo();

    string CreateJobTempDirectory(string jobId);

    string GetDuplicateSafeOutputPath(string sourcePath, OcrExportFormat format);

    void EnsureCreated();
}
