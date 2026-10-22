using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Media;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Forms;
using BlueConnect.Core;

namespace BlueConnect.App;

/// <summary>
/// Windows implementation of <see cref="IPluginHost"/>: lets a paired phone read this PC's
/// battery, make it ring, push clipboard text, open a link here or drop text into a balloon.
/// Everything that touches WPF/WinForms objects is marshalled to the UI thread.
/// </summary>
public sealed class WindowsPluginHost : IPluginHost
{
    private readonly System.Windows.Threading.Dispatcher _ui;
    private readonly Action<string, string> _balloon;
    private int _ringing;

    public WindowsPluginHost(System.Windows.Threading.Dispatcher ui, Action<string, string> balloon)
    {
        _ui = ui;
        _balloon = balloon;
    }

    public BatteryStatus? GetBattery()
    {
        var p = SystemInformation.PowerStatus;
        // BatteryChargeStatus.NoSystemBattery → a desktop PC
        if ((p.BatteryChargeStatus & BatteryChargeStatus.NoSystemBattery) != 0) return null;
        float life = p.BatteryLifePercent;
        if (life < 0 || life > 1) return null;
        return new BatteryStatus((int)Math.Round(life * 100), p.PowerLineStatus == PowerLineStatus.Online);
    }

    /// <summary>Plays the alert sound in a loop for ~15 s and flashes the window, unless it is already ringing.</summary>
    public void Ring()
    {
        if (Interlocked.Exchange(ref _ringing, 1) == 1) return;
        _balloon("Find my device", "A paired device is looking for this PC.");
        _ = Task.Run(async () =>
        {
            try
            {
                for (int i = 0; i < 15; i++)
                {
                    SystemSounds.Exclamation.Play();
                    await Task.Delay(1000).ConfigureAwait(false);
                }
            }
            finally { Interlocked.Exchange(ref _ringing, 0); }
        });
    }

    public void SetClipboard(string text) => OnUi(() =>
    {
        try { System.Windows.Clipboard.SetText(text); }
        catch (COMException) { /* another app holds the clipboard right now — drop this update */ }
    });

    public string? GetClipboard() => OnUi(() =>
    {
        try { return System.Windows.Clipboard.ContainsText() ? System.Windows.Clipboard.GetText() : null; }
        catch (COMException) { return null; }
    });

    /// <summary>The core has already restricted this to http/https/mailto/tel.</summary>
    public void OpenUrl(string url)
    {
        try { Process.Start(new ProcessStartInfo(url) { UseShellExecute = true }); }
        catch (Exception ex) { _balloon("Could not open link", ex.Message); }
    }

    public void ReceiveText(string text, DeviceInfo from)
    {
        SetClipboard(text);
        var preview = text.Length > 120 ? text[..120] + "…" : text;
        _balloon($"Text from {from.Name}", preview + "\n(copied to the clipboard)");
    }

    // Windows exposes no API to read other apps' toast notifications without a packaged app + user consent.
    public IReadOnlyList<NotificationInfo> GetNotifications() => Array.Empty<NotificationInfo>();

    private void OnUi(Action a) => _ui.Invoke(a);
    private T OnUi<T>(Func<T> f) => _ui.Invoke(f);
}
