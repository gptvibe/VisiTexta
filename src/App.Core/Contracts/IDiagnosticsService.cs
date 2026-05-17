using App.Models;

namespace App.Core.Contracts;

public interface IDiagnosticsService
{
    Task<DiagnosticSnapshot> CreateSnapshotAsync(CancellationToken cancellationToken = default);

    Task<string> CreateReportAsync(CancellationToken cancellationToken = default);

    void RecordError(string message);
}
