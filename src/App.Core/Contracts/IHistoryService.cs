using App.Models;

namespace App.Core.Contracts;

public interface IHistoryService
{
    Task<IReadOnlyList<OcrHistoryItem>> GetHistoryAsync(CancellationToken cancellationToken = default);

    Task<OcrHistoryItem?> GetAsync(string id, CancellationToken cancellationToken = default);

    Task SaveAsync(OcrHistoryItem item, CancellationToken cancellationToken = default);

    Task DeleteAsync(string id, CancellationToken cancellationToken = default);

    Task RecoverInterruptedJobsAsync(CancellationToken cancellationToken = default);
}
