using App.Models;
using App.Services.Storage;

namespace App.Tests;

[TestClass]
public sealed class StoragePathServiceTests
{
    [TestMethod]
    public void PortableDataFolder_SelectsPortableMode()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(root, "portable-data"));
        try
        {
            var service = new StoragePathService(root, Path.Combine(root, "local"));
            Assert.AreEqual(StorageMode.Portable, service.GetStorageInfo().Mode);
            StringAssert.Contains(service.RootDirectory, "portable-data");
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }
}
