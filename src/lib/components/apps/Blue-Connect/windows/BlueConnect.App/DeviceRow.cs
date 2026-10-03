using BlueConnect.Core;

namespace BlueConnect.App;

/// <summary>Display model for one device in the list (rebuilt on every change — the list is tiny).</summary>
public sealed class DeviceRow
{
    public string Id { get; }
    public string Name { get; }
    public string Subtitle { get; }
    public bool IsPaired { get; }
    public bool IsUnpaired => !IsPaired;
    /// <summary>Paired phones/tablets can be used as an SMS gateway.</summary>
    public bool IsPhone { get; }

    public DeviceRow(DeviceInfo d)
    {
        Id = d.Id;
        Name = d.Name;
        IsPaired = d.Paired;
        IsPhone = d.Paired && (d.DeviceType == "phone" || d.DeviceType == "tablet");
        Subtitle = $"{d.DeviceType} · {d.Address}:{d.TcpPort}" + (d.Paired ? " · paired" : "");
    }
}
