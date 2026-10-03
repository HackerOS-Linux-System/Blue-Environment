using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Forms;
using BlueConnect.Core;

namespace BlueConnect.App;

public partial class App : System.Windows.Application
{
    private Mutex? _singleInstance;
    private ConnectService? _service;
    private MainWindow? _window;
    private NotifyIcon? _tray;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        _singleInstance = new Mutex(true, "BlueConnect.SingleInstance", out var firstInstance);
        if (!firstInstance)
        {
            System.Windows.MessageBox.Show("Blue Connect is already running (look for its icon in the system tray).", "Blue Connect");
            Shutdown();
            return;
        }

        var dataDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Blue-Environment", "Connect");
        try
        {
            _service = new ConnectService(new ConnectOptions
            {
                DataDir = dataDir,
                DeviceName = Environment.MachineName,
                DeviceType = "laptop",
            });
            _service.ConfirmPairing = ConfirmPairingOnUiThread;
            _service.PingReceived += (dev, msg) => Balloon($"Ping from {dev.Name}", string.IsNullOrWhiteSpace(msg) ? "Ping!" : msg);
            _service.Start();
        }
        catch (Exception ex)
        {
            System.Windows.MessageBox.Show(
                "Blue Connect could not start its network listener:\n\n" + ex.Message +
                "\n\nAnother program (for example KDE Connect) may be using TCP 1717 / UDP 1716.",
                "Blue Connect", MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown();
            return;
        }

        _window = new MainWindow();
        _window.Attach(_service);

        _tray = new NotifyIcon { Icon = TrayIconFactory.Create(), Text = "Blue Connect", Visible = true };
        _tray.DoubleClick += (_, _) => ShowWindow();
        var menu = new ContextMenuStrip();
        menu.Items.Add("Open Blue Connect", null, (_, _) => ShowWindow());
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Quit", null, (_, _) => Quit());
        _tray.ContextMenuStrip = menu;

        // "--tray" starts hidden (handy for autostart); otherwise show the window.
        if (Array.IndexOf(e.Args, "--tray") < 0) ShowWindow();
    }

    private void ShowWindow()
    {
        if (_window is null) return;
        _window.Show();
        if (_window.WindowState == WindowState.Minimized) _window.WindowState = WindowState.Normal;
        _window.Activate();
    }

    private void Balloon(string title, string text)
    {
        Dispatcher.BeginInvoke(new Action(() => _tray?.ShowBalloonTip(5000, title, text, ToolTipIcon.Info)));
    }

    /// <summary>Called by the core on a background thread; shows the SAS dialog on the UI thread.</summary>
    private Task<bool> ConfirmPairingOnUiThread(IncomingPairingRequest request, CancellationToken expired)
    {
        var tcs = new TaskCompletionSource<bool>();
        Dispatcher.BeginInvoke(new Action(() =>
        {
            Balloon("Pairing request", $"{request.DeviceName} wants to pair — code {request.Sas}");
            var dialog = new PairingDialog(request);
            using var reg = expired.Register(() => Dispatcher.BeginInvoke(new Action(() => dialog.Close())));
            dialog.ShowDialog();
            tcs.TrySetResult(dialog.Accepted);
        }));
        return tcs.Task;
    }

    private void Quit()
    {
        if (_window is not null) _window.Quitting = true;
        _tray?.Dispose();
        _service?.Dispose();
        _window?.Close();
        _singleInstance?.ReleaseMutex();
        Shutdown();
    }
}
