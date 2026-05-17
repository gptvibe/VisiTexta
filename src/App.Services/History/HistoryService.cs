using App.Core.Contracts;
using App.Models;
using App.Services.Storage;

namespace App.Services.History;

public sealed class HistoryService : IHistoryService
{
    private readonly IStoragePathService _paths;
    private readonly JsonFileStore<OcrHistoryItem> _store = new();

    public HistoryService(IStoragePathService paths)
    {
        _paths = paths;
    }

    public async Task<IReadOnlyList<OcrHistoryItem>> GetHistoryAsync(CancellationToken cancellationToken = default)
    {
        _paths.EnsureCreated();
        var items = new List<OcrHistoryItem>();

        foreach (var file in Directory.EnumerateFiles(_paths.HistoryDirectory, "*.json"))
        {
            cancellationToken.ThrowIfCancellationRequested();
            var item = await _store.LoadAsync(file, cancellationToken);
            if (item is not null)
            {
                items.Add(item);
            }
        }

        return items.OrderByDescending(item => item.UpdatedAt).ToList();
    }

    public Task<OcrHistoryItem?> GetAsync(string id, CancellationToken cancellationToken = default)
    {
        return _store.LoadAsync(GetPath(id), cancellationToken);
    }

    public Task SaveAsync(OcrHistoryItem item, CancellationToken cancellationToken = default)
    {
        _paths.EnsureCreated();
        return _store.SaveAsync(GetPath(item.Id), item with { UpdatedAt = DateTimeOffset.Now }, cancellationToken);
    }

    public Task DeleteAsync(string id, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var path = GetPath(id);
        if (File.Exists(path))
        {
            File.Delete(path);
        }

        return Task.CompletedTask;
    }

    public async Task RecoverInterruptedJobsAsync(CancellationToken cancellationToken = default)
    {
        var items = await GetHistoryAsync(cancellationToken);
        foreach (var item in items.Where(item => item.Status is not (OcrJobStatus.Done or OcrJobStatus.Failed or OcrJobStatus.Canceled)))
        {
            await SaveAsync(item with
            {
                Status = OcrJobStatus.Failed,
                Error = "VisiTexta closed before this OCR job finished."
            }, cancellationToken);
        }
    }

    private string GetPath(string id)
    {
        foreach (var invalid in Path.GetInvalidFileNameChars())
        {
            id = id.Replace(invalid, '-');
        }

        return Path.Combine(_paths.HistoryDirectory, id + ".json");
    }
}
