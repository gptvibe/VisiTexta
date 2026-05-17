using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using App.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace App_Desktop.Pages;

public sealed partial class NewOcrPage : Page
{
    private readonly TaskCompletionSource _initializationCompletion = new(TaskCreationOptions.RunContinuationsAsynchronously);
    private CancellationTokenSource? _cts;
    private string? _sourcePath;
    private string? _outputPath;
    private bool _hasReadyModel;
    private bool _initialized;
    private bool _isAutoDownloading;
    private string? _recommendedProfileId;
    private string? _recommendedModelLabel;

    public NewOcrPage()
    {
        InitializeComponent();
        Loaded += NewOcrPage_Loaded;
    }

    private async void NewOcrPage_Loaded(object sender, RoutedEventArgs e)
    {
        if (_initialized)
        {
            return;
        }

        _initialized = true;
        var workflowOptions = Enum.GetValues<OcrWorkflowMode>().Select(mode => new DisplayOption<OcrWorkflowMode>(mode, FormatWorkflowMode(mode))).ToList();
        var runtimeOptions = Enum.GetValues<RuntimeProfile>().Select(profile => new DisplayOption<RuntimeProfile>(profile, FormatRuntimeProfile(profile))).ToList();
        WorkflowComboBox.ItemsSource = workflowOptions;
        RuntimeComboBox.ItemsSource = runtimeOptions;
        ExtractTemplateComboBox.ItemsSource = new[] { "invoice_receipt", "table_to_csv", "meeting_whiteboard", "contract_key_points" };

        var settings = await AppServices.Settings.LoadAsync();
        WorkflowComboBox.SelectedItem = workflowOptions.FirstOrDefault(option => option.Value == settings.DefaultWorkflowMode);
        RuntimeComboBox.SelectedItem = runtimeOptions.FirstOrDefault(option => option.Value == settings.RuntimeProfile);
        ExtractTemplateComboBox.SelectedItem = settings.ExtractTemplateId;
        DpiNumberBox.Value = settings.Dpi;
        MaxDimensionNumberBox.Value = settings.MaxOcrDimension;
        StudyBoostCheckBox.IsChecked = settings.StudyBoost;

        await RefreshModelChoicesAsync();
        StatusTextBlock.Text = _hasReadyModel ? "Ready" : "Preparing recommended model";
        UpdateModeControls();
        UpdateStartAvailability();
        _initializationCompletion.TrySetResult();

        if (!_hasReadyModel)
        {
            _ = EnsureRecommendedModelReadyAsync();
        }
    }

