using System.Net;
using System.Net.Security;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text.Json.Nodes;

namespace BlueConnect.Core;

public sealed class ConnectException : Exception
{
    public ConnectException(string message, Exception? inner = null) : base(message, inner) { }
}

/// <summary>A pairing request from another device, waiting for this person's yes/no.</summary>
public sealed record IncomingPairingRequest(string DeviceId, string DeviceName, string Address, string Sas);

/// <summary>Shown while WE are the one asking: the code to compare with the other screen.</summary>
public sealed record OutgoingPairingSas(string DeviceId, string DeviceName, string Sas);

public sealed record SmsMessage(string Address, string Body);

/// <summary>
/// The whole Blue Connect client: identity, device registry, UDP discovery,
/// the mutual-TLS listener (incoming pairing + packets from paired devices)
/// and the client side (pair with a device, ping, SMS relay through a phone).
/// UI-free; every event can fire on a background thread.
/// </summary>
public sealed class ConnectService : IDisposable
{
    private static readonly TimeSpan HandshakeTimeout = TimeSpan.FromSeconds(20);
    private static readonly TimeSpan FirstPacketTimeout = TimeSpan.FromSeconds(30);

    private readonly ConnectOptions _opt;
    private readonly DeviceStore _store;
    private readonly TlsIdentity _tls;
    private readonly Discovery _discovery;
    private readonly CancellationTokenSource _cts = new();
    private readonly SemaphoreSlim _confirmGate = new(1, 1);
    private TcpListener? _listener;
    private bool _started;

    public string DeviceId { get; }
    public string DeviceName => _opt.DeviceName;
    public string DeviceType => Protocol.NormalizeDeviceType(_opt.DeviceType);
    public string Fingerprint => _tls.Fingerprint;
    public int TcpPort => _opt.TcpPort;

    /// <summary>Raised whenever the device list or a pairing state changed.</summary>
    public event Action? DevicesChanged;
    /// <summary>We initiated pairing: show this code and wait for the other side.</summary>
    public event Action<OutgoingPairingSas>? OutgoingPairingSas;
    /// <summary>A paired device pinged us.</summary>
    public event Action<DeviceInfo, string?>? PingReceived;
    public event Action<string>? Log;

    /// <summary>
    /// Decides an incoming pairing request (show the SAS, ask the person).
    /// The token is cancelled when the request times out, so a dialog can close itself.
    /// Without a handler every request is declined — never auto-accept.
    /// </summary>
    public Func<IncomingPairingRequest, CancellationToken, Task<bool>>? ConfirmPairing { get; set; }

    /// <summary>Optional plugin hook for other packets from a PAIRED device (e.g. SMS on a phone).
    /// Parameters: device, packet, a function that writes a framed reply.</summary>
    public Func<DeviceInfo, Protocol.Packet, Func<byte[], Task>, Task>? PacketHandler { get; set; }

    public ConnectService(ConnectOptions options)
    {
        _opt = options;
        Directory.CreateDirectory(options.DataDir);
        DeviceId = LoadOrCreateDeviceId(options.DataDir);
        _store = new DeviceStore(options.DataDir);
        _tls = TlsIdentity.LoadOrCreate(options.DataDir);
        _discovery = new Discovery(options, () => DeviceId, OwnIdentity, OnIdentitySeen, s => Log?.Invoke(s));
    }

    private static string LoadOrCreateDeviceId(string dir)
    {
        var path = Path.Combine(dir, "device_id");
        try
        {
            if (File.Exists(path))
            {
                var existing = File.ReadAllText(path).Trim();
                if (existing.Length > 0) return existing;
            }
        }
        catch { /* regenerate */ }
        var id = "blue-connect-" + Convert.ToHexString(RandomNumberGenerator.GetBytes(10)).ToLowerInvariant();
        File.WriteAllText(path, id);
        return id;
    }

    private byte[] OwnIdentity() => Protocol.BuildIdentity(DeviceId, DeviceName, DeviceType, _opt.TcpPort);

