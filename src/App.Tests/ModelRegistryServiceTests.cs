using App.Services.Models;
using App.Services.Storage;

namespace App.Tests;

[TestClass]
public sealed class ModelRegistryServiceTests
{
    [TestMethod]
    public void ValidateRegistry_ReturnsNoErrors()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var registry = new ModelRegistryService(new StoragePathService(Path.Combine(root, "exe"), Path.Combine(root, "local")));

        var errors = registry.ValidateRegistry();

        Assert.AreEqual(0, errors.Count, string.Join(Environment.NewLine, errors));
    }

    [TestMethod]
    public void RecommendedProfile_IsGlmOcr()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var registry = new ModelRegistryService(new StoragePathService(Path.Combine(root, "exe"), Path.Combine(root, "local")));

        Assert.AreEqual("glm-ocr", registry.GetRecommendedProfile().Id);
    }
}
