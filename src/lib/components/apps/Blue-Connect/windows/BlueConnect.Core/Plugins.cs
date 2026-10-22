using System.Text;
using System.Text.Json.Nodes;

namespace BlueConnect.Core;

public sealed record BatteryStatus(int Percent, bool Charging);
public sealed record NotificationInfo(string Id, string App, string Title, string Text, long TimeMs);

/// <summary>
/// What a platform has to provide so a paired device can use it: battery, "find my device"
/// ringing, clipboard, opening a shared link / receiving shared text, and (where the OS
/// allows it) the list of current notifications. Everything is optional — return null / do
/// nothing when the platform cannot do it.
/// </summary>
public interface IPluginHost
{
    BatteryStatus? GetBattery();
    /// <summary>Make the device audibly/visibly attract attention (like "Find my phone").</summary>
    void Ring();
    void SetClipboard(string text);
    string? GetClipboard();
    /// <summary>Open an already validated http/https/mailto/tel URL.</summary>
    void OpenUrl(string url);
    /// <summary>Text pushed from another device ("Share → Blue Connect").</summary>
    void ReceiveText(string text, DeviceInfo from);
    IReadOnlyList<NotificationInfo> GetNotifications();
}

/// <summary>Packet builders / parsers / limits for the plugin protocol. Pure functions — unit-tested.</summary>
public static class PluginPackets
{
    public const int MaxClipboardBytes = 256 * 1024;
    public const int MaxSharedTextBytes = 64 * 1024;
    public const int MaxUrlLength = 2048;

    public static byte[] BatteryRequest() => Protocol.Frame(Protocol.TypeBatteryRequest, new JsonObject { ["request"] = true });
    public static byte[] FindMyPhone() => Protocol.Frame(Protocol.TypeFindMyPhone, new JsonObject());
    public static byte[] ClipboardRequest() => Protocol.Frame(Protocol.TypeClipboardRequest, new JsonObject());
    public static byte[] NotificationRequest() => Protocol.Frame(Protocol.TypeNotificationRequest, new JsonObject { ["request"] = true });

    public static byte[] Clipboard(string content) =>
        Protocol.Frame(Protocol.TypeClipboard, new JsonObject { ["content"] = FitUtf8(content, MaxClipboardBytes) });

    public static byte[] ShareText(string text) =>
        Protocol.Frame(Protocol.TypeShare, new JsonObject { ["text"] = FitUtf8(text, MaxSharedTextBytes) });

    public static byte[] ShareUrl(string url) =>
        Protocol.Frame(Protocol.TypeShare, new JsonObject { ["url"] = url });

    /// <summary>-1 means "this device has no battery" (desktops).</summary>
    public static byte[] Battery(BatteryStatus? b) => Protocol.Frame(Protocol.TypeBattery, new JsonObject
    {
        ["currentCharge"] = b is null ? -1 : Math.Clamp(b.Percent, 0, 100),
        ["isCharging"] = b?.Charging ?? false,
        ["thresholdEvent"] = 0,
    });

    /// <summary>Truncates to at most <paramref name="maxBytes"/> of UTF-8 without cutting a character in half.</summary>
    public static string FitUtf8(string s, int maxBytes)
    {
        if (Encoding.UTF8.GetByteCount(s) <= maxBytes) return s;
        var bytes = Encoding.UTF8.GetBytes(s);
        int len = maxBytes;
        while (len > 0 && (bytes[len] & 0xC0) == 0x80) len--; // step back to a character boundary
        return Encoding.UTF8.GetString(bytes, 0, len);
    }

    /// <summary>Only plain web / mail / phone links may be opened on request of another device.</summary>
    public static bool IsSafeUrl(string? url)
    {
        if (string.IsNullOrWhiteSpace(url) || url.Length > MaxUrlLength) return false;
        if (url.Any(c => char.IsControl(c) || char.IsWhiteSpace(c))) return false;
        if (!Uri.TryCreate(url, UriKind.Absolute, out var u)) return false;
        return u.Scheme is "http" or "https" or "mailto" or "tel";
    }

