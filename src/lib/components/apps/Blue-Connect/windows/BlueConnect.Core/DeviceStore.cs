using System.Text.Json;
using System.Text.Json.Serialization;

namespace BlueConnect.Core;

/// <summary>
/// A device we have seen or paired with. Field names/JSON shape are identical
/// to the desktop's <c>DiscoveredDevice</c> so a <c>devices.json</c> is
/// interchangeable between implementations.
/// </summary>
public sealed class DeviceInfo
{
    [JsonPropertyName("id")] public string Id { get; set; } = "";
    [JsonPropertyName("name")] public string Name { get; set; } = "";
    [JsonPropertyName("deviceType")] public string DeviceType { get; set; } = "unknown";
    [JsonPropertyName("address")] public string Address { get; set; } = "";
    [JsonPropertyName("tcpPort")] public int TcpPort { get; set; } = Protocol.PairingTcpPort;
    [JsonPropertyName("paired")] public bool Paired { get; set; }
    [JsonPropertyName("pinnedCertSha256")] public string? PinnedCertSha256 { get; set; }

    public DeviceInfo Clone() => (DeviceInfo)MemberwiseClone();
}

/// <summary>Thread-safe, persisted registry of known devices (<c>devices.json</c>).</summary>
public sealed class DeviceStore
{
    private static readonly JsonSerializerOptions JsonOpts = new() { WriteIndented = true };

    private readonly object _gate = new();
    private readonly string _path;
    private readonly Dictionary<string, DeviceInfo> _devices;

    public DeviceStore(string dataDir)
    {
        Directory.CreateDirectory(dataDir);
        _path = Path.Combine(dataDir, "devices.json");
        _devices = Load(_path);
    }

    private static Dictionary<string, DeviceInfo> Load(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                var d = JsonSerializer.Deserialize<Dictionary<string, DeviceInfo>>(File.ReadAllText(path));
                if (d is not null) return d;
            }
        }
        catch
        {
            // Unreadable file: start empty rather than crash on startup.
        }
        return new Dictionary<string, DeviceInfo>();
    }

    private void Save()
    {
        var tmp = _path + ".tmp";
        File.WriteAllText(tmp, JsonSerializer.Serialize(_devices, JsonOpts));
        File.Move(tmp, _path, overwrite: true);
    }

    public DeviceInfo? Get(string id)
    {
        lock (_gate) return _devices.TryGetValue(id, out var d) ? d.Clone() : null;
    }

    public DeviceInfo? FindPairedByFingerprint(string fingerprint)
    {
        lock (_gate)
            return _devices.Values.FirstOrDefault(d => d.Paired && d.PinnedCertSha256 == fingerprint)?.Clone();
    }

    /// <summary>Paired devices first, then by name — same order as the desktop's list.</summary>
    public List<DeviceInfo> All()
    {
        lock (_gate)
            return _devices.Values.Select(d => d.Clone())
                .OrderByDescending(d => d.Paired).ThenBy(d => d.Name, StringComparer.Ordinal).ToList();
    }

    /// <summary>Records what a discovery datagram told us; never touches pairing state.</summary>
    public void Seen(string id, string name, string type, string address, int tcpPort)
    {
        lock (_gate)
        {
            if (_devices.TryGetValue(id, out var d))
            {
                d.Name = name; d.DeviceType = type; d.Address = address; d.TcpPort = tcpPort;
            }
            else
            {
                _devices[id] = new DeviceInfo { Id = id, Name = name, DeviceType = type, Address = address, TcpPort = tcpPort };
            }
            Save();
        }
    }

    /// <summary>Marks a device paired and pins the certificate it presented.</summary>
    public void MarkPaired(string id, string fallbackName, string address, int tcpPort, string fingerprint)
    {
        lock (_gate)
        {
            if (!_devices.TryGetValue(id, out var d))
            {
                d = new DeviceInfo { Id = id, Name = fallbackName, DeviceType = "unknown", Address = address, TcpPort = tcpPort };
                _devices[id] = d;
            }
            d.Paired = true;
            d.PinnedCertSha256 = fingerprint;
            Save();
        }
    }

    public void UpdateAddress(string id, string address)
    {
        lock (_gate)
        {
            if (_devices.TryGetValue(id, out var d) && d.Address != address)
            {
                d.Address = address;
                Save();
            }
        }
    }

    public bool Forget(string id)
    {
        lock (_gate)
        {
            var removed = _devices.Remove(id);
            if (removed) Save();
            return removed;
        }
    }
}
