using App.Models;

namespace App.Core.Contracts;

public interface IRuntimeDetectionService
{
    RuntimeStatus GetRuntimeStatus(RuntimeProfile profile);
}
