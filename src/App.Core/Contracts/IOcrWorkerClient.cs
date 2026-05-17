using App.Models;

namespace App.Core.Contracts;

public interface IOcrWorkerClient
{
    Task<OcrJobResult> RunAsync(
        OcrJobOptions options,
        IProgress<OcrJobProgress>? progress = null,
        IProgress<OcrPageResult>? pageProgress = null,
        IProgress<string>? textDeltas = null,
        CancellationToken cancellationToken = default);
}
