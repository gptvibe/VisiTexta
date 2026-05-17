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
    private CancellationTokenSource? _cts;
    private string? _sourcePath;
    private string? _outputPath;
    private bool _hasReadyModel;

    public NewOcrPage()
    {
        InitializeComponent();
        Loaded += NewOcrPage_Loaded;
    }

    private async void NewOcrPage_Loaded(object sender, RoutedEventArgs e)
    {
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

        var catalog = await AppServices.ModelRegistry.GetCatalogAsync();
        var readyModels = catalog.LocalModels.Where(model => model.RuntimeReady).ToList();
        _hasReadyModel = readyModels.Count > 0;
        ModelComboBox.ItemsSource = readyModels.Count > 0
            ? readyModels
            : catalog.LocalModels.Count > 0
                ? catalog.LocalModels
                : catalog.Profiles.Select(profile => new LocalOcrModelInfo { Label = profile.Label, ProfileId = profile.Id, FileName = profile.DefaultFile }).ToList();
        ModelComboBox.SelectedIndex = 0;
        StatusTextBlock.Text = _hasReadyModel ? "Ready" : "Finish model setup first";
        UpdateModeControls();
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
        StartButton.IsEnabled = _hasReadyModel;
        OutputMetaTextBlock.Text = Path.GetFileName(path);
        if (!_hasReadyModel)
        {
            StatusTextBlock.Text = "Open Models and finish setup before running OCR";
        }
    }

    private async void Start_Click(object sender, RoutedEventArgs e)
    {
        if (_sourcePath is null)
        {
            return;
        }

        _cts = new CancellationTokenSource();
        StartButton.IsEnabled = false;
        CancelButton.IsEnabled = true;
        OpenOutputButton.IsEnabled = false;
        OutputTextBox.Text = string.Empty;
        ProgressBar.Value = 0;
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
            StartButton.IsEnabled = _sourcePath is not null && _hasReadyModel;
        }
    }

    private void Cancel_Click(object sender, RoutedEventArgs e)
    {
        _cts?.Cancel();
    }

    private void UpdateProgress(OcrJobProgress progress)
    {
        StatusTextBlock.Text = progress.Message;
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
