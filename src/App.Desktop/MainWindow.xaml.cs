using System;
using System.IO;
using App.Models;
using App_Desktop.Pages;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace App_Desktop;

public sealed partial class MainWindow : Window
{
    private readonly Grid _rootGrid = new();
    private readonly Frame _navFrame = new();
    private readonly TextBlock _pageErrorTextBlock = new()
    {
        Margin = new Thickness(24),
        TextWrapping = TextWrapping.Wrap,
        Visibility = Visibility.Collapsed
    };

    public MainWindow()
    {
        Content = _rootGrid;
        BuildShell();
        ApplySystemBackdrop();
        Title = "VisiTexta";
        ApplyWindowIcon();
        _rootGrid.Loaded += MainWindow_Loaded;
    }

    private void BuildShell()
    {
        _rootGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(220) });
        _rootGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var sidebar = new Border
        {
            Padding = new Thickness(16),
            BorderThickness = new Thickness(0, 0, 1, 0)
        };
        Grid.SetColumn(sidebar, 0);

        var sidebarStack = new StackPanel { Spacing = 12 };
        sidebarStack.Children.Add(new TextBlock
        {
            Text = "VisiTexta",
            FontSize = 24,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold
        });
        sidebarStack.Children.Add(CreateNavButton("New OCR", typeof(NewOcrPage)));
        sidebarStack.Children.Add(CreateNavButton("Models", typeof(ModelsPage)));
        sidebarStack.Children.Add(CreateNavButton("History", typeof(HistoryPage)));
        sidebarStack.Children.Add(CreateNavButton("Settings", typeof(SettingsPage)));
        sidebarStack.Children.Add(CreateNavButton("Diagnostics", typeof(DiagnosticsPage)));
        sidebar.Child = sidebarStack;

        var contentGrid = new Grid();
        Grid.SetColumn(contentGrid, 1);
        contentGrid.Children.Add(_navFrame);
        contentGrid.Children.Add(_pageErrorTextBlock);

        _rootGrid.Children.Add(sidebar);
        _rootGrid.Children.Add(contentGrid);
    }

    private Button CreateNavButton(string label, Type pageType)
    {
        var button = new Button { Content = label };
        button.Click += (_, _) => NavigateTo(pageType);
        return button;
    }

    private void ApplySystemBackdrop()
    {
        if (!OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000))
        {
            return;
        }

        try
        {
            SystemBackdrop = new MicaBackdrop();
        }
        catch (Exception ex)
        {
            App.RecordStartupFailure("System backdrop initialization failed.", ex);
        }
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

        NavigateTo(typeof(NewOcrPage));
    }

    private void ApplyWindowIcon()
    {
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(iconPath))
        {
            AppWindow.SetIcon(iconPath);
        }
    }

    private void NavigateTo(Type pageType)
    {
        if (_navFrame.CurrentSourcePageType == pageType)
        {
            return;
        }

        try
        {
            _pageErrorTextBlock.Visibility = Visibility.Collapsed;
            _navFrame.Visibility = Visibility.Visible;
            _navFrame.Navigate(pageType);
        }
        catch (Exception ex)
        {
            App.RecordStartupFailure($"Navigation to {pageType.Name} failed.", ex);
            _navFrame.Visibility = Visibility.Collapsed;
            _pageErrorTextBlock.Text = $"{pageType.Name} failed to load.{Environment.NewLine}{Environment.NewLine}{ex.Message}";
            _pageErrorTextBlock.Visibility = Visibility.Visible;
        }
    }
}
