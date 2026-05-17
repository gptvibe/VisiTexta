using App.Core.Output;
using App.Models;

namespace App.Tests;

[TestClass]
public sealed class OutputPathHelperTests
{
    [TestMethod]
    public void GetDuplicateSafePath_UsesOcrSuffix()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            var source = Path.Combine(root, "scan.pdf");
            File.WriteAllText(source, "pdf");

            var first = OutputPathHelper.GetDuplicateSafePath(source, OcrExportFormat.Markdown);
            Assert.AreEqual("scan.ocr.md", Path.GetFileName(first));

            File.WriteAllText(first, "one");
            var second = OutputPathHelper.GetDuplicateSafePath(source, OcrExportFormat.Markdown);
            Assert.AreEqual("scan (ocr 2).md", Path.GetFileName(second));
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }
}
