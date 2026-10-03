using System;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;
using BlueConnect.Core;

namespace BlueConnect.App;

public partial class MainWindow : Window
{
    private ConnectService? _service;

    /// <summary>True when the user really wants to quit (tray menu); otherwise closing just hides to the tray.</summary>
    public bool Quitting { get; set; }

    public MainWindow()
    {
        InitializeComponent();
    }

    public void Attach(ConnectService service)
    {
        _service = service;
        SubtitleText.Text = $"Visible as “{service.DeviceName}” on this network";
        FingerprintText.Text = $"This computer's certificate: {service.Fingerprint[..16]}…";
        service.DevicesChanged += () => Dispatcher.BeginInvoke(new Action(Refresh));
        service.Log += msg => Dispatcher.BeginInvoke(new Action(() => StatusText.Text = msg));
        service.OutgoingPairingSas += info => Dispatcher.BeginInvoke(new Action(() =>
        {
            OutgoingTitle.Text = $"Confirm on “{info.DeviceName}”";
            OutgoingSas.Text = info.Sas.Length == 6 ? $"{info.Sas[..3]} {info.Sas[3..]}" : info.Sas;
            OutgoingBanner.Visibility = Visibility.Visible;
        }));
        Refresh();
    }

    private void Refresh()
    {
        if (_service is null) return;
        DevicesList.ItemsSource = _service.Devices.Select(d => new DeviceRow(d)).ToList();
    }

    protected override void OnClosing(System.ComponentModel.CancelEventArgs e)
    {
        if (!Quitting)
        {
            e.Cancel = true; // keep listening in the tray
            Hide();
        }
        base.OnClosing(e);
    }

    private static string IdOf(object sender) => (string)((FrameworkElement)sender).Tag;

    private async void OnScan(object sender, RoutedEventArgs e)
    {
        if (_service is null) return;
        ScanButton.IsEnabled = false;
        ScanButton.Content = "Scanning…";
        try
        {
            await _service.ScanAsync(TimeSpan.FromSeconds(3));
            Refresh();
            StatusText.Text = _service.Devices.Count == 0
                ? "No devices found. Make sure Blue Connect is open on the other device and both are on the same network."
                : $"{_service.Devices.Count} device(s) known.";
        }
        catch (Exception ex)
        {
            StatusText.Text = "Scan failed: " + ex.Message;
        }
        finally
        {
            ScanButton.Content = "Scan for devices";
            ScanButton.IsEnabled = true;
        }
    }

    private async void OnPair(object sender, RoutedEventArgs e)
    {
        if (_service is null) return;
        var id = IdOf(sender);
        try
        {
            await _service.PairAsync(id);
            StatusText.Text = "Paired.";
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
        finally
        {
            OutgoingBanner.Visibility = Visibility.Collapsed;
            Refresh();
        }
    }

    private async void OnPing(object sender, RoutedEventArgs e)
    {
        if (_service is null) return;
        try
        {
            await _service.PingAsync(IdOf(sender), $"Hello from {_service.DeviceName}");
            StatusText.Text = "Ping sent.";
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
    }

    private void OnSms(object sender, RoutedEventArgs e)
    {
        if (_service is null) return;
        var phone = _service.Devices.FirstOrDefault(d => d.Id == IdOf(sender));
        if (phone is null) return;
        new SmsWindow(_service, phone) { Owner = this }.Show();
    }

    private void OnForget(object sender, RoutedEventArgs e)
    {
        if (_service is null) return;
        var id = IdOf(sender);
        var name = _service.Devices.FirstOrDefault(d => d.Id == id)?.Name ?? "this device";
        var answer = MessageBox.Show(this, $"Forget {name}? You will have to pair again to use it.", "Blue Connect",
            MessageBoxButton.YesNo, MessageBoxImage.Question);
        if (answer == MessageBoxResult.Yes) _service.Forget(id);
    }
}
