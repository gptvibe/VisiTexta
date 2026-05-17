using App.Models;

namespace App.Core.Contracts;

public interface IModelDownloadService
{
    Task<ModelDownloadResult> DownloadAsync(
        string profileIdOrLocator,
        IProgress<ModelDownloadProgress>? progress = null,
        CancellationToken cancellationToken = default);

    Task DeleteModelAsync(string fileName, CancellationToken cancellationToken = default);
}
