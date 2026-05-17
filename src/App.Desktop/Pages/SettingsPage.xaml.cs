using System;
using System.Linq;
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
        var themeOptions = Enum.GetValues<AppThemePreference>().Select(theme => new DisplayOption<AppThemePreference>(theme, FormatTheme(theme))).ToList();
        var runtimeOptions = Enum.GetValues<RuntimeProfile>().Select(profile => new DisplayOption<RuntimeProfile>(profile, FormatRuntimeProfile(profile))).ToList();
        var modeOptions = Enum.GetValues<OcrWorkflowMode>().Select(mode => new DisplayOption<OcrWorkflowMode>(mode, FormatWorkflowMode(mode))).ToList();
        var formatOptions = Enum.GetValues<OcrExportFormat>().Select(format => new DisplayOption<OcrExportFormat>(format, format.ToString())).ToList();
        ThemeComboBox.ItemsSource = themeOptions;
        RuntimeComboBox.ItemsSource = runtimeOptions;
        ModeComboBox.ItemsSource = modeOptions;
        ExportFormatComboBox.ItemsSource = formatOptions;

        var settings = await AppServices.Settings.LoadAsync();
        ThemeComboBox.SelectedItem = themeOptions.FirstOrDefault(option => option.Value == settings.Theme);
        RuntimeComboBox.SelectedItem = runtimeOptions.FirstOrDefault(option => option.Value == settings.RuntimeProfile);
        ModeComboBox.SelectedItem = modeOptions.FirstOrDefault(option => option.Value == settings.DefaultWorkflowMode);
        ExportFormatComboBox.SelectedItem = formatOptions.FirstOrDefault(option => option.Value == settings.DefaultExportFormat);
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
            Theme = ThemeComboBox.SelectedItem is DisplayOption<AppThemePreference> theme ? theme.Value : AppThemePreference.System,
            RuntimeProfile = RuntimeComboBox.SelectedItem is DisplayOption<RuntimeProfile> runtime ? runtime.Value : RuntimeProfile.CpuCompatible,
            DefaultWorkflowMode = ModeComboBox.SelectedItem is DisplayOption<OcrWorkflowMode> mode ? mode.Value : OcrWorkflowMode.ExactOcr,
            DefaultExportFormat = ExportFormatComboBox.SelectedItem is DisplayOption<OcrExportFormat> format ? format.Value : OcrExportFormat.Markdown,
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

    private static string FormatTheme(AppThemePreference theme)
    {
        return theme switch
        {
            AppThemePreference.System => "System",
            AppThemePreference.Light => "Light",
            AppThemePreference.Dark => "Dark",
            _ => theme.ToString()
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

    private sealed record DisplayOption<T>(T Value, string Label)
    {
        public override string ToString() => Label;
    }
}
