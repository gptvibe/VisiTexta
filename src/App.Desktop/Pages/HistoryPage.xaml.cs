using System;
using System.Diagnostics;
using System.IO;
using System.Threading.Tasks;
using App.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace App_Desktop.Pages;

public sealed partial class HistoryPage : Page
{
    private OcrHistoryItem? _selected;

    public HistoryPage()
    {
        InitializeComponent();
        Loaded += HistoryPage_Loaded;
    }

    private async void HistoryPage_Loaded(object sender, RoutedEventArgs e)
    {
        await RenderHistoryAsync();
    }

    private async Task RenderHistoryAsync()
    {
        HistoryStackPanel.Children.Clear();
        var items = await AppServices.HistoryService.GetHistoryAsync();
        if (items.Count == 0)
        {
            HistoryStackPanel.Children.Add(new TextBlock { Text = "No OCR history yet.", Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] });
            return;
        }

        foreach (var item in items)
        {
            var button = new Button
            {
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Tag = item.Id,
                Content = new StackPanel
                {
                    Spacing = 3,
                    Children =
                    {
                        new TextBlock { Text = item.SourceName, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold },
                        new TextBlock { Text = $"{item.UpdatedAt:g} - {item.WorkflowMode} - {item.Status}", Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] },
                        new TextBlock { Text = item.OutputPath ?? item.Error ?? "No output yet", Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] }
                    }
                }
            };
            button.Click += HistoryItem_Click;
            HistoryStackPanel.Children.Add(button);
        }
    }

    private async void HistoryItem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is not Button { Tag: string id })
        {
            return;
        }

        _selected = await AppServices.HistoryService.GetAsync(id);
        if (_selected is null)
        {
            return;
        }

        TitleTextBlock.Text = _selected.SourceName;
        MetaTextBlock.Text = $"{_selected.WorkflowMode} - {_selected.Status} - {_selected.ModelFile ?? _selected.ModelProfileId ?? "No model"} - {_selected.EffectiveRuntimeLabel ?? _selected.RuntimeProfile.ToString()}";
        ResultTextBox.Text = File.Exists(_selected.OutputPath) ? await File.ReadAllTextAsync(_selected.OutputPath) : _selected.Error ?? string.Empty;
    }

    private void OpenResult_Click(object sender, RoutedEventArgs e)
    {
        if (_selected?.OutputPath is { } path && File.Exists(path))
        {
            Process.Start(new ProcessStartInfo { FileName = path, UseShellExecute = true });
        }
    }

    private void OpenFolder_Click(object sender, RoutedEventArgs e)
    {
        var path = _selected?.OutputPath ?? _selected?.SourcePath;
        var folder = string.IsNullOrWhiteSpace(path) ? null : Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(folder) && Directory.Exists(folder))
        {
            Process.Start(new ProcessStartInfo { FileName = folder, UseShellExecute = true });
        }
    }

    private async void Retry_Click(object sender, RoutedEventArgs e)
    {
        if (_selected is null)
        {
            return;
        }

        try
        {
            await AppServices.OcrWorker.RunAsync(_selected.RetryOptions);
            await RenderHistoryAsync();
        }
        catch (Exception ex)
        {
            AppServices.Diagnostics.RecordError(ex.Message);
            var dialog = new ContentDialog { Title = "Retry failed", Content = ex.Message, CloseButtonText = "OK", XamlRoot = XamlRoot };
            await dialog.ShowAsync();
        }
    }

    private async void Delete_Click(object sender, RoutedEventArgs e)
    {
        if (_selected is null)
        {
            return;
        }

        await AppServices.HistoryService.DeleteAsync(_selected.Id);
        _selected = null;
        ResultTextBox.Text = string.Empty;
        TitleTextBlock.Text = "Select a job";
        MetaTextBlock.Text = "OCR history appears here.";
        await RenderHistoryAsync();
    }
}
