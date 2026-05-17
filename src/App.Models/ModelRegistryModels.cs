namespace App.Models;

public sealed record RunnerCompatibility
{
    public bool TransientCli { get; init; } = true;

    public bool PersistentServer { get; init; } = true;

    public string Notes { get; init; } = string.Empty;
}

public sealed record OcrModelProfile
{
    public string Id { get; init; } = string.Empty;

    public string Label { get; init; } = string.Empty;

    public string Family { get; init; } = string.Empty;

    public string Repo { get; init; } = string.Empty;

    public string DefaultFile { get; init; } = string.Empty;

    public bool RequiresMmproj { get; init; }

    public bool Tested { get; init; }

    public bool Recommended { get; init; }

    public string Notes { get; init; } = string.Empty;

    public IReadOnlyList<string> FileMarkers { get; init; } = [];

    public RunnerCompatibility RunnerCompatibility { get; init; } = new();
}

public sealed record LocalOcrModelInfo
{
    public string FileName { get; init; } = string.Empty;

    public string FilePath { get; init; } = string.Empty;

    public string Label { get; init; } = string.Empty;

    public string Family { get; init; } = string.Empty;

    public string? Repo { get; init; }

    public string? ProfileId { get; init; }

    public bool RequiresMmproj { get; init; }

    public string? MmprojPath { get; init; }

    public bool RuntimeReady { get; init; }

    public bool Tested { get; init; }

    public bool Recommended { get; init; }

    public bool AutoSelectable { get; init; }

    public ModelSupportTier SupportTier { get; init; }

    public ModelInstallSource Source { get; init; }
}

public sealed record OcrModelCatalog
{
    public string DefaultProfileId { get; init; } = string.Empty;

    public IReadOnlyList<OcrModelProfile> Profiles { get; init; } = [];

    public IReadOnlyList<LocalOcrModelInfo> LocalModels { get; init; } = [];
}

public sealed record ModelDownloadProgress
{
    public string Repo { get; init; } = string.Empty;

    public string FileName { get; init; } = string.Empty;

    public long DownloadedBytes { get; init; }

    public long? TotalBytes { get; init; }

    public ModelDownloadStatus Status { get; init; }

    public string Message { get; init; } = string.Empty;

    public double? Percent => TotalBytes is null or <= 0 ? null : Math.Clamp(DownloadedBytes / (double)TotalBytes.Value * 100d, 0d, 100d);
}

public sealed record ModelDownloadResult
{
    public string Repo { get; init; } = string.Empty;

    public string FileName { get; init; } = string.Empty;

    public string FilePath { get; init; } = string.Empty;

    public string? ProfileId { get; init; }
}
