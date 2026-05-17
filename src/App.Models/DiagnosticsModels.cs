namespace App.Models;

public sealed record DiagnosticSnapshot
{
    public string AppVersion { get; init; } = string.Empty;

    public string WindowsVersion { get; init; } = string.Empty;

    public string DotNetVersion { get; init; } = string.Empty;

    public string ProcessArchitecture { get; init; } = string.Empty;

    public StorageInfo Storage { get; init; } = new();

    public RuntimeStatus Runtime { get; init; } = new();

    public IReadOnlyList<string> LastErrors { get; init; } = [];
}
