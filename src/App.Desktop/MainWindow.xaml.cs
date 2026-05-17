using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using App.Models;
using App_Desktop.Pages;
using Microsoft.UI.Windowing;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace App_Desktop;

public sealed partial class MainWindow : Window
{
    private static readonly Color ShellBackgroundColor = Color.FromArgb(255, 248, 246, 242);
    private static readonly Color SidebarBackgroundColor = Color.FromArgb(255, 22, 22, 20);
    private static readonly Color SidebarButtonHoverColor = Color.FromArgb(255, 40, 40, 37);
    private static readonly Color SidebarButtonActiveColor = Color.FromArgb(255, 55, 55, 51);
    private static readonly Color SidebarTextColor = Color.FromArgb(255, 242, 240, 235);
    private static readonly Color SidebarMutedTextColor = Color.FromArgb(255, 164, 160, 151);
    private static readonly Color AccentColor = Color.FromArgb(255, 16, 163, 127);

    private readonly Grid _rootGrid = new();
    private readonly Frame _navFrame = new();
    private readonly Dictionary<Type, NavButtonVisuals> _navButtons = new();
    private readonly StackPanel _historyStackPanel = new() { Spacing = 6 };
    private readonly TextBlock _historySummaryTextBlock = new()
    {
        FontSize = 12,
        Foreground = new SolidColorBrush(SidebarMutedTextColor)
    };
    private readonly TextBlock _pageErrorTextBlock = new()
    {
        Margin = new Thickness(24),
        TextWrapping = TextWrapping.Wrap,
        Visibility = Visibility.Collapsed
    };
    private string? _selectedHistoryId;

    public MainWindow()
    {
        Content = _rootGrid;
        BuildShell();
        ApplySystemBackdrop();
        Title = "VisiTexta";
        ApplyWindowIcon();
        ApplyTitleBarColors();
        _rootGrid.Loaded += MainWindow_Loaded;
    }