    /// <summary>Notification list packet, dropping entries (oldest last) until it fits one read.</summary>
    public static byte[] NotificationList(IReadOnlyList<NotificationInfo> items)
    {
        for (int n = items.Count; n >= 0; n--)
        {
            var arr = new JsonArray();
            foreach (var i in items.Take(n))
                arr.Add(new JsonObject
                {
                    ["id"] = i.Id, ["appName"] = FitUtf8(i.App, 80), ["title"] = FitUtf8(i.Title, 200),
                    ["text"] = FitUtf8(i.Text, 400), ["time"] = i.TimeMs,
                });
            var frame = Protocol.Frame(Protocol.TypeNotificationList, new JsonObject { ["notifications"] = arr });
            if (frame.Length <= Protocol.MaxSingleReadReplyBytes) return frame;
        }
        return Protocol.Frame(Protocol.TypeNotificationList, new JsonObject { ["notifications"] = new JsonArray() });
    }

    public static BatteryStatus? ParseBattery(Protocol.Packet p)
    {
        if (p.Type != Protocol.TypeBattery) return null;
        int charge = p.Body["currentCharge"]?.GetValue<int>() ?? -1;
        if (charge < 0) return null;
        return new BatteryStatus(Math.Min(charge, 100), p.Body["isCharging"]?.GetValue<bool>() ?? false);
    }

    public static IReadOnlyList<NotificationInfo> ParseNotifications(Protocol.Packet p)
    {
        var list = new List<NotificationInfo>();
        if (p.Type != Protocol.TypeNotificationList || p.Body["notifications"] is not JsonArray arr) return list;
        foreach (var n in arr)
        {
            if (n is not JsonObject o) continue;
            list.Add(new NotificationInfo(
                o["id"]?.ToString() ?? "", o["appName"]?.ToString() ?? "", o["title"]?.ToString() ?? "",
                o["text"]?.ToString() ?? "", o["time"]?.GetValue<long>() ?? 0));
        }
        return list;
    }
}

/// <summary>Routes plugin packets from a PAIRED device to an <see cref="IPluginHost"/>.</summary>
public sealed class PluginRouter
{
    private readonly IPluginHost _host;
    public PluginRouter(IPluginHost host) => _host = host;

    /// <returns>true when the packet was a plugin packet (handled or deliberately dropped).</returns>
    public async Task<bool> HandleAsync(DeviceInfo from, Protocol.Packet pkt, Func<byte[], Task> reply)
    {
        switch (pkt.Type)
        {
            case Protocol.TypeBatteryRequest:
                await reply(PluginPackets.Battery(_host.GetBattery())).ConfigureAwait(false);
                return true;

            case Protocol.TypeFindMyPhone:
                _host.Ring();
                return true;

            case Protocol.TypeClipboard:
            {
                var text = pkt.Body["content"]?.GetValue<string>();
                if (text is not null && Encoding.UTF8.GetByteCount(text) <= PluginPackets.MaxClipboardBytes)
                    _host.SetClipboard(text);
                return true;
            }

            case Protocol.TypeClipboardRequest:
            {
                var text = PluginPackets.FitUtf8(_host.GetClipboard() ?? "", Protocol.MaxSingleReadReplyBytes - 200);
                await reply(Protocol.Frame(Protocol.TypeClipboard, new JsonObject { ["content"] = text })).ConfigureAwait(false);
                return true;
            }

            case Protocol.TypeShare:
            {
                var url = pkt.Body["url"]?.GetValue<string>();
                var text = pkt.Body["text"]?.GetValue<string>();
                if (url is not null) { if (PluginPackets.IsSafeUrl(url)) _host.OpenUrl(url); }
                else if (!string.IsNullOrEmpty(text) && Encoding.UTF8.GetByteCount(text) <= PluginPackets.MaxSharedTextBytes)
                    _host.ReceiveText(text, from);
                return true;
            }

            case Protocol.TypeNotificationRequest:
                await reply(PluginPackets.NotificationList(_host.GetNotifications())).ConfigureAwait(false);
                return true;

            default:
                return false;
        }
    }

    /// <summary>
    /// A <see cref="ConnectService.PacketHandler"/> that serves the plugins first and passes
    /// everything else (e.g. the SMS gateway) on to <paramref name="next"/>.
    /// </summary>
    public Func<DeviceInfo, Protocol.Packet, Func<byte[], Task>, Task> AsPacketHandler(
        Func<DeviceInfo, Protocol.Packet, Func<byte[], Task>, Task>? next = null) =>
        async (dev, pkt, reply) =>
        {
            if (await HandleAsync(dev, pkt, reply).ConfigureAwait(false)) return;
            if (next is not null) await next(dev, pkt, reply).ConfigureAwait(false);
        };
}
