using System;
using System.IO;
using Microsoft.UI.Xaml;

namespace App_Desktop;

public partial class App : Application
{
    public App()
    {
        InitializeComponent();
    }

    public static Window? CurrentWindow { get; private set; }

    internal static void RecordStartupFailure(string message, Exception ex)
    {
        try
        {
            AppServices.Paths.EnsureCreated();
            var logPath = Path.Combine(AppServices.Paths.DiagnosticsDirectory, "startup-errors.log");
            var content = $"{DateTimeOffset.Now:O} {message}{Environment.NewLine}{ex}{Environment.NewLine}{Environment.NewLine}";
            File.AppendAllText(logPath, content);
        }
        catch
        {
        }
    }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            AppServices.Paths.EnsureCreated();
            await AppServices.HistoryService.RecoverInterruptedJobsAsync();
            CurrentWindow = new MainWindow();
            CurrentWindow.Activate();
        }
        catch (Exception ex)
        {
            RecordStartupFailure("Application startup failed.", ex);
            throw;
        }
    }
}
