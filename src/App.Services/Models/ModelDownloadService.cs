using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text.Json;
using App.Core.Contracts;
using App.Models;

namespace App.Services.Models;

public sealed class ModelDownloadService : IModelDownloadService
{
    private const string HuggingFaceApiBase = "https://huggingface.co/api/models";
    private readonly IStoragePathService _paths;
    private readonly IModelRegistryService _registry;
    private readonly HttpClient _httpClient;

    public ModelDownloadService(IStoragePathService paths, IModelRegistryService registry, HttpClient? httpClient = null)
    {
        _paths = paths;
        _registry = registry;
        _httpClient = httpClient ?? new HttpClient();
        _httpClient.DefaultRequestHeaders.UserAgent.ParseAdd("VisiTexta-Native/3.0.4");
    }

    public async Task<ModelDownloadResult> DownloadAsync(
        string profileIdOrLocator,
        IProgress<ModelDownloadProgress>? progress = null,
        CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(profileIdOrLocator))
        {
            throw new ArgumentException("Model profile or locator is required.", nameof(profileIdOrLocator));
        }

        var plan = await ResolvePlanAsync(profileIdOrLocator.Trim(), cancellationToken);
        _paths.EnsureCreated();

        var targetPath = Path.Combine(_paths.ModelsDirectory, plan.FileName);
        await DownloadFileAsync(plan.Repo, plan.FileName, targetPath, plan.Metadata, plan.ProfileId is not null, progress, cancellationToken);

        string? downloadedMmproj = null;
        if (plan.Profile?.RequiresMmproj == true)
        {
            var mmprojFile = plan.Files
                .Where(file => file.Name.EndsWith(".gguf", StringComparison.OrdinalIgnoreCase) && file.Name.Contains("mmproj", StringComparison.OrdinalIgnoreCase))
                .OrderBy(file => file.Name.Contains("f16", StringComparison.OrdinalIgnoreCase) ? 0 : 1)
                .ThenBy(file => file.Size ?? long.MaxValue)
                .FirstOrDefault();
            if (mmprojFile is null)
            {
                throw new InvalidOperationException("This model requires a companion mmproj file, but none was found in the Hugging Face repo.");
            }

            downloadedMmproj = mmprojFile.Name;
            await DownloadFileAsync(plan.Repo, mmprojFile.Name, Path.Combine(_paths.ModelsDirectory, mmprojFile.Name), mmprojFile, true, progress, cancellationToken);
        }

        await File.WriteAllTextAsync(
            Path.Combine(_paths.ModelsDirectory, ".visitexta-models.json"),
            JsonSerializer.Serialize(new
            {
                updated_at = DateTimeOffset.Now,
                repo = plan.Repo,
                file_name = plan.FileName,
                profile_id = plan.ProfileId,
                mmproj_file = downloadedMmproj
            }, new JsonSerializerOptions { WriteIndented = true }),
            cancellationToken);