    private void OnIdentitySeen(Protocol.Identity id, IPAddress from)
    {
        _store.Seen(id.DeviceId, id.DeviceName, id.DeviceType, from.ToString(), id.TcpPort);
        DevicesChanged?.Invoke();
    }

    public IReadOnlyList<DeviceInfo> Devices => _store.All();

    // ── Lifecycle ────────────────────────────────────────────────────────

    /// <summary>Starts the TLS listener and the discovery listener. Throws if the TCP port is taken.</summary>
    public void Start()
    {
        if (_started) return;
        _listener = new TcpListener(_opt.BindAddress, _opt.TcpPort);
        _listener.Start();
        _started = true;
        _ = Task.Run(() => AcceptLoopAsync(_cts.Token));
        _ = Task.Run(() => _discovery.RunListenerAsync(_cts.Token));
        Log?.Invoke($"Blue Connect listening: TCP {_opt.TcpPort}, UDP {_opt.DiscoveryPort} as \"{DeviceName}\" ({DeviceId})");
    }

    public void Dispose()
    {
        _cts.Cancel();
        try { _listener?.Stop(); } catch { /* shutting down */ }
        _discovery.Dispose();
        _tls.Dispose();
    }

    // ── Discovery ────────────────────────────────────────────────────────

    /// <summary>Broadcasts and listens for replies for <paramref name="duration"/>, then returns the known devices.</summary>
    public async Task<IReadOnlyList<DeviceInfo>> ScanAsync(TimeSpan duration, CancellationToken ct = default)
    {
        await _discovery.ScanAsync(duration, ct).ConfigureAwait(false);
        return _store.All();
    }

    public void Forget(string deviceId)
    {
        if (_store.Forget(deviceId)) DevicesChanged?.Invoke();
    }

    // ── Incoming connections ─────────────────────────────────────────────

