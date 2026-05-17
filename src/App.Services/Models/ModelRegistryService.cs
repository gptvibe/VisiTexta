using App.Core.Contracts;
using App.Models;

namespace App.Services.Models;

public sealed class ModelRegistryService : IModelRegistryService
{
    public const string DefaultProfileId = "glm-ocr";
    private readonly IStoragePathService _paths;

    private static readonly RunnerCompatibility CuratedRunnerCompatibility = new()
    {
        TransientCli = true,
        PersistentServer = true,
        Notes = "Curated support path with VisiTexta's bundled multimodal llama.cpp runners."
    };

    private static readonly IReadOnlyList<OcrModelProfile> Profiles =
    [
        new()
        {
            Id = "glm-ocr",
            Label = "GLM-OCR",
            Family = "GLM-OCR",
            Repo = "mradermacher/GLM-OCR-GGUF",
            DefaultFile = "GLM-OCR.Q4_K_M.gguf",
            RequiresMmproj = true,
            Tested = true,
            Recommended = true,
            Notes = "OCR-first default profile. This is the recommended setup path.",
            FileMarkers = ["glm-ocr"],
            RunnerCompatibility = CuratedRunnerCompatibility
        },
        new()
        {
            Id = "qwen2-vl-ocr-2b",
            Label = "Qwen2-VL OCR 2B",
            Family = "Qwen2-VL OCR",
            Repo = "mradermacher/Qwen2-VL-OCR-2B-Instruct-GGUF",
            DefaultFile = "Qwen2-VL-OCR-2B-Instruct.Q4_K_M.gguf",
            RequiresMmproj = true,
            Tested = true,
            Recommended = false,
            Notes = "Tested OCR-focused alternative. GLM-OCR remains the default.",
            FileMarkers = ["qwen2-vl-ocr-2b-instruct"],
            RunnerCompatibility = CuratedRunnerCompatibility
        },
        new()
        {
            Id = "qwen2.5-vl-3b",
            Label = "Qwen2.5-VL 3B",
            Family = "Qwen2.5-VL",
            Repo = "mradermacher/Qwen2.5-VL-3B-Instruct-GGUF",
            DefaultFile = "Qwen2.5-VL-3B-Instruct.Q4_K_M.gguf",
            RequiresMmproj = true,
            Tested = true,
            Recommended = false,
            Notes = "Tested general-purpose vision-language alternative. Heavier than the OCR-specific default.",
            FileMarkers = ["qwen2.5-vl-3b-instruct"],
            RunnerCompatibility = CuratedRunnerCompatibility
        }
    ];

    public ModelRegistryService(IStoragePathService paths)
    {
        _paths = paths;
    }

    public IReadOnlyList<OcrModelProfile> GetProfiles()
    {
        return Profiles;
    }

    public OcrModelProfile GetRecommendedProfile()
    {
        return Profiles.Single(profile => profile.Recommended);
    }

    public async Task<OcrModelCatalog> GetCatalogAsync(CancellationToken cancellationToken = default)
    {
        _paths.EnsureCreated();
        var localModels = await DiscoverLocalModelsAsync(cancellationToken);
        return new OcrModelCatalog
        {
            DefaultProfileId = DefaultProfileId,
            Profiles = Profiles,
            LocalModels = localModels
        };
    }

    public async Task<LocalOcrModelInfo?> ResolveActiveModelAsync(AppSettings settings, CancellationToken cancellationToken = default)
    {
        var localModels = await DiscoverLocalModelsAsync(cancellationToken);

        if (!string.IsNullOrWhiteSpace(settings.ModelFile))
        {
            return localModels.FirstOrDefault(model => model.FileName.Equals(settings.ModelFile, StringComparison.OrdinalIgnoreCase));
        }

        if (!string.IsNullOrWhiteSpace(settings.ModelProfileId))
        {
            return localModels.FirstOrDefault(model =>
                model.ProfileId?.Equals(settings.ModelProfileId, StringComparison.OrdinalIgnoreCase) == true
                && model.RuntimeReady);
        }

        return localModels
            .Where(model => model.AutoSelectable && model.RuntimeReady)
            .OrderBy(model => SupportRank(model.SupportTier))
            .ThenBy(model => ProfileOrder(model.ProfileId))
            .ThenByDescending(model => model.Recommended)
            .FirstOrDefault();
    }

    public IReadOnlyList<string> ValidateRegistry()
    {
        var errors = new List<string>();
        if (Profiles.Count(profile => profile.Recommended) != 1)
        {
            errors.Add("Exactly one OCR model profile must be recommended.");
        }

        foreach (var profile in Profiles)
        {
            if (string.IsNullOrWhiteSpace(profile.Id)) errors.Add("A profile is missing an id.");
            if (string.IsNullOrWhiteSpace(profile.Label)) errors.Add($"{profile.Id} is missing a label.");
            if (string.IsNullOrWhiteSpace(profile.Repo) || !profile.Repo.Contains('/')) errors.Add($"{profile.Id} has an invalid repo.");
            if (string.IsNullOrWhiteSpace(profile.DefaultFile) || !profile.DefaultFile.EndsWith(".gguf", StringComparison.OrdinalIgnoreCase)) errors.Add($"{profile.Id} has an invalid default GGUF file.");
            if (profile.FileMarkers.Count == 0) errors.Add($"{profile.Id} should define file markers.");
        }

        return errors;
    }

