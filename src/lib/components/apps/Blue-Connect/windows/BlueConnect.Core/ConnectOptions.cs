using System.Net;

namespace BlueConnect.Core;

public sealed class ConnectOptions
{
    /// <summary>Folder for device_id, devices.json and the TLS identity.</summary>
    public required string DataDir { get; init; }

    public string DeviceName { get; init; } = Environment.MachineName;

    /// <summary>phone | tablet | desktop | laptop | tv</summary>
    public string DeviceType { get; init; } = "desktop";

    /// <summary>UDP discovery port (KDE Connect's real port by default).</summary>
    public int DiscoveryPort { get; init; } = Protocol.DiscoveryPort;

    /// <summary>TCP port of the mutual-TLS listener; advertised as tcpPort in our identity.</summary>
    public int TcpPort { get; init; } = Protocol.PairingTcpPort;

    /// <summary>UDP port scans send to. Null = <see cref="DiscoveryPort"/>. Only tests need to differ.</summary>
    public int? ScanTargetPort { get; init; }

    /// <summary>Where scans send the identity datagram. Null = broadcast + every interface's directed broadcast.</summary>
    public IReadOnlyList<IPAddress>? DiscoveryTargets { get; init; }

    public IPAddress BindAddress { get; init; } = IPAddress.Any;
}
