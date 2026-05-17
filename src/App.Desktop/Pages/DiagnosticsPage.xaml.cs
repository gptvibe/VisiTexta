using System.Diagnostics;
using System.IO;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace App_Desktop.Pages;

public sealed partial class DiagnosticsPage : Page
{
    public DiagnosticsPage()
    {
        InitializeComponent();
        Loaded += DiagnosticsPage_Loaded;
    }

    private async void DiagnosticsPage_Loaded(object sender, RoutedEventArgs e)
    {
        await RefreshAsync();
    }

    private async void Refresh_Click(object sender, RoutedEventArgs e)
    {
        await RefreshAsync();
    }

    private async Task RefreshAsync()
    {
        ReportTextBox.Text = await AppServices.Diagnostics.CreateReportAsync();
    }

    private void Copy_Click(object sender, RoutedEventArgs e)
    {
        var package = new DataPackage();
        package.SetText(ReportTextBox.Text);
        Clipboard.SetContent(package);
    }

    private void OpenLogs_Click(object sender, RoutedEventArgs e)
    {
        Directory.CreateDirectory(AppServices.Paths.LogsDirectory);
        Process.Start(new ProcessStartInfo { FileName = AppServices.Paths.LogsDirectory, UseShellExecute = true });
    }
}
