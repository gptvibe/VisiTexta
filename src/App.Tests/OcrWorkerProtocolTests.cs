using App.Inference.Worker;
using App.Models;

namespace App.Tests;

[TestClass]
public sealed class OcrWorkerProtocolTests
{
    [TestMethod]
    public void ParseTextDelta_ReturnsDelta()
    {
        var parsed = OcrWorkerProtocol.ParseEvent("{\"event\":\"text_delta\",\"job_id\":\"abc\",\"page_number\":2,\"delta\":\"hello\"}");

        Assert.AreEqual("text_delta", parsed.Event);
        Assert.AreEqual("abc", parsed.JobId);
        Assert.AreEqual(2, parsed.PageNumber);
        Assert.AreEqual("hello", parsed.Delta);
    }

    [TestMethod]
    public void ParseDone_ReturnsOutputPath()
    {
        var parsed = OcrWorkerProtocol.ParseEvent("{\"event\":\"done\",\"job_id\":\"abc\",\"status\":\"done\",\"pages\":3,\"output_markdown_path\":\"C:/x/out.md\"}");

        Assert.AreEqual(OcrJobStatus.Done, parsed.Status);
        Assert.AreEqual(3, parsed.Pages);
        Assert.AreEqual("C:/x/out.md", parsed.OutputMarkdownPath);
    }
}
