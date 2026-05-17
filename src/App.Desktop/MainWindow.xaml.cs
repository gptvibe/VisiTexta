using System;
using System.IO;
using App.Models;
using App_Desktop.Pages;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace App_Desktop;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        Title = "VisiTexta";
        ApplyWindowIcon();
        NavFrame.Navigate(typeof(NewOcrPage));
        NavView.Loaded += MainWindow_Loaded;
    }

    private async void MainWindow_Loaded(object sender, RoutedEventArgs e)
    {
        var settings = await AppServices.Settings.LoadAsync();
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = settings.Theme switch
            {
                AppThemePreference.Light => ElementTheme.Light,
                AppThemePreference.Dark => ElementTheme.Dark,
                _ => ElementTheme.Default
            };
        }
    }

    private void ApplyWindowIcon()
    {
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(iconPath))
        {
            AppWindow.SetIcon(iconPath);
        }
    }

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is not NavigationViewItem item)
        {
            return;
        }

        switch (item.Tag)
        {
            case "new":
                NavFrame.Navigate(typeof(NewOcrPage));
                break;
            case "models":
                NavFrame.Navigate(typeof(ModelsPage));
                break;
            case "history":
                NavFrame.Navigate(typeof(HistoryPage));
                break;
            case "settings":
                NavFrame.Navigate(typeof(SettingsPage));
                break;
            case "diagnostics":
                NavFrame.Navigate(typeof(DiagnosticsPage));
                break;
        }
    }
}
