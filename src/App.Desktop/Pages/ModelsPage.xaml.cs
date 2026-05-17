using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using App.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace App_Desktop.Pages;

public sealed partial class ModelsPage : Page
{
    private bool _isDownloading;

    public ModelsPage()
    {
        InitializeComponent();
        Loaded += ModelsPage_Loaded;
    }

    private async void ModelsPage_Loaded(object sender, RoutedEventArgs e)
    {
        await RenderAsync();
    }

    private async Task RenderAsync()
    {
        ModelsStackPanel.Children.Clear();
        CustomLocatorTextBox.IsEnabled = !_isDownloading;
        var catalog = await AppServices.ModelRegistry.GetCatalogAsync();
        foreach (var profile in catalog.Profiles)
        {
            var local = catalog.LocalModels.FirstOrDefault(model => model.ProfileId == profile.Id);
            ModelsStackPanel.Children.Add(CreateProfileCard(profile, local));
        }

        var custom = catalog.LocalModels.Where(model => model.ProfileId is null).ToList();
        if (custom.Count > 0)
        {
            ModelsStackPanel.Children.Add(new TextBlock { Text = "Local custom models", FontSize = 18, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
            foreach (var model in custom)
            {
                ModelsStackPanel.Children.Add(CreateLocalModelCard(model));
            }
        }
    }

    private FrameworkElement CreateProfileCard(OcrModelProfile profile, LocalOcrModelInfo? local)
    {
        var root = new Border { Style = (Style)Application.Current.Resources["CardBorderStyle"] };
        var grid = new Grid { ColumnSpacing = 14 };
        grid.ColumnDefinitions.Add(new ColumnDefinition());
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var text = new StackPanel { Spacing = 5 };
        text.Children.Add(new TextBlock { Text = profile.Recommended ? profile.Label + " (Recommended)" : profile.Label, FontSize = 18, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        text.Children.Add(new TextBlock { Text = $"{profile.Repo} - {profile.DefaultFile}", Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] });
        text.Children.Add(new TextBlock { Text = profile.Notes, Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] });
        text.Children.Add(new TextBlock { Text = BuildModelStatus(profile, local), Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] });

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        var download = new Button
        {
            Content = local is null ? "Download" : local.RuntimeReady ? "Redownload" : "Finish setup",
            Tag = profile.Id,
            IsEnabled = !_isDownloading
        };
        download.Click += DownloadProfile_Click;
        actions.Children.Add(download);

        var open = new Button { Content = "Open Folder", IsEnabled = local is not null && !_isDownloading, Tag = AppServices.Paths.ModelsDirectory };
        open.Click += OpenFolder_Click;
        actions.Children.Add(open);

        var delete = new Button { Content = "Delete", IsEnabled = local is not null && !_isDownloading, Tag = local?.FileName };
        delete.Click += Delete_Click;
        actions.Children.Add(delete);

        Grid.SetColumn(actions, 1);
        grid.Children.Add(text);
        grid.Children.Add(actions);
        root.Child = grid;
        return root;
    }

    private FrameworkElement CreateLocalModelCard(LocalOcrModelInfo model)
    {
        var root = new Border { Style = (Style)Application.Current.Resources["CardBorderStyle"] };
        var text = new StackPanel { Spacing = 5 };
        text.Children.Add(new TextBlock { Text = model.Label, FontSize = 18, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        text.Children.Add(new TextBlock { Text = $"{model.SupportTier} - {(model.RuntimeReady ? "ready" : "needs mmproj")} - {model.FilePath}", Style = (Style)Application.Current.Resources["MutedTextBlockStyle"] });
        var open = new Button { Content = "Open Folder", Tag = Path.GetDirectoryName(model.FilePath), HorizontalAlignment = HorizontalAlignment.Left, IsEnabled = !_isDownloading };
        open.Click += OpenFolder_Click;
        text.Children.Add(open);
        root.Child = text;
        return root;
    }

    private async void DownloadProfile_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button button && button.Tag is string profileId)
        {
            await DownloadAsync(profileId);
        }
    }

    private async void DownloadCustom_Click(object sender, RoutedEventArgs e)
    {
        var locator = CustomLocatorTextBox.Text.Trim();
        if (string.IsNullOrWhiteSpace(locator))
        {
            await ShowErrorAsync("Model locator needed", "Enter a curated profile id or owner/repo/file.gguf.");
            return;
        }

        await DownloadAsync(locator);
    }

    private async Task DownloadAsync(string locator)
    {
        if (_isDownloading)
        {
            return;
        }

        _isDownloading = true;
        DownloadProgressBar.Value = 0;
        DownloadProgressBar.IsIndeterminate = true;
        DownloadStatusTextBlock.Text = "Starting download";
        await RenderAsync();
        try
        {
            await AppServices.ModelDownloads.DownloadAsync(locator, new Progress<ModelDownloadProgress>(progress =>
            {
                DownloadStatusTextBlock.Text = BuildDownloadStatus(progress);
                if (progress.Percent is not null)
                {
                    DownloadProgressBar.IsIndeterminate = false;
                    DownloadProgressBar.Value = progress.Percent.Value;
                }
            }));
            DownloadStatusTextBlock.Text = "Model ready";
        }
        catch (Exception ex)
        {
            AppServices.Diagnostics.RecordError(ex.Message);
            DownloadStatusTextBlock.Text = "Download failed";
            await ShowErrorAsync("Download failed", ex.Message);
        }
        finally
        {
            _isDownloading = false;
            DownloadProgressBar.IsIndeterminate = false;
            await RenderAsync();
        }
    }

    private async void Delete_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: string fileName })
        {
            await AppServices.ModelDownloads.DeleteModelAsync(fileName);
            await RenderAsync();
        }
    }

    private void OpenFolder_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: string path } && Directory.Exists(path))
        {
            Process.Start(new ProcessStartInfo { FileName = path, UseShellExecute = true });
        }
    }

    private static string BuildDownloadStatus(ModelDownloadProgress progress)
    {
        if (progress.TotalBytes is null)
        {
            return $"{progress.Message} - {progress.FileName}";
        }

        return $"{progress.Message} - {FormatBytes(progress.DownloadedBytes)} / {FormatBytes(progress.TotalBytes.Value)}";
    }

    private static string BuildModelStatus(OcrModelProfile profile, LocalOcrModelInfo? local)
    {
        if (local is null)
        {
            return "Not downloaded";
        }

        if (local.RuntimeReady)
        {
            var companion = string.IsNullOrWhiteSpace(local.MmprojPath) ? "no companion needed" : Path.GetFileName(local.MmprojPath);
            return $"Ready - {Path.GetFileName(local.FilePath)} + {companion}";
        }

        var expected = profile.PreferredMmprojFile ?? "companion mmproj";
        var partPath = Path.Combine(AppServices.Paths.ModelsDirectory, expected + ".part");
        if (File.Exists(partPath))
        {
            return $"Main model downloaded. Companion is incomplete - {FormatBytes(new FileInfo(partPath).Length)} saved.";
        }

        return $"Main model downloaded. Companion missing - click Finish setup.";
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
