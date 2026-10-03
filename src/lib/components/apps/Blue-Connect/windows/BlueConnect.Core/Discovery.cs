using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace BlueConnect.Core;

/// <summary>
/// UDP side of Blue Connect.
///
///  * The listener (bound to the discovery port) records every identity it
///    hears and answers with our own identity, unicast to the sender's source
///    port — that is exactly what the desktop's <c>bc_start_discovery</c>
///    waits for (it broadcasts from an ephemeral port and reads the replies).
///  * A scan broadcasts our identity from an ephemeral port and collects the
///    replies of other Blue Connect clients.
///
/// Reply storms are avoided by never answering a datagram whose source port is
/// the discovery port itself (that is another listener's reply, not a scan).
/// </summary>
internal sealed class Discovery : IDisposable
{
    private readonly ConnectOptions _opt;
    private readonly Func<string> _ownId;
    private readonly Func<byte[]> _ownIdentity;
    private readonly Action<Protocol.Identity, IPAddress> _onSeen;
    private readonly Action<string> _log;
    private UdpClient? _listener;

    public Discovery(ConnectOptions opt, Func<string> ownId, Func<byte[]> ownIdentity,
        Action<Protocol.Identity, IPAddress> onSeen, Action<string> log)
    {
        _opt = opt; _ownId = ownId; _ownIdentity = ownIdentity; _onSeen = onSeen; _log = log;
    }

    private static void DisableConnReset(UdpClient c)
    {
        // Windows reports ICMP "port unreachable" from an earlier send as a
        // ConnectionReset on the NEXT receive; that would kill the loop.
        if (OperatingSystem.IsWindows())
        {
            try { c.Client.IOControl((IOControlCode)(-1744830452), new byte[] { 0 }, null); } catch { /* best effort */ }
        }
    }

    public async Task RunListenerAsync(CancellationToken ct)
    {
        UdpClient udp;
        try
        {
            udp = new UdpClient(AddressFamily.InterNetwork);
            udp.Client.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
            udp.Client.Bind(new IPEndPoint(_opt.BindAddress, _opt.DiscoveryPort));
            udp.EnableBroadcast = true;
            DisableConnReset(udp);
        }
        catch (Exception e)
        {
            _log($"Discovery listener could not start on UDP {_opt.DiscoveryPort}: {e.Message} " +
                 "(another KDE Connect / Blue Connect instance, or a firewall?). Scanning still works.");
            return;
        }
        _listener = udp;

        while (!ct.IsCancellationRequested)
        {
            UdpReceiveResult r;
            try { r = await udp.ReceiveAsync(ct).ConfigureAwait(false); }
            catch (OperationCanceledException) { break; }
            catch (ObjectDisposedException) { break; }
            catch (SocketException) { continue; }

            var id = Protocol.ParseIdentity(r.Buffer);
            if (id is null || id.DeviceId == _ownId()) continue;

            _onSeen(id, r.RemoteEndPoint.Address);

            if (r.RemoteEndPoint.Port != _opt.DiscoveryPort)
            {
                try
                {
                    var me = _ownIdentity();
                    await udp.SendAsync(me, me.Length, r.RemoteEndPoint).ConfigureAwait(false);
                }
                catch (Exception e) { _log($"Could not answer discovery from {r.RemoteEndPoint}: {e.Message}"); }
            }
        }
    }

    private IEnumerable<IPAddress> Targets()
    {
        if (_opt.DiscoveryTargets is { Count: > 0 }) return _opt.DiscoveryTargets;

        var list = new List<IPAddress> { IPAddress.Broadcast };
        try
        {
            foreach (var nic in NetworkInterface.GetAllNetworkInterfaces())
            {
                if (nic.OperationalStatus != OperationalStatus.Up || nic.NetworkInterfaceType == NetworkInterfaceType.Loopback) continue;
                foreach (var ua in nic.GetIPProperties().UnicastAddresses)
                {
                    if (ua.Address.AddressFamily != AddressFamily.InterNetwork || ua.IPv4Mask is null) continue;
                    var ip = ua.Address.GetAddressBytes();
                    var mask = ua.IPv4Mask.GetAddressBytes();
                    var b = new byte[4];
                    for (int i = 0; i < 4; i++) b[i] = (byte)(ip[i] | ~mask[i]);
                    var bc = new IPAddress(b);
                    if (!list.Contains(bc)) list.Add(bc);
                }
            }
        }
        catch { /* limited broadcast alone still works on most LANs */ }
        return list;
    }

    /// <summary>Broadcast our identity and gather replies for <paramref name="duration"/>.</summary>
    public async Task ScanAsync(TimeSpan duration, CancellationToken ct)
    {
        using var udp = new UdpClient(new IPEndPoint(_opt.BindAddress, 0)) { EnableBroadcast = true };
        DisableConnReset(udp);

        var me = _ownIdentity();
        foreach (var target in Targets())
        {
            try { await udp.SendAsync(me, me.Length, new IPEndPoint(target, _opt.ScanTargetPort ?? _opt.DiscoveryPort)).ConfigureAwait(false); }
            catch (Exception e) { _log($"Broadcast to {target} failed: {e.Message}"); }
        }

        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        cts.CancelAfter(duration);
        while (!cts.IsCancellationRequested)
        {
            try
            {
                var r = await udp.ReceiveAsync(cts.Token).ConfigureAwait(false);
                var id = Protocol.ParseIdentity(r.Buffer);
                if (id is not null && id.DeviceId != _ownId()) _onSeen(id, r.RemoteEndPoint.Address);
            }
            catch (OperationCanceledException) { break; }
            catch (SocketException) { /* keep listening until the window ends */ }
        }
    }

    public void Dispose() => _listener?.Dispose();
}
