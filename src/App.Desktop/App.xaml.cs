using Microsoft.UI.Xaml;

namespace App_Desktop;

public partial class App : Application
{
    public App()
    {
        InitializeComponent();
    }

    public static Window? CurrentWindow { get; private set; }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        AppServices.Paths.EnsureCreated();
        await AppServices.HistoryService.RecoverInterruptedJobsAsync();
        CurrentWindow = new MainWindow();
        CurrentWindow.Activate();
    }
}
