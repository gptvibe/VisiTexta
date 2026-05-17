using App.Core.Contracts;
using App.Inference.Runtime;
using App.Inference.Worker;
using App.Services.Diagnostics;
using App.Services.Export;
using App.Services.History;
using App.Services.Models;
using App.Services.Settings;
using App.Services.Storage;

namespace App_Desktop;

public static class AppServices
{
    static AppServices()
    {
        Paths.EnsureCreated();
    }

    public static IStoragePathService Paths { get; } = new StoragePathService();

    public static ISettingsService Settings { get; } = new SettingsService(Paths);

    public static IRuntimeDetectionService RuntimeDetection { get; } = new RuntimeDetectionService();

    public static IModelRegistryService ModelRegistry { get; } = new ModelRegistryService(Paths);

    public static IModelDownloadService ModelDownloads { get; } = new ModelDownloadService(Paths, ModelRegistry);

    public static IHistoryService HistoryService { get; } = new HistoryService(Paths);

    public static IExportService ExportService { get; } = new ExportService(Paths);

    public static IDiagnosticsService Diagnostics { get; } = new DiagnosticsService(Paths, RuntimeDetection);

    public static IOcrWorkerClient OcrWorker { get; } = new OcrWorkerClient(Paths, Settings, ModelRegistry, RuntimeDetection, HistoryService);
}
