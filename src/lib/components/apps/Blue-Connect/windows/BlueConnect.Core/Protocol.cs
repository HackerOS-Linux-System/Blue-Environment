using System.Text;
using System.Text.Json.Nodes;

namespace BlueConnect.Core;

/// <summary>
/// Wire constants + packet helpers. Mirrors the desktop implementation in
/// <c>src-tauri/src/BlueConnect/mod.rs</c> exactly — if you change anything
/// here, that file (and the Android/macOS clients) must change with it.
///
/// Transport summary:
///  * Discovery: one UDP datagram holding a <c>kdeconnect.identity</c> JSON
///    document (no trailing newline), port 1716.
///  * Everything else: mutual TLS over TCP (port from the identity's tcpPort,
///    1717 by default), newline-terminated JSON packets. Both sides present a
///    self-signed certificate; trust is decided by pinning the certificate's
///    SHA-256 fingerprint after a Short-Authentication-String confirmation.
/// </summary>
public static class Protocol
{
    public const int DiscoveryPort = 1716;
    public const int PairingTcpPort = 1717;
    public const int ProtocolVersion = 7;

    public const string TypeIdentity = "kdeconnect.identity";
    public const string TypePair = "kdeconnect.pair";
    public const string TypePing = "kdeconnect.ping";
    public const string TypeSmsRequest = "kdeconnect.sms.request";
    public const string TypeSmsMessages = "kdeconnect.sms.messages";

    /// <summary>How long either side waits for the other person to answer a pairing request.</summary>
    public static readonly TimeSpan PairingDecisionTimeout = TimeSpan.FromSeconds(120);

    /// <summary>Upper bound for one packet we are willing to buffer.</summary>
    public const int MaxPacketBytes = 1024 * 1024;

    public static readonly string[] DeviceTypes = { "phone", "tablet", "desktop", "laptop", "tv", "unknown" };

    public static string NormalizeDeviceType(string? t)
    {
        t = (t ?? "").Trim().ToLowerInvariant();
        return Array.IndexOf(DeviceTypes, t) >= 0 ? t : "unknown";
    }

    /// <summary>Builds the discovery datagram (no trailing newline — matches the desktop).</summary>
    public static byte[] BuildIdentity(string deviceId, string deviceName, string deviceType, int tcpPort)
    {
        var body = new JsonObject
        {
            ["deviceId"] = deviceId,
            ["deviceName"] = deviceName,
            ["deviceType"] = NormalizeDeviceType(deviceType),
            ["protocolVersion"] = ProtocolVersion,
            ["tcpPort"] = tcpPort,
            // Extra fields: ignored by the desktop (serde ignores unknown keys),
            // but let real KDE Connect negotiate plugins.
            ["incomingCapabilities"] = new JsonArray(TypePair, TypePing, TypeSmsRequest),
            ["outgoingCapabilities"] = new JsonArray(TypePair, TypePing, TypeSmsMessages),
        };
        var packet = new JsonObject { ["type"] = TypeIdentity, ["body"] = body };
        return Encoding.UTF8.GetBytes(packet.ToJsonString());
    }

    /// <summary>One framed packet: compact JSON + '\n'. Always written with a single Write call.</summary>
    public static byte[] Frame(string type, JsonObject body)
    {
        var packet = new JsonObject { ["type"] = type, ["body"] = body };
        return Encoding.UTF8.GetBytes(packet.ToJsonString() + "\n");
    }

    public static byte[] PairRequest(string deviceId) =>
        Frame(TypePair, new JsonObject { ["pair"] = true, ["deviceId"] = deviceId });

    public static byte[] PairResponse(bool accepted) =>
        Frame(TypePair, new JsonObject { ["pair"] = accepted });

    public sealed record Packet(string Type, JsonObject Body);

    /// <summary>Parses one JSON packet; returns null for anything that is not a packet object.</summary>
    public static Packet? Parse(string json)
    {
        try
        {
            if (JsonNode.Parse(json) is not JsonObject root) return null;
            var type = root["type"]?.GetValue<string>();
            if (string.IsNullOrEmpty(type)) return null;
            var body = root["body"] as JsonObject ?? new JsonObject();
            return new Packet(type, body);
        }
        catch
        {
            return null;
        }
    }

    public sealed record Identity(string DeviceId, string DeviceName, string DeviceType, int ProtocolVersion, int TcpPort);

    /// <summary>
    /// Parses a discovery datagram. Same strictness as the desktop's serde
    /// struct: every field is mandatory, otherwise the datagram is ignored.
    /// </summary>
    public static Identity? ParseIdentity(ReadOnlySpan<byte> datagram)
    {
        var p = Parse(Encoding.UTF8.GetString(datagram));
        if (p is null || p.Type != TypeIdentity) return null;
        try
        {
            var b = p.Body;
            var id = b["deviceId"]?.GetValue<string>();
            var name = b["deviceName"]?.GetValue<string>();
            var type = b["deviceType"]?.GetValue<string>();
            var ver = b["protocolVersion"]?.GetValue<int>();
            var port = b["tcpPort"]?.GetValue<int>();
            if (string.IsNullOrWhiteSpace(id) || name is null || type is null || ver is null || port is null) return null;
            if (port < 1 || port > 65535) return null;
            return new Identity(id, name, NormalizeDeviceType(type), ver.Value, port.Value);
        }
        catch
        {
            return null;
        }
    }
}