    private async Task<IReadOnlyList<LocalOcrModelInfo>> DiscoverLocalModelsAsync(CancellationToken cancellationToken)
    {
        _paths.EnsureCreated();
        if (!Directory.Exists(_paths.ModelsDirectory))
        {
            return [];
        }

        var results = new List<LocalOcrModelInfo>();
        foreach (var file in Directory.EnumerateFiles(_paths.ModelsDirectory, "*.gguf", SearchOption.TopDirectoryOnly))
        {
            cancellationToken.ThrowIfCancellationRequested();
            var name = Path.GetFileName(file);
            if (name.Contains("mmproj", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            var profile = Profiles.FirstOrDefault(candidate =>
                candidate.FileMarkers.Any(marker => name.Contains(marker, StringComparison.OrdinalIgnoreCase)));
            var requiresMmproj = profile?.RequiresMmproj ?? LooksLikeVisionModel(name);
            var mmproj = requiresMmproj ? ResolveMmprojPath(file, profile) : null;
            var tier = profile is null
                ? LooksLikeVisionModel(name) ? ModelSupportTier.Legacy : ModelSupportTier.Experimental
                : profile.Recommended ? ModelSupportTier.Recommended : ModelSupportTier.Tested;

            results.Add(new LocalOcrModelInfo
            {
                FileName = name,
                FilePath = file,
                Label = profile?.Label ?? name,
                Family = profile?.Family ?? InferFamily(name),
                Repo = profile?.Repo,
                ProfileId = profile?.Id,
                RequiresMmproj = requiresMmproj,
                MmprojPath = mmproj,
                RuntimeReady = !requiresMmproj || !string.IsNullOrWhiteSpace(mmproj),
                Tested = profile?.Tested ?? false,
                Recommended = profile?.Recommended ?? false,
                AutoSelectable = profile is not null || LooksLikeVisionModel(name),
                SupportTier = tier,
                Source = profile is null ? ModelInstallSource.Heuristic : ModelInstallSource.Registry
            });
        }

        await Task.CompletedTask;
        return results
            .OrderBy(model => SupportRank(model.SupportTier))
            .ThenBy(model => ProfileOrder(model.ProfileId))
            .ThenBy(model => model.FileName, StringComparer.OrdinalIgnoreCase)
            .ToList();
    }

    private string? ResolveMmprojPath(string modelPath, OcrModelProfile? profile)
    {
        var directory = Path.GetDirectoryName(modelPath);
        if (string.IsNullOrWhiteSpace(directory))
        {
            return null;
        }

        var candidates = Directory.EnumerateFiles(directory, "*.gguf", SearchOption.TopDirectoryOnly)
            .Where(path => Path.GetFileName(path).Contains("mmproj", StringComparison.OrdinalIgnoreCase));

        if (profile is not null)
        {
            candidates = candidates
                .OrderByDescending(path => profile.FileMarkers.Any(marker => Path.GetFileName(path).Contains(marker, StringComparison.OrdinalIgnoreCase)))
                .ThenBy(path => Path.GetFileName(path), StringComparer.OrdinalIgnoreCase);
        }

        return candidates.FirstOrDefault();
    }

    private static bool LooksLikeVisionModel(string fileName)
    {
        return fileName.Contains("glm-ocr", StringComparison.OrdinalIgnoreCase)
            || fileName.Contains("qwen", StringComparison.OrdinalIgnoreCase)
            || fileName.Contains("-vl", StringComparison.OrdinalIgnoreCase)
            || fileName.Contains("vision", StringComparison.OrdinalIgnoreCase)
            || fileName.Contains("llava", StringComparison.OrdinalIgnoreCase);
    }

    private static string InferFamily(string fileName)
    {
        if (fileName.Contains("glm-ocr", StringComparison.OrdinalIgnoreCase)) return "GLM-OCR";
        if (fileName.Contains("qwen", StringComparison.OrdinalIgnoreCase)) return "Qwen-VL";
        if (fileName.Contains("llava", StringComparison.OrdinalIgnoreCase)) return "LLaVA";
        if (fileName.Contains("vision", StringComparison.OrdinalIgnoreCase)) return "Vision GGUF";
        return "Custom";
    }

    private static int SupportRank(ModelSupportTier tier)
    {
        return tier switch
        {
            ModelSupportTier.Recommended => 0,
            ModelSupportTier.Tested => 1,
            ModelSupportTier.Legacy => 2,
            _ => 3
        };
    }

    private static int ProfileOrder(string? profileId)
    {
        if (string.IsNullOrWhiteSpace(profileId))
        {
            return Profiles.Count;
        }

        var index = Profiles.ToList().FindIndex(profile => profile.Id.Equals(profileId, StringComparison.OrdinalIgnoreCase));
        return index < 0 ? Profiles.Count : index;
    }
}
