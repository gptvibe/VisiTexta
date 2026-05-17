using System;
using System.Text;
using System.Threading.Tasks;
using App.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace App_Desktop.Pages;

public sealed partial class SettingsPage : Page
{
    public SettingsPage()
    {
        InitializeComponent();
        Loaded += SettingsPage_Loaded;
    }

    private async void SettingsPage_Loaded(object sender, RoutedEventArgs e)
    {
        ThemeComboBox.ItemsSource = Enum.GetValues<AppThemePreference>();
        RuntimeComboBox.ItemsSource = Enum.GetValues<RuntimeProfile>();
        ModeComboBox.ItemsSource = Enum.GetValues<OcrWorkflowMode>();
        ExportFormatComboBox.ItemsSource = Enum.GetValues<OcrExportFormat>();

        var settings = await AppServices.Settings.LoadAsync();
        ThemeComboBox.SelectedItem = settings.Theme;
        RuntimeComboBox.SelectedItem = settings.RuntimeProfile;
        ModeComboBox.SelectedItem = settings.DefaultWorkflowMode;
        ExportFormatComboBox.SelectedItem = settings.DefaultExportFormat;
        DpiNumberBox.Value = settings.Dpi;
        MaxDimensionNumberBox.Value = settings.MaxOcrDimension;
        IdlePrewarmCheckBox.IsChecked = settings.IdleModelPrewarm;
        RenderPaths();
    }

    private async void Save_Click(object sender, RoutedEventArgs e)
    {
        var current = await AppServices.Settings.LoadAsync();
        await AppServices.Settings.SaveAsync(current with
        {
            Theme = ThemeComboBox.SelectedItem is AppThemePreference theme ? theme : AppThemePreference.System,
            RuntimeProfile = RuntimeComboBox.SelectedItem is RuntimeProfile runtime ? runtime : RuntimeProfile.CpuCompatible,
            DefaultWorkflowMode = ModeComboBox.SelectedItem is OcrWorkflowMode mode ? mode : OcrWorkflowMode.ExactOcr,
            DefaultExportFormat = ExportFormatComboBox.SelectedItem is OcrExportFormat format ? format : OcrExportFormat.Markdown,
            Dpi = SafeNumber(DpiNumberBox.Value, 300),
            MaxOcrDimension = SafeNumber(MaxDimensionNumberBox.Value, 1600),
            IdleModelPrewarm = IdlePrewarmCheckBox.IsChecked == true
        });

        var dialog = new ContentDialog { Title = "Settings saved", Content = "Preferences were saved locally.", CloseButtonText = "OK", XamlRoot = XamlRoot };
        await dialog.ShowAsync();
    }

    private void RenderPaths()
    {
        var storage = AppServices.Paths.GetStorageInfo();
        StorageModeTextBlock.Text = $"Storage mode: {storage.Mode}";
        var builder = new StringBuilder();
        builder.AppendLine($"Root:        {storage.RootPath}");
        builder.AppendLine($"Settings:    {storage.SettingsPath}");
        builder.AppendLine($"History:     {storage.HistoryPath}");
        builder.AppendLine($"Models:      {storage.ModelsPath}");
        builder.AppendLine($"Downloads:   {storage.DownloadsPath}");
        builder.AppendLine($"Temp:        {storage.TempPath}");
        builder.AppendLine($"Logs:        {storage.LogsPath}");
        builder.AppendLine($"Diagnostics: {storage.DiagnosticsPath}");
        builder.AppendLine($"Pasted:      {storage.PastedInputsPath}");
        PathsTextBox.Text = builder.ToString();
    }

    private static int SafeNumber(double value, int fallback)
    {
        return double.IsNaN(value) || value <= 0 ? fallback : (int)Math.Round(value);
    }
}
