using App.Core.Contracts;
using App.Models;
using App.Services.Storage;

namespace App.Services.Settings;

public sealed class SettingsService : ISettingsService
{
    private readonly IStoragePathService _paths;
    private readonly JsonFileStore<AppSettings> _store = new();

    public SettingsService(IStoragePathService paths)
    {
        _paths = paths;
    }

    public async Task<AppSettings> LoadAsync(CancellationToken cancellationToken = default)
    {
        _paths.EnsureCreated();
        return await _store.LoadAsync(_paths.SettingsPath, cancellationToken) ?? new AppSettings();
    }

    public Task SaveAsync(AppSettings settings, CancellationToken cancellationToken = default)
    {
        _paths.EnsureCreated();
        return _store.SaveAsync(_paths.SettingsPath, settings, cancellationToken);
    }
}
