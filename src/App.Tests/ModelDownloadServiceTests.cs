using System.Net;
using System.Security.Cryptography;
using System.Text;
using App.Services.Models;
using App.Services.Storage;

namespace App.Tests;

[TestClass]
public sealed class ModelDownloadServiceTests
{
    [TestMethod]
    public async Task DownloadAsync_PromotesCompletePartWithoutRangeRequest()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var payload = Encoding.UTF8.GetBytes("hello");
        var hash = Sha256(payload);
        var paths = new StoragePathService(Path.Combine(root, "exe"), Path.Combine(root, "local"));
        paths.EnsureCreated();
        await File.WriteAllBytesAsync(Path.Combine(paths.ModelsDirectory, "model.gguf.part"), payload);

        var handler = new StubHttpHandler(hash, payload, failIfDownloadRequested: true);
        var service = new ModelDownloadService(paths, new ModelRegistryService(paths), new HttpClient(handler));

        try
        {
            var result = await service.DownloadAsync("owner/repo/model.gguf");

            Assert.IsTrue(File.Exists(result.FilePath));
            Assert.IsFalse(File.Exists(result.FilePath + ".part"));
            CollectionAssert.AreEqual(payload, await File.ReadAllBytesAsync(result.FilePath));
            Assert.AreEqual(0, handler.DownloadRequestCount);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    [TestMethod]
    public async Task DownloadAsync_RestartsWhenResumeRangeIsRejected()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var payload = Encoding.UTF8.GetBytes("hello");
        var hash = Sha256(payload);
        var paths = new StoragePathService(Path.Combine(root, "exe"), Path.Combine(root, "local"));
        paths.EnsureCreated();
        await File.WriteAllTextAsync(Path.Combine(paths.ModelsDirectory, "model.gguf.part"), "abc");

        var handler = new StubHttpHandler(hash, payload, rejectFirstRange: true);
        var service = new ModelDownloadService(paths, new ModelRegistryService(paths), new HttpClient(handler));

        try
        {
            var result = await service.DownloadAsync("owner/repo/model.gguf");

            Assert.IsTrue(handler.RequestedRanges.Contains("bytes=3-"));
            Assert.AreEqual(2, handler.DownloadRequestCount);
            Assert.IsTrue(File.Exists(result.FilePath));
            Assert.IsFalse(File.Exists(result.FilePath + ".part"));
            CollectionAssert.AreEqual(payload, await File.ReadAllBytesAsync(result.FilePath));
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    private static string Sha256(byte[] payload)
    {
        return Convert.ToHexString(SHA256.HashData(payload)).ToLowerInvariant();
    }

    private sealed class StubHttpHandler : HttpMessageHandler
    {
        private readonly string _sha256;
        private readonly byte[] _payload;
        private readonly bool _failIfDownloadRequested;
        private readonly bool _rejectFirstRange;

        public StubHttpHandler(string sha256, byte[] payload, bool failIfDownloadRequested = false, bool rejectFirstRange = false)
        {
            _sha256 = sha256;
            _payload = payload;
            _failIfDownloadRequested = failIfDownloadRequested;
            _rejectFirstRange = rejectFirstRange;
        }

        public int DownloadRequestCount { get; private set; }

        public List<string> RequestedRanges { get; } = [];

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/api/models/owner/repo/tree/main")
            {
                var json = $$"""
                    [
                      {
                        "path": "model.gguf",
                        "size": {{_payload.Length}},
                        "lfs": {
                          "oid": "{{_sha256}}",
                          "size": {{_payload.Length}}
                        }
                      }
                    ]
                    """;
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new StringContent(json, Encoding.UTF8, "application/json")
                });
            }

            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/owner/repo/resolve/main/model.gguf")
            {
                DownloadRequestCount++;
                if (_failIfDownloadRequested)
                {
                    return Task.FromResult(new HttpResponseMessage(HttpStatusCode.InternalServerError));
                }

                if (request.Headers.Range is not null)
                {
                    RequestedRanges.Add(request.Headers.Range.ToString());
                    if (_rejectFirstRange && DownloadRequestCount == 1)
                    {
                        return Task.FromResult(new HttpResponseMessage(HttpStatusCode.RequestedRangeNotSatisfiable));
                    }
                }

                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new ByteArrayContent(_payload)
                });
            }

            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.NotFound));
        }
    }
}