    private void WorkflowComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        UpdateModeControls();
    }

    private void UpdateModeControls()
    {
        var workflow = WorkflowComboBox.SelectedItem is DisplayOption<OcrWorkflowMode> option ? option.Value : OcrWorkflowMode.ExactOcr;
        ExtractTemplateComboBox.IsEnabled = workflow is OcrWorkflowMode.Extract;
        StudyBoostCheckBox.IsEnabled = workflow is OcrWorkflowMode.Notes;
    }

    private void DropZone_DragOver(object sender, DragEventArgs e)
    {
        e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy;
        e.DragUIOverride.Caption = "Use this file for local OCR";
    }

    private async void DropZone_Drop(object sender, DragEventArgs e)
    {
        if (!e.DataView.Contains(StandardDataFormats.StorageItems))
        {
            return;
        }

        var items = await e.DataView.GetStorageItemsAsync();
        var file = items.OfType<StorageFile>().FirstOrDefault();
        if (file is not null)
        {
            SelectSource(file.Path);
        }
    }

    private async void BrowseFile_Click(object sender, RoutedEventArgs e)
    {
        var picker = new FileOpenPicker
        {
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
            FileTypeFilter = { ".png", ".jpg", ".jpeg", ".pdf" }
        };
        InitializePicker(picker);
        var file = await picker.PickSingleFileAsync();
        if (file is not null)
        {
            SelectSource(file.Path);
        }
    }

    private void SelectSource(string path)
    {
        _sourcePath = path;
        SourceTextBox.Text = path;
        UpdateStartAvailability();
        OutputMetaTextBlock.Text = Path.GetFileName(path);
        if (!_hasReadyModel)
        {
            StatusTextBlock.Text = _isAutoDownloading ? "Downloading recommended model" : "Preparing recommended model";
        }
    }

    private async void Start_Click(object sender, RoutedEventArgs e)
    {
        if (_sourcePath is null)
        {
            return;
        }

        _cts = new CancellationTokenSource();
        UpdateStartAvailability();
        StartButton.IsEnabled = false;
        CancelButton.IsEnabled = true;
        OpenOutputButton.IsEnabled = false;
        OutputTextBox.Text = string.Empty;
        ProgressBar.Value = 0;
        ProgressBar.IsIndeterminate = false;
        StatusTextBlock.Text = "Starting OCR";
        _outputPath = null;

        try
        {
            var selectedModel = ModelComboBox.SelectedItem as LocalOcrModelInfo;
            if (selectedModel is null || !selectedModel.RuntimeReady)
            {
                StatusTextBlock.Text = "Model setup incomplete";
                await ShowErrorAsync("Model setup incomplete", "Open Models and click Finish setup so the OCR model and its companion mmproj file are both installed.");
                return;
            }

            var workflow = WorkflowComboBox.SelectedItem is DisplayOption<OcrWorkflowMode> workflowOption ? workflowOption.Value : OcrWorkflowMode.ExactOcr;
            var runtime = RuntimeComboBox.SelectedItem is DisplayOption<RuntimeProfile> runtimeOption ? runtimeOption.Value : RuntimeProfile.CpuCompatible;
            var options = new OcrJobOptions
            {
                SourcePath = _sourcePath,
                WorkflowMode = workflow,
                RuntimeProfile = runtime,
                PromptOverride = string.IsNullOrWhiteSpace(PromptTextBox.Text) ? null : PromptTextBox.Text.Trim(),
                StudyBoost = StudyBoostCheckBox.IsChecked == true,
                ExtractTemplateId = ExtractTemplateComboBox.SelectedItem?.ToString() ?? "invoice_receipt",
                Dpi = SafeNumber(DpiNumberBox.Value, 300),
                MaxOcrDimension = SafeNumber(MaxDimensionNumberBox.Value, 1600),
                ModelProfileId = selectedModel?.ProfileId,
                ModelFile = selectedModel?.FileName
            };

            var result = await AppServices.OcrWorker.RunAsync(
                options,
                new Progress<OcrJobProgress>(UpdateProgress),
                new Progress<OcrPageResult>(_ => { }),
                new Progress<string>(AppendDelta),
                _cts.Token);

            OutputTextBox.Text = result.Markdown;
            _outputPath = result.OutputPath;
            OpenOutputButton.IsEnabled = !string.IsNullOrWhiteSpace(_outputPath) && File.Exists(_outputPath);
            StatusTextBlock.Text = result.Status == OcrJobStatus.Done ? "OCR complete" : result.Status.ToString();
            ProgressBar.Value = 100;

            if (App.CurrentWindow is MainWindow window)
            {
                await window.RefreshSidebarHistoryAsync(result.JobId);
            }
        }
        catch (OperationCanceledException)
        {
            StatusTextBlock.Text = "Canceled";
        }
        catch (Exception ex)
        {
            AppServices.Diagnostics.RecordError(ex.Message);
            StatusTextBlock.Text = "OCR failed";
            await ShowErrorAsync("OCR failed", ex.Message);
        }
        finally
        {
            _cts?.Dispose();
            _cts = null;
            CancelButton.IsEnabled = false;
            UpdateStartAvailability();
        }
    }

    private void Cancel_Click(object sender, RoutedEventArgs e)
    {
        _cts?.Cancel();
    }

    private void UpdateProgress(OcrJobProgress progress)
    {
        StatusTextBlock.Text = progress.Message;
        ProgressBar.IsIndeterminate = false;
        ProgressBar.Value = Math.Clamp(progress.Percent, 0, 100);
        if (progress.PageNumber is not null && progress.TotalPages is not null)
        {
            OutputMetaTextBlock.Text = $"Page {progress.PageNumber}/{progress.TotalPages} - {progress.Status}";
        }
    }

    private void AppendDelta(string delta)
    {
        OutputTextBox.SelectionStart = OutputTextBox.Text.Length;
        OutputTextBox.SelectedText = delta;
        OutputTextBox.SelectionStart = OutputTextBox.Text.Length;
    }

    private void Copy_Click(object sender, RoutedEventArgs e)
    {
        var package = new DataPackage();
        package.SetText(OutputTextBox.Text);
        Clipboard.SetContent(package);
    }

    private void OpenOutput_Click(object sender, RoutedEventArgs e)
    {
        if (!string.IsNullOrWhiteSpace(_outputPath) && File.Exists(_outputPath))
        {
            Process.Start(new ProcessStartInfo { FileName = _outputPath, UseShellExecute = true });
        }
    }

    public async Task ResetForNewRunAsync()
    {
        await EnsureInitializedAsync();

        _sourcePath = null;
        _outputPath = null;
        SourceTextBox.Text = string.Empty;
        OutputTextBox.Text = string.Empty;
        OutputMetaTextBlock.Text = _hasReadyModel
            ? "Choose a file to begin."
            : "VisiTexta is preparing the recommended model for first-time setup.";
        StatusTextBlock.Text = _hasReadyModel
            ? "Ready"
            : _isAutoDownloading
                ? "Downloading recommended model"
                : "Preparing recommended model";
        ProgressBar.IsIndeterminate = _isAutoDownloading;
        ProgressBar.Value = 0;
        OpenOutputButton.IsEnabled = false;
        CancelButton.IsEnabled = false;
        UpdateStartAvailability();
    }

    public async Task OpenHistoryItemAsync(OcrHistoryItem item)
    {
        await EnsureInitializedAsync();

        _sourcePath = item.SourcePath;
        _outputPath = item.OutputPath;
        SourceTextBox.Text = item.SourcePath;
        PromptTextBox.Text = item.RetryOptions.PromptOverride ?? string.Empty;
        StudyBoostCheckBox.IsChecked = item.RetryOptions.StudyBoost;
        DpiNumberBox.Value = item.RetryOptions.Dpi;
        MaxDimensionNumberBox.Value = item.RetryOptions.MaxOcrDimension;
        ExtractTemplateComboBox.SelectedItem = item.RetryOptions.ExtractTemplateId;

        if (WorkflowComboBox.ItemsSource is System.Collections.IEnumerable workflowItems)
        {
            WorkflowComboBox.SelectedItem = workflowItems
                .OfType<DisplayOption<OcrWorkflowMode>>()
                .FirstOrDefault(option => option.Value == item.WorkflowMode);
        }

        if (RuntimeComboBox.ItemsSource is System.Collections.IEnumerable runtimeItems)
        {
            RuntimeComboBox.SelectedItem = runtimeItems
                .OfType<DisplayOption<RuntimeProfile>>()
                .FirstOrDefault(option => option.Value == item.RuntimeProfile);
        }

        if (ModelComboBox.ItemsSource is System.Collections.IEnumerable modelItems)
        {
            ModelComboBox.SelectedItem = modelItems
                .OfType<LocalOcrModelInfo>()
                .FirstOrDefault(model =>
                    (!string.IsNullOrWhiteSpace(item.ModelFile) && model.FileName.Equals(item.ModelFile, StringComparison.OrdinalIgnoreCase))
                    || (!string.IsNullOrWhiteSpace(item.ModelProfileId) && string.Equals(model.ProfileId, item.ModelProfileId, StringComparison.OrdinalIgnoreCase)))
                ?? ModelComboBox.SelectedItem;
        }

        OutputTextBox.Text = item.OutputPath is { Length: > 0 } path && File.Exists(path)
            ? await File.ReadAllTextAsync(path)
            : item.Error ?? string.Empty;
        OutputMetaTextBlock.Text = $"{item.UpdatedAt:g} • {item.WorkflowMode} • {item.Status}";
        StatusTextBlock.Text = item.Status == OcrJobStatus.Done ? "Viewing transcript" : item.Status.ToString();
        ProgressBar.IsIndeterminate = false;
        ProgressBar.Value = item.Status == OcrJobStatus.Done ? 100 : 0;
        OpenOutputButton.IsEnabled = !string.IsNullOrWhiteSpace(item.OutputPath) && File.Exists(item.OutputPath);
        UpdateModeControls();
        UpdateStartAvailability();
    }

    private async Task EnsureInitializedAsync()
    {
        await _initializationCompletion.Task;
    }

    private async Task RefreshModelChoicesAsync()
    {
        var catalog = await AppServices.ModelRegistry.GetCatalogAsync();
        var readyModels = catalog.LocalModels.Where(model => model.RuntimeReady).ToList();
        var models = readyModels.Count > 0
            ? readyModels
            : catalog.LocalModels.Count > 0
                ? catalog.LocalModels
                : catalog.Profiles.Select(profile => new LocalOcrModelInfo
                {
                    Label = profile.Label,
                    ProfileId = profile.Id,
                    FileName = profile.DefaultFile,
                    Family = profile.Family,
                    Recommended = profile.Recommended,
                    Tested = profile.Tested,
                    RequiresMmproj = profile.RequiresMmproj,
                    RuntimeReady = false,
                    SupportTier = profile.Recommended ? ModelSupportTier.Recommended : ModelSupportTier.Tested,
                    Source = ModelInstallSource.Registry
                }).ToList();

        _recommendedProfileId = catalog.Profiles.FirstOrDefault(profile => profile.Recommended)?.Id;
        _recommendedModelLabel = catalog.Profiles.FirstOrDefault(profile => profile.Recommended)?.Label;
        _hasReadyModel = readyModels.Count > 0;

        var selectedFileName = (ModelComboBox.SelectedItem as LocalOcrModelInfo)?.FileName;
        ModelComboBox.ItemsSource = models;
        ModelComboBox.SelectedItem = models.FirstOrDefault(model => string.Equals(model.FileName, selectedFileName, StringComparison.OrdinalIgnoreCase));
        if (ModelComboBox.SelectedItem is null)
        {
            ModelComboBox.SelectedIndex = models.Count > 0 ? 0 : -1;
        }
    }

    private async Task EnsureRecommendedModelReadyAsync()
    {
        if (_isAutoDownloading || _hasReadyModel || string.IsNullOrWhiteSpace(_recommendedProfileId))
        {
            return;
        }

        _isAutoDownloading = true;
        UpdateStartAvailability();
        ProgressBar.IsIndeterminate = true;
        ProgressBar.Value = 0;
        StatusTextBlock.Text = "Downloading recommended model";
        OutputMetaTextBlock.Text = $"{_recommendedModelLabel ?? "Recommended OCR model"} is downloading for first-time setup.";

        try
        {
            await AppServices.ModelDownloads.DownloadAsync(_recommendedProfileId, new Progress<ModelDownloadProgress>(progress =>
            {
                StatusTextBlock.Text = BuildDownloadStatus(progress);
                if (progress.Percent is not null)
                {
                    ProgressBar.IsIndeterminate = false;
                    ProgressBar.Value = progress.Percent.Value;
                }
            }));

            await RefreshModelChoicesAsync();
            StatusTextBlock.Text = "Recommended model ready";
            OutputMetaTextBlock.Text = _sourcePath is null ? "Choose a file to begin." : Path.GetFileName(_sourcePath);
        }
        catch (Exception ex)
        {
            AppServices.Diagnostics.RecordError(ex.Message);
            ProgressBar.IsIndeterminate = false;
            ProgressBar.Value = 0;
            StatusTextBlock.Text = "Model download failed";
            OutputMetaTextBlock.Text = "Open Models from the sidebar to retry the recommended setup.";
        }
        finally
        {
            _isAutoDownloading = false;
            UpdateStartAvailability();
        }
    }

    private void UpdateStartAvailability()
    {
        StartButton.IsEnabled = _sourcePath is not null && _hasReadyModel && !_isAutoDownloading && _cts is null;
    }

    private static string BuildDownloadStatus(ModelDownloadProgress progress)
    {
        if (progress.TotalBytes is null)
        {
            return progress.Message;
        }

        return $"{progress.Message} - {FormatBytes(progress.DownloadedBytes)} / {FormatBytes(progress.TotalBytes.Value)}";
    }

    private static string FormatBytes(long bytes)
    {
        string[] units = ["B", "KB", "MB", "GB"];
        var value = (double)bytes;
        var unit = 0;
        while (value >= 1024 && unit < units.Length - 1)
        {
            value /= 1024;
            unit++;
        }

        return unit == 0 ? $"{bytes} {units[unit]}" : $"{value:0.0} {units[unit]}";
    }

    private static int SafeNumber(double value, int fallback)
    {
        return double.IsNaN(value) || value <= 0 ? fallback : (int)Math.Round(value);
    }

    private static string FormatWorkflowMode(OcrWorkflowMode mode)
    {
        return mode switch
        {
            OcrWorkflowMode.ExactOcr => "Exact OCR",
            OcrWorkflowMode.Notes => "Notes",
            OcrWorkflowMode.Extract => "Extract",
            _ => mode.ToString()
        };
    }

    private static string FormatRuntimeProfile(RuntimeProfile profile)
    {
        return profile switch
        {
            RuntimeProfile.Auto => "Auto",
            RuntimeProfile.CpuCompatible => "CPU compatible",
            RuntimeProfile.AcceleratedIfAvailable => "Accelerated if available",
            _ => profile.ToString()
        };
    }

    private sealed record DisplayOption<T>(T Value, string Label)
    {
        public override string ToString() => Label;
    }

    private static void InitializePicker(object picker)
    {
        if (App.CurrentWindow is null)
        {
            return;
        }

        InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(App.CurrentWindow));
    }

    private async Task ShowErrorAsync(string title, string message)
    {
        var dialog = new ContentDialog
        {
            Title = title,
            Content = message,
            CloseButtonText = "OK",
            XamlRoot = XamlRoot
        };
        await dialog.ShowAsync();
    }
}
