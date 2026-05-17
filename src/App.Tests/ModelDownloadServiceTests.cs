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

    [TestMethod]
    public async Task DownloadAsync_CuratedProfileDownloadsPreferredMmproj()
    {
        var root = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var modelPayload = Encoding.UTF8.GetBytes("model");
        var preferredMmprojPayload = Encoding.UTF8.GetBytes("preferred-mmproj");
        var f16MmprojPayload = Encoding.UTF8.GetBytes("f16-mmproj");
        var paths = new StoragePathService(Path.Combine(root, "exe"), Path.Combine(root, "local"));
        paths.EnsureCreated();

        var handler = new CuratedMmprojHttpHandler(modelPayload, preferredMmprojPayload, f16MmprojPayload);
        var service = new ModelDownloadService(paths, new ModelRegistryService(paths), new HttpClient(handler));

        try
        {
            var result = await service.DownloadAsync("glm-ocr");

            Assert.AreEqual("GLM-OCR.Q4_K_M.gguf", result.FileName);
            Assert.IsTrue(File.Exists(Path.Combine(paths.ModelsDirectory, "GLM-OCR.Q4_K_M.gguf")));
            Assert.IsTrue(File.Exists(Path.Combine(paths.ModelsDirectory, "GLM-OCR.mmproj-Q8_0.gguf")));
            Assert.IsFalse(File.Exists(Path.Combine(paths.ModelsDirectory, "GLM-OCR.mmproj-f16.gguf")));
            CollectionAssert.AreEqual(preferredMmprojPayload, await File.ReadAllBytesAsync(Path.Combine(paths.ModelsDirectory, "GLM-OCR.mmproj-Q8_0.gguf")));
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
            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/api/models/owner/repo/tree/main?recursive=true")
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

    private sealed class CuratedMmprojHttpHandler : HttpMessageHandler
    {
        private readonly byte[] _modelPayload;
        private readonly byte[] _preferredMmprojPayload;
        private readonly byte[] _f16MmprojPayload;

        public CuratedMmprojHttpHandler(byte[] modelPayload, byte[] preferredMmprojPayload, byte[] f16MmprojPayload)
        {
            _modelPayload = modelPayload;
            _preferredMmprojPayload = preferredMmprojPayload;
            _f16MmprojPayload = f16MmprojPayload;
        }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/api/models/mradermacher/GLM-OCR-GGUF/tree/main?recursive=true")
            {
                var json = $$"""
                    [
                      {
                        "path": "GLM-OCR.Q4_K_M.gguf",
                        "size": {{_modelPayload.Length}},
                        "lfs": { "oid": "{{Sha256(_modelPayload)}}", "size": {{_modelPayload.Length}} }
                      },
                      {
                        "path": "GLM-OCR.mmproj-f16.gguf",
                        "size": {{_f16MmprojPayload.Length}},
                        "lfs": { "oid": "{{Sha256(_f16MmprojPayload)}}", "size": {{_f16MmprojPayload.Length}} }
                      },
                      {
                        "path": "GLM-OCR.mmproj-Q8_0.gguf",
                        "size": {{_preferredMmprojPayload.Length}},
                        "lfs": { "oid": "{{Sha256(_preferredMmprojPayload)}}", "size": {{_preferredMmprojPayload.Length}} }
                      }
                    ]
                    """;
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new StringContent(json, Encoding.UTF8, "application/json")
                });
            }

            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/mradermacher/GLM-OCR-GGUF/resolve/main/GLM-OCR.Q4_K_M.gguf")
            {
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new ByteArrayContent(_modelPayload)
                });
            }

            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/mradermacher/GLM-OCR-GGUF/resolve/main/GLM-OCR.mmproj-Q8_0.gguf")
            {
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new ByteArrayContent(_preferredMmprojPayload)
                });
            }

            if (request.RequestUri?.AbsoluteUri == "https://huggingface.co/mradermacher/GLM-OCR-GGUF/resolve/main/GLM-OCR.mmproj-f16.gguf")
            {
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new ByteArrayContent(_f16MmprojPayload)
                });
            }

            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.NotFound));
        }
    }
}