    private async Task AcceptLoopAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            TcpClient client;
            try { client = await _listener!.AcceptTcpClientAsync(ct).ConfigureAwait(false); }
            catch (OperationCanceledException) { return; }
            catch (ObjectDisposedException) { return; }
            catch (SocketException) { if (ct.IsCancellationRequested) return; continue; }
            _ = Task.Run(() => HandleConnectionAsync(client, ct), ct);
        }
    }

    private async Task HandleConnectionAsync(TcpClient client, CancellationToken ct)
    {
        using var _c = client;
        var remote = (IPEndPoint?)client.Client.RemoteEndPoint;
        var ip = remote?.Address.ToString() ?? "unknown";
        try
        {
            string? peerFp = null;
            await using var ssl = new SslStream(client.GetStream(), false);
            using (var hs = CancellationTokenSource.CreateLinkedTokenSource(ct))
            {
                hs.CancelAfter(HandshakeTimeout);
                await ssl.AuthenticateAsServerAsync(_tls.ServerOptions(fp => peerFp = fp), hs.Token).ConfigureAwait(false);
            }
            if (peerFp is null) return;

            var reader = new PacketReader(ssl);
            string? line;
            using (var rd = CancellationTokenSource.CreateLinkedTokenSource(ct))
            {
                rd.CancelAfter(FirstPacketTimeout);
                line = await reader.ReadAsync(rd.Token).ConfigureAwait(false);
            }
            var first = line is null ? null : Protocol.Parse(line);
            if (first is null) return;

            if (first.Type == Protocol.TypePair)
            {
                await HandlePairPacketAsync(ssl, first, peerFp, ip, ct).ConfigureAwait(false);
                return;
            }

            // Any other packet: only from a device we paired (certificate pinned).
            var dev = _store.FindPairedByFingerprint(peerFp);
            if (dev is null)
            {
                Log?.Invoke($"Ignored a '{first.Type}' packet from {ip}: not a paired device");
                return;
            }
            _store.UpdateAddress(dev.Id, ip);

            Protocol.Packet? pkt = first;
            while (pkt is not null)
            {
                await DispatchAsync(dev, pkt, ssl, ct).ConfigureAwait(false);
                using var rd = CancellationTokenSource.CreateLinkedTokenSource(ct);
                rd.CancelAfter(TimeSpan.FromSeconds(10));
                string? next;
                try { next = await reader.ReadAsync(rd.Token).ConfigureAwait(false); }
                catch (OperationCanceledException) { break; }
                pkt = next is null ? null : Protocol.Parse(next);
            }
        }
        catch (OperationCanceledException) { /* timeout or shutdown */ }
        catch (Exception e)
        {
            Log?.Invoke($"Connection from {ip} failed: {e.Message}");
        }
    }

    private async Task DispatchAsync(DeviceInfo dev, Protocol.Packet pkt, SslStream ssl, CancellationToken ct)
    {
        switch (pkt.Type)
        {
            case Protocol.TypePing:
                PingReceived?.Invoke(dev, pkt.Body["message"]?.GetValue<string>());
                break;
            default:
                if (PacketHandler is { } h)
                {
                    await h(dev, pkt, async bytes =>
                    {
                        await ssl.WriteAsync(bytes, ct).ConfigureAwait(false);
                        await ssl.FlushAsync(ct).ConfigureAwait(false);
                    }).ConfigureAwait(false);
                }
                break;
        }
    }

    private async Task HandlePairPacketAsync(SslStream ssl, Protocol.Packet pkt, string peerFp, string ip, CancellationToken ct)
    {
        bool wantsPair = pkt.Body["pair"] is JsonValue v && v.TryGetValue<bool>(out var b) && b;

        if (!wantsPair)
        {
            // The other side unpaired us: drop the pin so it can no longer talk to us.
            var dev = _store.FindPairedByFingerprint(peerFp);
            if (dev is not null)
            {
                _store.Forget(dev.Id);
                Log?.Invoke($"{dev.Name} unpaired this device");
                DevicesChanged?.Invoke();
            }
            return;
        }

        var deviceId = pkt.Body["deviceId"]?.GetValue<string>();
        if (string.IsNullOrEmpty(deviceId))
        {
            Log?.Invoke($"Pairing packet from {ip} is missing deviceId");
            return;
        }

        var existing = _store.Get(deviceId);
        if (existing is { Paired: true, PinnedCertSha256: { } pinned } && pinned != peerFp)
        {
            // Same stance as the desktop: never silently re-pin a different key.
            Log?.Invoke($"Refused pairing from {existing.Name}: certificate differs from the one pinned earlier. " +
                        "Forget the device first if this is intentional (e.g. it was reset).");
            await WriteAsync(ssl, Protocol.PairResponse(false), ct).ConfigureAwait(false);
            return;
        }

        var name = existing?.Name ?? $"Device @ {ip}";
        var sas = Sas.Compute(_tls.Fingerprint, peerFp);
        var accepted = await AskAsync(new IncomingPairingRequest(deviceId, name, ip, sas), ct).ConfigureAwait(false);

        await WriteAsync(ssl, Protocol.PairResponse(accepted), ct).ConfigureAwait(false);
        if (!accepted)
        {
            Log?.Invoke($"Pairing request from {name} was declined or timed out");
            return;
        }

        _store.MarkPaired(deviceId, name, ip, existing?.TcpPort ?? Protocol.PairingTcpPort, peerFp);
        _store.UpdateAddress(deviceId, ip);
        Log?.Invoke($"Paired with {name}");
        DevicesChanged?.Invoke();
    }

    private async Task<bool> AskAsync(IncomingPairingRequest req, CancellationToken ct)
    {
        var handler = ConfirmPairing;
        if (handler is null) return false;

        await _confirmGate.WaitAsync(ct).ConfigureAwait(false); // one dialog at a time
        try
        {
            using var expiry = CancellationTokenSource.CreateLinkedTokenSource(ct);
            expiry.CancelAfter(Protocol.PairingDecisionTimeout);
            var decision = handler(req, expiry.Token);
            var timeout = Task.Delay(Protocol.PairingDecisionTimeout, ct);
            var finished = await Task.WhenAny(decision, timeout).ConfigureAwait(false);
            if (finished != decision) { expiry.Cancel(); return false; }
            return await decision.ConfigureAwait(false);
        }
        catch (OperationCanceledException) { return false; }
        finally { _confirmGate.Release(); }
    }

    private static async Task WriteAsync(Stream s, byte[] framed, CancellationToken ct)
    {
        await s.WriteAsync(framed, ct).ConfigureAwait(false); // one Write call = one TLS record for the desktop's single read()
        await s.FlushAsync(ct).ConfigureAwait(false);
    }

    // ── Outgoing connections ─────────────────────────────────────────────

    private async Task<(TcpClient tcp, SslStream ssl, string peerFp)> ConnectTlsAsync(DeviceInfo dev, CancellationToken ct)
    {
        if (!IPAddress.TryParse(dev.Address, out var addr))
            throw new ConnectException($"Invalid address for {dev.Name}: \"{dev.Address}\"");

        var tcp = new TcpClient();
        try
        {
            using (var c = CancellationTokenSource.CreateLinkedTokenSource(ct))
            {
                c.CancelAfter(TimeSpan.FromSeconds(10));
                await tcp.ConnectAsync(addr, dev.TcpPort, c.Token).ConfigureAwait(false);
            }
            string? peerFp = null;
            var ssl = new SslStream(tcp.GetStream(), false);
            try
            {
                using var hs = CancellationTokenSource.CreateLinkedTokenSource(ct);
                hs.CancelAfter(HandshakeTimeout);
                await ssl.AuthenticateAsClientAsync(_tls.ClientOptions(dev.Address, fp => peerFp = fp), hs.Token).ConfigureAwait(false);
            }
            catch { ssl.Dispose(); throw; }
            if (peerFp is null) { ssl.Dispose(); throw new ConnectException("TLS handshake completed without a peer certificate"); }
            return (tcp, ssl, peerFp);
        }
        catch (OperationCanceledException) when (!ct.IsCancellationRequested)
        {
            tcp.Dispose();
            throw new ConnectException($"Timed out connecting to {dev.Name} ({dev.Address}:{dev.TcpPort})");
        }
        catch (ConnectException) { tcp.Dispose(); throw; }
        catch (Exception e)
        {
            tcp.Dispose();
            throw new ConnectException($"Could not connect to {dev.Name} ({dev.Address}:{dev.TcpPort}): {e.Message}", e);
        }
    }

    /// <summary>
    /// Pairs with a discovered device (works against another Blue Connect client
    /// and against the desktop while its "Listen" button is active). Shows the
    /// SAS via <see cref="OutgoingPairingSas"/>, then waits up to two minutes
    /// for the other person to accept.
    /// </summary>
    public async Task PairAsync(string deviceId, CancellationToken ct = default)
    {
        var dev = _store.Get(deviceId) ?? throw new ConnectException("Unknown device — scan first");
        var (tcp, ssl, peerFp) = await ConnectTlsAsync(dev, ct).ConfigureAwait(false);
        using var _t = tcp;
        await using var _s = ssl;

        if (dev is { Paired: true, PinnedCertSha256: { } pinned } && pinned != peerFp)
            throw new ConnectException(
                $"Refusing to pair: {dev.Name} presented a different certificate than the one pinned when it was last paired. " +
                "This can mean the device was reset, or that something is impersonating it — Forget the device and re-pair only if you are sure.");

        var sas = Sas.Compute(_tls.Fingerprint, peerFp);
        OutgoingPairingSas?.Invoke(new OutgoingPairingSas(dev.Id, dev.Name, sas));

        await WriteAsync(ssl, Protocol.PairRequest(DeviceId), ct).ConfigureAwait(false);

        string? line;
        using (var rd = CancellationTokenSource.CreateLinkedTokenSource(ct))
        {
            rd.CancelAfter(Protocol.PairingDecisionTimeout);
            try { line = await new PacketReader(ssl).ReadAsync(rd.Token).ConfigureAwait(false); }
            catch (OperationCanceledException) when (!ct.IsCancellationRequested)
            {
                throw new ConnectException($"{dev.Name} did not respond to the pairing request in time");
            }
        }
        if (line is null) throw new ConnectException($"{dev.Name} closed the connection without responding");

        var reply = Protocol.Parse(line);
        bool accepted = reply?.Body["pair"] is JsonValue v && v.TryGetValue<bool>(out var b) && b;
        if (!accepted) throw new ConnectException($"Pairing was declined on {dev.Name}");

        _store.MarkPaired(dev.Id, dev.Name, dev.Address, dev.TcpPort, peerFp);
        Log?.Invoke($"Paired with {dev.Name}");
        DevicesChanged?.Invoke();
    }

    /// <summary>Opens a connection to a PAIRED device and verifies the pinned certificate.</summary>
    private async Task<(TcpClient tcp, SslStream ssl, DeviceInfo dev)> OpenAuthenticatedAsync(string deviceId, CancellationToken ct)
    {
        var dev = _store.Get(deviceId);
        if (dev is not { Paired: true, PinnedCertSha256: { } pinned })
            throw new ConnectException("That device is not paired — pair it first");

        var (tcp, ssl, peerFp) = await ConnectTlsAsync(dev, ct).ConfigureAwait(false);
        if (peerFp != pinned)
        {
            ssl.Dispose(); tcp.Dispose();
            throw new ConnectException(
                $"Refusing to talk to {dev.Name}: it presented a different certificate than the one pinned when it was paired. " +
                "Forget the device and re-pair only if you are sure it is really that device.");
        }
        return (tcp, ssl, dev);
    }

    public async Task PingAsync(string deviceId, string? message = null, CancellationToken ct = default)
    {
        var (tcp, ssl, _) = await OpenAuthenticatedAsync(deviceId, ct).ConfigureAwait(false);
        using var _t = tcp;
        await using var _s = ssl;
        var body = new JsonObject();
        if (!string.IsNullOrEmpty(message)) body["message"] = message;
        await WriteAsync(ssl, Protocol.Frame(Protocol.TypePing, body), ct).ConfigureAwait(false);
    }

    /// <summary>Asks a paired phone to send an SMS (phone must run Blue Connect for Android).</summary>
    public async Task SendSmsAsync(string deviceId, string phoneNumber, string messageBody, CancellationToken ct = default)
    {
        var (tcp, ssl, _) = await OpenAuthenticatedAsync(deviceId, ct).ConfigureAwait(false);
        using var _t = tcp;
        await using var _s = ssl;
        var body = new JsonObject { ["sendSms"] = true, ["phoneNumber"] = phoneNumber, ["messageBody"] = messageBody };
        await WriteAsync(ssl, Protocol.Frame(Protocol.TypeSmsRequest, body), ct).ConfigureAwait(false);
    }

    /// <summary>Requests the phone's SMS history (address + body pairs).</summary>
    public async Task<IReadOnlyList<SmsMessage>> RequestSmsAsync(string deviceId, TimeSpan timeout, CancellationToken ct = default)
    {
        var (tcp, ssl, dev) = await OpenAuthenticatedAsync(deviceId, ct).ConfigureAwait(false);
        using var _t = tcp;
        await using var _s = ssl;
        var body = new JsonObject { ["requestAllConversations"] = true };
        await WriteAsync(ssl, Protocol.Frame(Protocol.TypeSmsRequest, body), ct).ConfigureAwait(false);

        using var rd = CancellationTokenSource.CreateLinkedTokenSource(ct);
        rd.CancelAfter(timeout);
        string? line;
        try { line = await new PacketReader(ssl).ReadAsync(rd.Token).ConfigureAwait(false); }
        catch (OperationCanceledException) when (!ct.IsCancellationRequested)
        {
            throw new ConnectException($"{dev.Name} did not reply with SMS history in time");
        }
        if (line is null) throw new ConnectException($"{dev.Name} closed the connection without replying");

        var reply = Protocol.Parse(line);
        var result = new List<SmsMessage>();
        if (reply?.Body["messages"] is JsonArray arr)
        {
            foreach (var m in arr)
            {
                var text = m?["body"]?.GetValue<string>();
                if (text is null) continue;
                result.Add(new SmsMessage(m?["address"]?.GetValue<string>() ?? "", text));
            }
        }
        return result;
    }
}
