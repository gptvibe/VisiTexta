using App.Models;

namespace App.Core.Contracts;

public interface IModelRegistryService
{
    IReadOnlyList<OcrModelProfile> GetProfiles();

    OcrModelProfile GetRecommendedProfile();

    Task<OcrModelCatalog> GetCatalogAsync(CancellationToken cancellationToken = default);

    Task<LocalOcrModelInfo?> ResolveActiveModelAsync(AppSettings settings, CancellationToken cancellationToken = default);

    IReadOnlyList<string> ValidateRegistry();
}
