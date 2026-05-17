using App.Models;
using App.Services.History;
using App.Services.Storage;

namespace App.Tests;

[TestClass]
public sealed class HistoryServiceTests
{
    [TestMethod]
    public async Task SaveAndLoad_RoundTripsJob()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var paths = new StoragePathService(Path.Combine(root, "exe"), Path.Combine(root, "local"));
        var service = new HistoryService(paths);

        try
        {
            var item = new OcrHistoryItem
            {
                Id = "job",
                SourcePath = @"C:\Temp\scan.png",
                SourceName = "scan.png",
                WorkflowMode = OcrWorkflowMode.ExactOcr,
                Status = OcrJobStatus.Done,
                OutputPath = @"C:\Temp\scan.ocr.md"
            };

            await service.SaveAsync(item);
            var loaded = await service.GetAsync("job");

            Assert.IsNotNull(loaded);
            Assert.AreEqual("scan.png", loaded.SourceName);
            Assert.AreEqual(OcrJobStatus.Done, loaded.Status);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }
}
