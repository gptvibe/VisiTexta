namespace App.Models;

public sealed record RuntimeExecutableInfo
{
    public string Path { get; init; } = string.Empty;

    public RuntimeBackend Backend { get; init; }

    public string Label { get; init; } = string.Empty;

    public bool IsServer { get; init; }
}

public sealed record RuntimeStatus
{
    public RuntimeProfile SelectedProfile { get; init; }

    public RuntimeProfile SafeDefaultProfile { get; init; } = RuntimeProfile.CpuCompatible;

    public bool UsableRuntime { get; init; }

    public bool CpuRuntimeAvailable { get; init; }

    public bool AcceleratedRuntimeAvailable { get; init; }

    public bool PdfiumAvailable { get; init; }

    public string? PdfiumPath { get; init; }

    public string EffectiveRuntimeLabel { get; init; } = "No usable runtime";

    public string Summary { get; init; } = string.Empty;

    public IReadOnlyList<RuntimeExecutableInfo> ServerRuntimes { get; init; } = [];

    public IReadOnlyList<RuntimeExecutableInfo> CliRuntimes { get; init; } = [];
}
