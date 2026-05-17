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

    public NewOcrPage()
    {
        InitializeComponent();
        Loaded += NewOcrPage_Loaded;
    }

    private async void NewOcrPage_Loaded(object sender, RoutedEventArgs e)
    {
        WorkflowComboBox.ItemsSource = Enum.GetValues<OcrWorkflowMode>();
        RuntimeComboBox.ItemsSource = Enum.GetValues<RuntimeProfile>();
        ExtractTemplateComboBox.ItemsSource = new[] { "invoice_receipt", "table_to_csv", "meeting_whiteboard", "contract_key_points" };

        var settings = await AppServices.Settings.LoadAsync();
        WorkflowComboBox.SelectedItem = settings.DefaultWorkflowMode;
        RuntimeComboBox.SelectedItem = settings.RuntimeProfile;
        ExtractTemplateComboBox.SelectedItem = settings.ExtractTemplateId;
        DpiNumberBox.Value = settings.Dpi;
        MaxDimensionNumberBox.Value = settings.MaxOcrDimension;
        StudyBoostCheckBox.IsChecked = settings.StudyBoost;

        var catalog = await AppServices.ModelRegistry.GetCatalogAsync();
        ModelComboBox.ItemsSource = catalog.LocalModels.Count > 0 ? catalog.LocalModels : catalog.Profiles.Select(profile => new LocalOcrModelInfo { Label = profile.Label, ProfileId = profile.Id, FileName = profile.DefaultFile }).ToList();
        ModelComboBox.SelectedIndex = 0;
        UpdateModeControls();
    }

    private void WorkflowComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        UpdateModeControls();
    }

    private void UpdateModeControls()
    {
        ExtractTemplateComboBox.IsEnabled = WorkflowComboBox.SelectedItem is OcrWorkflowMode.Extract;
        StudyBoostCheckBox.IsEnabled = WorkflowComboBox.SelectedItem is OcrWorkflowMode.Notes;
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
        StartButton.IsEnabled = true;
        OutputMetaTextBlock.Text = Path.GetFileName(path);
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
            var workflow = WorkflowComboBox.SelectedItem is OcrWorkflowMode mode ? mode : OcrWorkflowMode.ExactOcr;
            var runtime = RuntimeComboBox.SelectedItem is RuntimeProfile profile ? profile : RuntimeProfile.CpuCompatible;
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
            StartButton.IsEnabled = _sourcePath is not null;
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
        OutputTextBox.Text += delta;
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