    private void BuildShell()
    {
        _rootGrid.Background = new SolidColorBrush(ShellBackgroundColor);
        _rootGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(312) });
        _rootGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var sidebar = new Border
        {
            Padding = new Thickness(18, 18, 18, 16),
            Background = new SolidColorBrush(SidebarBackgroundColor)
        };
        Grid.SetColumn(sidebar, 0);

        var sidebarGrid = new Grid { RowSpacing = 18 };
        sidebarGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        sidebarGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        sidebarGrid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        sidebarGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        sidebarGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var brand = new Grid { ColumnSpacing = 12, Margin = new Thickness(2, 2, 2, 10) };
        brand.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        brand.ColumnDefinitions.Add(new ColumnDefinition());
        brand.Children.Add(new Border
        {
            Width = 34,
            Height = 34,
            CornerRadius = new CornerRadius(8),
            Background = new SolidColorBrush(AccentColor),
            Child = new FontIcon
            {
                Glyph = "\uE8A5",
                FontSize = 17,
                Foreground = new SolidColorBrush(Colors.White)
            }
        });

        var brandText = new StackPanel { Spacing = 0, VerticalAlignment = VerticalAlignment.Center };
        brandText.Children.Add(new TextBlock
        {
            Text = "VisiTexta",
            FontSize = 18,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(SidebarTextColor)
        });
        brandText.Children.Add(new TextBlock
        {
            Text = "Local OCR workspace",
            FontSize = 12,
            Foreground = new SolidColorBrush(SidebarMutedTextColor)
        });
        Grid.SetColumn(brandText, 1);
        brand.Children.Add(brandText);
        sidebarGrid.Children.Add(brand);

        var workspaceStack = new StackPanel { Spacing = 10 };
        Grid.SetRow(workspaceStack, 1);
        workspaceStack.Children.Add(new TextBlock
        {
            Text = "Workspace",
            FontSize = 12,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(SidebarMutedTextColor)
        });
        workspaceStack.Children.Add(CreateNavButton("New OCR", "\uE8B7", typeof(NewOcrPage), OpenNewWorkspace, true));
        sidebarGrid.Children.Add(workspaceStack);

        var historyPanel = new Border
        {
            Padding = new Thickness(12),
            CornerRadius = new CornerRadius(12),
            Background = new SolidColorBrush(Color.FromArgb(255, 32, 32, 29))
        };
        Grid.SetRow(historyPanel, 2);
        var historyGrid = new Grid { RowSpacing = 10 };
        historyGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        historyGrid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        var historyHeader = new StackPanel { Spacing = 2 };
        historyHeader.Children.Add(new TextBlock
        {
            Text = "Recent transcripts",
            FontSize = 14,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(SidebarTextColor)
        });
        historyHeader.Children.Add(_historySummaryTextBlock);
        historyGrid.Children.Add(historyHeader);

        var historyScroller = new ScrollViewer
        {
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Content = _historyStackPanel
        };
        Grid.SetRow(historyScroller, 1);
        historyGrid.Children.Add(historyScroller);
        historyPanel.Child = historyGrid;
        sidebarGrid.Children.Add(historyPanel);

        var utilityStack = new StackPanel { Spacing = 8 };
        Grid.SetRow(utilityStack, 3);
        utilityStack.Children.Add(new TextBlock
        {
            Text = "Library",
            FontSize = 12,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(SidebarMutedTextColor)
        });
        utilityStack.Children.Add(CreateNavButton("Models", "\uE8D4", typeof(ModelsPage)));
        utilityStack.Children.Add(CreateNavButton("Settings", "\uE713", typeof(SettingsPage)));
        utilityStack.Children.Add(CreateNavButton("Diagnostics", "\uE9D9", typeof(DiagnosticsPage)));
        sidebarGrid.Children.Add(utilityStack);

        var footer = new Border
        {
            CornerRadius = new CornerRadius(8),
            Background = new SolidColorBrush(Color.FromArgb(255, 32, 32, 29)),
            Padding = new Thickness(12),
            VerticalAlignment = VerticalAlignment.Bottom,
            Child = new TextBlock
            {
                Text = "Private by default. Models and OCR stay on this PC.",
                TextWrapping = TextWrapping.WrapWholeWords,
                FontSize = 12,
                Foreground = new SolidColorBrush(SidebarMutedTextColor)
            }
        };
        Grid.SetRow(footer, 4);
        sidebarGrid.Children.Add(footer);
        sidebar.Child = sidebarGrid;

        var contentGrid = new Grid
        {
            Background = new SolidColorBrush(ShellBackgroundColor)
        };
        Grid.SetColumn(contentGrid, 1);
        contentGrid.Children.Add(_navFrame);
        contentGrid.Children.Add(_pageErrorTextBlock);

        _rootGrid.Children.Add(sidebar);
        _rootGrid.Children.Add(contentGrid);
    }

    private Button CreateNavButton(string label, string glyph, Type pageType, Action? clickAction = null, bool emphasize = false)
    {
        var icon = new FontIcon
        {
            Glyph = glyph,
            Width = 18,
            FontSize = 16,
            Foreground = new SolidColorBrush(SidebarMutedTextColor)
        };
        var text = new TextBlock
        {
            Text = label,
            FontSize = 14,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(SidebarMutedTextColor),
            VerticalAlignment = VerticalAlignment.Center
        };
        var content = new Grid { ColumnSpacing = 12 };
        content.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        content.ColumnDefinitions.Add(new ColumnDefinition());
        content.Children.Add(icon);
        Grid.SetColumn(text, 1);
        content.Children.Add(text);

        var button = new Button
        {
            Content = content,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Height = emphasize ? 46 : 42,
            Padding = new Thickness(12, 0, 12, 0),
            Background = new SolidColorBrush(Colors.Transparent),
            BorderBrush = new SolidColorBrush(Colors.Transparent),
            CornerRadius = new CornerRadius(emphasize ? 10 : 8),
            Tag = pageType
        };
        AutomationProperties.SetName(button, label);
        button.Click += (_, _) =>
        {
            if (clickAction is not null)
            {
                clickAction();
            }
            else
            {
                NavigateTo(pageType);
            }
        };
        button.PointerEntered += (_, _) =>
        {
            if (_navFrame.CurrentSourcePageType != pageType)
            {
                button.Background = new SolidColorBrush(SidebarButtonHoverColor);
            }
        };
        button.PointerExited += (_, _) => UpdateActiveNavButton(_navFrame.CurrentSourcePageType);
        _navButtons[pageType] = new NavButtonVisuals(button, icon, text);
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
        await RefreshSidebarHistoryAsync();
    }

    private async void OpenNewWorkspace()
    {
        _selectedHistoryId = null;
        await RefreshSidebarHistoryAsync();

        if (_navFrame.CurrentSourcePageType == typeof(NewOcrPage) && _navFrame.Content is NewOcrPage currentPage)
        {
            await currentPage.ResetForNewRunAsync();
            UpdateActiveNavButton(typeof(NewOcrPage));
            return;
        }

        NavigateTo(typeof(NewOcrPage));
    }

    public async Task RefreshSidebarHistoryAsync(string? selectHistoryId = null)
    {
        if (!string.IsNullOrWhiteSpace(selectHistoryId))
        {
            _selectedHistoryId = selectHistoryId;
        }

        _historyStackPanel.Children.Clear();
        _historySummaryTextBlock.Text = "Loading local history...";

        var items = (await AppServices.HistoryService.GetHistoryAsync()).Take(14).ToList();
        if (items.Count == 0)
        {
            _historySummaryTextBlock.Text = "No transcripts yet";
            _historyStackPanel.Children.Add(new TextBlock
            {
                Text = "Run OCR once and recent transcripts will stay here for quick reopening.",
                TextWrapping = TextWrapping.Wrap,
                FontSize = 12,
                Foreground = new SolidColorBrush(SidebarMutedTextColor)
            });
            return;
        }

        _historySummaryTextBlock.Text = items.Count == 1 ? "1 recent transcript" : $"{items.Count} recent transcripts";
        foreach (var item in items)
        {
            _historyStackPanel.Children.Add(CreateHistoryButton(item));
        }
    }

    public async Task OpenHistoryItemAsync(string id)
    {
        var item = await AppServices.HistoryService.GetAsync(id);
        if (item is null)
        {
            await RefreshSidebarHistoryAsync();
            return;
        }

        _selectedHistoryId = id;
        await RefreshSidebarHistoryAsync();

        if (_navFrame.CurrentSourcePageType != typeof(NewOcrPage))
        {
            NavigateTo(typeof(NewOcrPage));
        }

        if (_navFrame.Content is NewOcrPage page)
        {
            await page.OpenHistoryItemAsync(item);
            UpdateActiveNavButton(typeof(NewOcrPage));
        }
    }

    private Button CreateHistoryButton(OcrHistoryItem item)
    {
        var active = string.Equals(_selectedHistoryId, item.Id, StringComparison.OrdinalIgnoreCase);
        var sourceLine = new TextBlock
        {
            Text = item.SourceName,
            FontSize = 13,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(SidebarTextColor),
            TextTrimming = TextTrimming.CharacterEllipsis
        };
        var metaLine = new TextBlock
        {
            Text = $"{item.UpdatedAt:g} • {item.WorkflowMode} • {item.Status}",
            FontSize = 11,
            Foreground = new SolidColorBrush(SidebarMutedTextColor),
            TextTrimming = TextTrimming.CharacterEllipsis
        };
        var detailLine = new TextBlock
        {
            Text = item.OutputPath is { Length: > 0 } path ? Path.GetFileName(path) : item.Error ?? "No output yet",
            FontSize = 11,
            Foreground = new SolidColorBrush(SidebarMutedTextColor),
            TextTrimming = TextTrimming.CharacterEllipsis
        };

        var button = new Button
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(10, 8, 10, 8),
            Background = new SolidColorBrush(active ? SidebarButtonActiveColor : Colors.Transparent),
            BorderBrush = new SolidColorBrush(active ? AccentColor : Colors.Transparent),
            BorderThickness = new Thickness(active ? 1 : 0),
            CornerRadius = new CornerRadius(10),
            Content = new StackPanel
            {
                Spacing = 2,
                Children = { sourceLine, metaLine, detailLine }
            }
        };
        AutomationProperties.SetName(button, $"Open transcript {item.SourceName}");
        button.Click += async (_, _) => await OpenHistoryItemAsync(item.Id);
        button.PointerEntered += (_, _) =>
        {
            if (!string.Equals(_selectedHistoryId, item.Id, StringComparison.OrdinalIgnoreCase))
            {
                button.Background = new SolidColorBrush(SidebarButtonHoverColor);
            }
        };
        button.PointerExited += (_, _) =>
        {
            var isSelected = string.Equals(_selectedHistoryId, item.Id, StringComparison.OrdinalIgnoreCase);
            button.Background = new SolidColorBrush(isSelected ? SidebarButtonActiveColor : Colors.Transparent);
        };
        return button;
    }

    private void ApplyWindowIcon()
    {
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(iconPath))
        {
            AppWindow.SetIcon(iconPath);
        }
    }

    private void ApplyTitleBarColors()
    {
        var titleBar = AppWindow.TitleBar;
        titleBar.BackgroundColor = SidebarBackgroundColor;
        titleBar.ForegroundColor = SidebarTextColor;
        titleBar.ButtonBackgroundColor = SidebarBackgroundColor;
        titleBar.ButtonForegroundColor = SidebarTextColor;
        titleBar.ButtonHoverBackgroundColor = SidebarButtonHoverColor;
        titleBar.ButtonHoverForegroundColor = SidebarTextColor;
        titleBar.ButtonPressedBackgroundColor = SidebarButtonActiveColor;
        titleBar.ButtonPressedForegroundColor = SidebarTextColor;
        titleBar.InactiveBackgroundColor = SidebarBackgroundColor;
        titleBar.InactiveForegroundColor = SidebarMutedTextColor;
        titleBar.ButtonInactiveBackgroundColor = SidebarBackgroundColor;
        titleBar.ButtonInactiveForegroundColor = SidebarMutedTextColor;
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
            UpdateActiveNavButton(pageType);
        }
        catch (Exception ex)
        {
            App.RecordStartupFailure($"Navigation to {pageType.Name} failed.", ex);
            _navFrame.Visibility = Visibility.Collapsed;
            _pageErrorTextBlock.Text = $"{pageType.Name} failed to load.{Environment.NewLine}{Environment.NewLine}{ex.Message}";
            _pageErrorTextBlock.Visibility = Visibility.Visible;
        }
    }

    private void UpdateActiveNavButton(Type? activePageType)
    {
        foreach (var pair in _navButtons)
        {
            var active = pair.Key == activePageType;
            pair.Value.Button.Background = new SolidColorBrush(active ? SidebarButtonActiveColor : Colors.Transparent);
            pair.Value.Button.BorderBrush = new SolidColorBrush(Colors.Transparent);
            pair.Value.Icon.Foreground = new SolidColorBrush(active ? AccentColor : SidebarMutedTextColor);
            pair.Value.Label.Foreground = new SolidColorBrush(active ? SidebarTextColor : SidebarMutedTextColor);
        }
    }

    private sealed record NavButtonVisuals(Button Button, FontIcon Icon, TextBlock Label);
}