        return new ModelDownloadResult
        {
            Repo = plan.Repo,
            FileName = plan.FileName,
            FilePath = targetPath,
            ProfileId = plan.ProfileId
        };
    }

    public Task DeleteModelAsync(string fileName, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var safeFile = Path.GetFileName(fileName);
        var path = Path.Combine(_paths.ModelsDirectory, safeFile);
        if (File.Exists(path))
        {
            File.Delete(path);
        }

        var part = path + ".part";
        if (File.Exists(part))
        {
            File.Delete(part);
        }

        return Task.CompletedTask;
    }

    private async Task<DownloadPlan> ResolvePlanAsync(string input, CancellationToken cancellationToken)
    {
        var profile = _registry.GetProfiles().FirstOrDefault(candidate => candidate.Id.Equals(input, StringComparison.OrdinalIgnoreCase));
        if (profile is not null)
        {
            var files = await FetchFilesAsync(profile.Repo, cancellationToken);
            var file = files.FirstOrDefault(item => item.Name.Equals(profile.DefaultFile, StringComparison.OrdinalIgnoreCase))
                ?? throw new InvalidOperationException($"The curated model file was not found: {profile.DefaultFile}");
            return new DownloadPlan(profile.Repo, profile.DefaultFile, profile.Id, profile, file, files);
        }

        var locator = NormalizeLocator(input);
        var slash = locator.LastIndexOf('/');
        if (slash <= 0 || !locator.EndsWith(".gguf", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException("Custom model downloads must use owner/repo/file.gguf. Curated profiles can use their profile id.");
        }

        var repo = locator[..slash];
        var fileName = locator[(slash + 1)..];
        var remoteFiles = await FetchFilesAsync(repo, cancellationToken);
        var remoteFile = remoteFiles.FirstOrDefault(item => item.Name.Equals(fileName, StringComparison.OrdinalIgnoreCase))
            ?? throw new InvalidOperationException($"The requested model file was not found: {fileName}");
        return new DownloadPlan(repo, fileName, null, null, remoteFile, remoteFiles);
    }

    private static string NormalizeLocator(string input)
    {
        var value = input.Trim().Split(['?', '#'])[0].Trim().TrimEnd('/');
        value = value.Replace("https://huggingface.co/", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace("http://huggingface.co/", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace("https://hf.co/", string.Empty, StringComparison.OrdinalIgnoreCase)
            .Replace("http://hf.co/", string.Empty, StringComparison.OrdinalIgnoreCase);
        value = value.Replace("/blob/main/", "/", StringComparison.OrdinalIgnoreCase)
            .Replace("/resolve/main/", "/", StringComparison.OrdinalIgnoreCase);
        return value;
    }

    private async Task<IReadOnlyList<RemoteFileMetadata>> FetchFilesAsync(string repo, CancellationToken cancellationToken)
    {
        using var response = await _httpClient.GetAsync($"{HuggingFaceApiBase}/{repo}/tree/main", cancellationToken);
        if (!response.IsSuccessStatusCode)
        {
            throw new InvalidOperationException($"Unable to read model files from Hugging Face: {(int)response.StatusCode} {response.ReasonPhrase}");
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var document = await JsonDocument.ParseAsync(stream, cancellationToken: cancellationToken);
        var files = new List<RemoteFileMetadata>();
        foreach (var item in document.RootElement.EnumerateArray())
        {
            var name = item.TryGetProperty("rfilename", out var rfilename)
                ? rfilename.GetString()
                : item.TryGetProperty("path", out var pathElement)
                    ? pathElement.GetString()
                    : null;
            if (string.IsNullOrWhiteSpace(name))
            {
                continue;
            }

            long? size = item.TryGetProperty("size", out var sizeElement) && sizeElement.TryGetInt64(out var sizeValue) ? sizeValue : null;
            string? sha256 = null;
            if (item.TryGetProperty("lfs", out var lfs))
            {
                if (lfs.TryGetProperty("oid", out var oid))
                {
                    sha256 = oid.GetString();
                }

                if (size is null && lfs.TryGetProperty("size", out var lfsSize) && lfsSize.TryGetInt64(out var lfsSizeValue))
                {
                    size = lfsSizeValue;
                }
            }

            files.Add(new RemoteFileMetadata(name, size, sha256));
        }

        return files;
    }

    private async Task DownloadFileAsync(
        string repo,
        string fileName,
        string targetPath,
        RemoteFileMetadata metadata,
        bool requireChecksum,
        IProgress<ModelDownloadProgress>? progress,
        CancellationToken cancellationToken)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(targetPath)!);
        var partPath = targetPath + ".part";

        if (await TryUseExistingFileAsync(repo, fileName, targetPath, metadata, requireChecksum, progress, cancellationToken))
        {
            if (File.Exists(partPath))
            {
                File.Delete(partPath);
            }

            return;
        }

        if (await TryPromoteCompletePartAsync(repo, fileName, partPath, targetPath, metadata, requireChecksum, progress, cancellationToken))
        {
            return;
        }

        var resumeFrom = File.Exists(partPath) ? new FileInfo(partPath).Length : 0;
        if (metadata.Size is not null && resumeFrom >= metadata.Size.Value)
        {
            File.Delete(partPath);
            resumeFrom = 0;
        }

        long downloaded;
        while (true)
        {
            progress?.Report(new ModelDownloadProgress
            {
                Repo = repo,
                FileName = fileName,
                DownloadedBytes = resumeFrom,
                TotalBytes = metadata.Size,
                Status = ModelDownloadStatus.Downloading,
                Message = resumeFrom > 0 ? "Resuming partial download" : "Starting download"
            });

            using var request = new HttpRequestMessage(HttpMethod.Get, $"https://huggingface.co/{repo}/resolve/main/{Uri.EscapeDataString(fileName)}");
            if (resumeFrom > 0)
            {
                request.Headers.Range = new RangeHeaderValue(resumeFrom, null);
            }

            using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
            if (resumeFrom > 0 && response.StatusCode == HttpStatusCode.RequestedRangeNotSatisfiable)
            {
                File.Delete(partPath);
                resumeFrom = 0;
                progress?.Report(new ModelDownloadProgress
                {
                    Repo = repo,
                    FileName = fileName,
                    DownloadedBytes = 0,
                    TotalBytes = metadata.Size,
                    Status = ModelDownloadStatus.Downloading,
                    Message = "Partial download was stale; restarting"
                });
                continue;
            }

            if (resumeFrom > 0 && response.StatusCode == HttpStatusCode.OK)
            {
                resumeFrom = 0;
            }

            response.EnsureSuccessStatusCode();
            downloaded = await WriteResponseToPartAsync(response, partPath, resumeFrom, repo, fileName, metadata, progress, cancellationToken);
            break;
        }

        progress?.Report(new ModelDownloadProgress
        {
            Repo = repo,
            FileName = fileName,
            DownloadedBytes = downloaded,
            TotalBytes = metadata.Size,
            Status = ModelDownloadStatus.Verifying,
            Message = "Verifying download checksum"
        });

        if (!string.IsNullOrWhiteSpace(metadata.Sha256))
        {
            var actual = await ComputeSha256Async(partPath, cancellationToken);
            if (!actual.Equals(metadata.Sha256, StringComparison.OrdinalIgnoreCase))
            {
                File.Delete(partPath);
                throw new InvalidOperationException("Download checksum verification failed; the file will be downloaded again next time.");
            }
        }
        else if (requireChecksum)
        {
            throw new InvalidOperationException("Checksum verification is required for curated downloads, but Hugging Face did not provide one.");
        }

        if (File.Exists(targetPath))
        {
            File.Delete(targetPath);
        }

        File.Move(partPath, targetPath);
        progress?.Report(new ModelDownloadProgress
        {
            Repo = repo,
            FileName = fileName,
            DownloadedBytes = downloaded,
            TotalBytes = metadata.Size,
            Status = ModelDownloadStatus.Downloaded,
            Message = string.IsNullOrWhiteSpace(metadata.Sha256) ? "Download complete" : "Download complete and checksum verified"
        });
    }

    private static async Task<long> WriteResponseToPartAsync(
        HttpResponseMessage response,
        string partPath,
        long resumeFrom,
        string repo,
        string fileName,
        RemoteFileMetadata metadata,
        IProgress<ModelDownloadProgress>? progress,
        CancellationToken cancellationToken)
    {
        await using var input = await response.Content.ReadAsStreamAsync(cancellationToken);
        await using var output = new FileStream(partPath, resumeFrom > 0 ? FileMode.Append : FileMode.Create, FileAccess.Write, FileShare.Read);
        var buffer = new byte[1024 * 128];
        var downloaded = resumeFrom;
        int read;
        while ((read = await input.ReadAsync(buffer, cancellationToken)) > 0)
        {
            await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
            downloaded += read;
            progress?.Report(new ModelDownloadProgress
            {
                Repo = repo,
                FileName = fileName,
                DownloadedBytes = downloaded,
                TotalBytes = metadata.Size,
                Status = ModelDownloadStatus.Downloading,
                Message = "Downloading " + fileName
            });
        }

        await output.FlushAsync(cancellationToken);
        return downloaded;
    }

    private async Task<bool> TryUseExistingFileAsync(
        string repo,
        string fileName,
        string targetPath,
        RemoteFileMetadata metadata,
        bool requireChecksum,
        IProgress<ModelDownloadProgress>? progress,
        CancellationToken cancellationToken)
    {
        if (!File.Exists(targetPath))
        {
            return false;
        }

        if (!await IsValidDownloadedFileAsync(targetPath, metadata, requireChecksum, cancellationToken))
        {
            File.Delete(targetPath);
            return false;
        }

        progress?.Report(new ModelDownloadProgress
        {
            Repo = repo,
            FileName = fileName,
            DownloadedBytes = new FileInfo(targetPath).Length,
            TotalBytes = metadata.Size,
            Status = ModelDownloadStatus.Downloaded,
            Message = "Model already downloaded"
        });
        return true;
    }

    private async Task<bool> TryPromoteCompletePartAsync(
        string repo,
        string fileName,
        string partPath,
        string targetPath,
        RemoteFileMetadata metadata,
        bool requireChecksum,
        IProgress<ModelDownloadProgress>? progress,
        CancellationToken cancellationToken)
    {
        if (!File.Exists(partPath) || metadata.Size is null || new FileInfo(partPath).Length != metadata.Size.Value)
        {
            return false;
        }

        progress?.Report(new ModelDownloadProgress
        {
            Repo = repo,
            FileName = fileName,
            DownloadedBytes = metadata.Size.Value,
            TotalBytes = metadata.Size,
            Status = ModelDownloadStatus.Verifying,
            Message = "Verifying completed partial download"
        });

        if (!await IsValidDownloadedFileAsync(partPath, metadata, requireChecksum, cancellationToken))
        {
            File.Delete(partPath);
            return false;
        }

        if (File.Exists(targetPath))
        {
            File.Delete(targetPath);
        }

        File.Move(partPath, targetPath);
        progress?.Report(new ModelDownloadProgress
        {
            Repo = repo,
            FileName = fileName,
            DownloadedBytes = metadata.Size.Value,
            TotalBytes = metadata.Size,
            Status = ModelDownloadStatus.Downloaded,
            Message = "Completed partial download verified"
        });
        return true;
    }

    private async Task<bool> IsValidDownloadedFileAsync(
        string path,
        RemoteFileMetadata metadata,
        bool requireChecksum,
        CancellationToken cancellationToken)
    {
        if (metadata.Size is not null && new FileInfo(path).Length != metadata.Size.Value)
        {
            return false;
        }

        if (!string.IsNullOrWhiteSpace(metadata.Sha256))
        {
            var actual = await ComputeSha256Async(path, cancellationToken);
            return actual.Equals(metadata.Sha256, StringComparison.OrdinalIgnoreCase);
        }

        if (requireChecksum)
        {
            throw new InvalidOperationException("Checksum verification is required for curated downloads, but Hugging Face did not provide one.");
        }

        return true;
    }

    private static async Task<string> ComputeSha256Async(string path, CancellationToken cancellationToken)
    {
        await using var stream = File.OpenRead(path);
        var hash = await SHA256.HashDataAsync(stream, cancellationToken);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    private sealed record RemoteFileMetadata(string Name, long? Size, string? Sha256);

    private sealed record DownloadPlan(string Repo, string FileName, string? ProfileId, OcrModelProfile? Profile, RemoteFileMetadata Metadata, IReadOnlyList<RemoteFileMetadata> Files);
}
