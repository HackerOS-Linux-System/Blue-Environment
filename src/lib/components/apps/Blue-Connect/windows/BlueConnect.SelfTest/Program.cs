using System.Net;
using System.Text;
using System.Text.Json.Nodes;
using BlueConnect.Core;

// ─────────────────────────────────────────────────────────────────────────
//  dotnet run --project BlueConnect.SelfTest            → run all self-tests
//  dotnet run --project BlueConnect.SelfTest -- serve   → interactive node
//        [--tcp N] [--udp N] [--dir PATH] [--name NAME]   (used to check
//        interoperability against the Rust desktop implementation)
// ─────────────────────────────────────────────────────────────────────────

if (args.Length > 0 && args[0] == "serve") return await Serve(args);
return await SelfTests.RunAsync();

static async Task<int> Serve(string[] a)
{
    string Opt(string name, string def) { var i = Array.IndexOf(a, name); return i >= 0 && i + 1 < a.Length ? a[i + 1] : def; }
    var dir = Opt("--dir", Path.Combine(Path.GetTempPath(), "bc-serve"));
    using var svc = new ConnectService(new ConnectOptions
    {
        DataDir = dir,
        DeviceName = Opt("--name", "SelfTest Node"),
        DeviceType = "laptop",
        TcpPort = int.Parse(Opt("--tcp", "1717")),
        DiscoveryPort = int.Parse(Opt("--udp", "1716")),
    });
    svc.Log += s => Console.WriteLine("LOG " + s);
    svc.DevicesChanged += () => Console.WriteLine("DEVICES " + string.Join(", ", svc.Devices.Select(d => $"{d.Id}@{d.Address}:{d.TcpPort} paired={d.Paired}")));
    svc.PingReceived += (d, m) => Console.WriteLine($"PING from {d.Name} msg={m}");
    svc.ConfirmPairing = (req, _) =>
    {
        Console.WriteLine($"INCOMING-PAIR id={req.DeviceId} name={req.DeviceName} sas={req.Sas}");
        return Task.FromResult(true);
    };
    svc.OutgoingPairingSas += s => Console.WriteLine($"OUTGOING-SAS id={s.DeviceId} sas={s.Sas}");
    // Fake "phone": answers SMS requests like the Android client does.
    svc.PacketHandler = async (dev, pkt, reply) =>
    {
        if (pkt.Type != Protocol.TypeSmsRequest) return;
        if (pkt.Body["sendSms"]?.GetValue<bool>() == true)
        {
            Console.WriteLine($"SMS-SEND to={pkt.Body["phoneNumber"]} body={pkt.Body["messageBody"]}");
            return;
        }
        var msgs = new JsonArray();
        msgs.Add(new JsonObject { ["address"] = "+48600100200", ["body"] = "Cześć! Zażółć gęślą jaźń 🙂" });
        msgs.Add(new JsonObject { ["address"] = "ORANGE", ["body"] = "Twój kod: 123456" });
        await reply(Protocol.Frame(Protocol.TypeSmsMessages, new JsonObject { ["messages"] = msgs }));
    };
    svc.Start();
    Console.WriteLine($"READY id={svc.DeviceId} fp={svc.Fingerprint}");
    Console.Out.Flush();

    string? line;
    while ((line = Console.ReadLine()) is not null)
    {
        var parts = line.Split(' ', 2, StringSplitOptions.RemoveEmptyEntries);
        try
        {
            switch (parts.FirstOrDefault())
            {
                case "scan": await svc.ScanAsync(TimeSpan.FromSeconds(2)); Console.WriteLine("SCAN done"); break;
                case "pair": await svc.PairAsync(parts[1]); Console.WriteLine("PAIR ok"); break;
                case "ping": await svc.PingAsync(parts[1], "hello"); Console.WriteLine("PING sent"); break;
                case "list": Console.WriteLine("LIST " + string.Join(", ", svc.Devices.Select(d => $"{d.Id} paired={d.Paired} pin={d.PinnedCertSha256}"))); break;
                case "quit": return 0;
            }
        }
        catch (Exception e) { Console.WriteLine("ERROR " + e.Message); }
        Console.Out.Flush();
    }
    await Task.Delay(Timeout.Infinite);
    return 0;
}

static class SelfTests
{
    static int _failed, _passed;

    static void Check(bool ok, string name)
    {
        if (ok) { _passed++; Console.WriteLine($"  ok   {name}"); }
        else { _failed++; Console.WriteLine($"  FAIL {name}"); }
    }

    static void Eq<T>(T actual, T expected, string name)
    {
        var ok = EqualityComparer<T>.Default.Equals(actual, expected);
        if (!ok) Console.WriteLine($"       expected {expected}, got {actual}");
        Check(ok, name);
    }

    public static async Task<int> RunAsync()
    {
        Console.WriteLine("Sas / fingerprint");
        // Vectors produced by the desktop's own tls.rs::compute_sas.
        Eq(Sas.Compute("aaaa1111", "bbbb2222"), "790764", "SAS vector 1 (from Rust)");
        Eq(Sas.Compute("bbbb2222", "aaaa1111"), "790764", "SAS is order independent");
        Eq(Sas.Compute("x", "y"), "781997", "SAS vector 2 (from Rust)");
        Eq(Sas.Compute("0123456789abcdef", "fedcba9876543210"), "433941", "SAS vector 3 (from Rust)");
        var fp = Sas.Fingerprint(new byte[] { 1, 2, 3, 4 });
        Eq(fp.Length, 64, "fingerprint is 64 hex chars");
        Check(fp == fp.ToLowerInvariant(), "fingerprint is lowercase");
        Eq(fp, "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a", "fingerprint of [1,2,3,4] (known SHA-256)");

        Console.WriteLine("Packets");
        var id = Protocol.BuildIdentity("blue-connect-abc", "Test PC", "desktop", 1717);
        var parsed = Protocol.ParseIdentity(id);
        Check(parsed is not null && parsed.DeviceId == "blue-connect-abc" && parsed.TcpPort == 1717 && parsed.ProtocolVersion == 7, "identity round trip");
        Check(!Encoding.UTF8.GetString(id).EndsWith('\n'), "identity datagram has no trailing newline");
        Check(Protocol.ParseIdentity(Encoding.UTF8.GetBytes("{\"type\":\"kdeconnect.identity\",\"body\":{\"deviceId\":\"x\"}}")) is null, "identity with missing fields is ignored");
        Check(Protocol.ParseIdentity(Encoding.UTF8.GetBytes("{\"type\":\"kdeconnect.pair\",\"body\":{}}")) is null, "non-identity datagram is ignored");
        Check(Protocol.ParseIdentity(Encoding.UTF8.GetBytes("not json")) is null, "garbage datagram is ignored");
        var framed = Protocol.PairRequest("dev1");
        Check(framed[^1] == (byte)'\n', "framed packet ends with newline");
        Check(Protocol.Parse(Encoding.UTF8.GetString(framed))!.Body["pair"]!.GetValue<bool>(), "pair request carries pair=true");

        Console.WriteLine("PacketReader");
        {
            var ms = new MemoryStream(Encoding.UTF8.GetBytes("{\"type\":\"a\",\"body\":{}}\n\n{\"type\":\"b\",\"body\":{}}\n{\"type\":\"c\",\"body\":{}}"));
            var r = new PacketReader(ms);
            var p1 = await r.ReadAsync(default); var p2 = await r.ReadAsync(default); var p3 = await r.ReadAsync(default); var p4 = await r.ReadAsync(default);
            Check(p1!.Contains("\"a\"") && p2!.Contains("\"b\"") && p3!.Contains("\"c\"") && p4 is null, "back-to-back packets, blank line skipped, last one without newline");
        }

        Console.WriteLine("DeviceStore (desktop devices.json compatibility)");
        {
            var dir = Path.Combine(Path.GetTempPath(), "bc-test-store-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            File.WriteAllText(Path.Combine(dir, "devices.json"),
                "{\"d1\":{\"id\":\"d1\",\"name\":\"Phone\",\"deviceType\":\"phone\",\"address\":\"192.168.1.5\",\"tcpPort\":1717,\"paired\":true,\"pinnedCertSha256\":\"abc\"}," +
                "\"d2\":{\"id\":\"d2\",\"name\":\"Old\",\"deviceType\":\"desktop\",\"address\":\"10.0.0.2\",\"tcpPort\":1717,\"paired\":false,\"pinnedCertSha256\":null}}");
            var st = new DeviceStore(dir);
            var all = st.All();
            Check(all.Count == 2 && all[0].Id == "d1" && all[0].Paired, "reads desktop file, paired devices first");
            Eq(st.FindPairedByFingerprint("abc")?.Id, "d1", "lookup by pinned fingerprint");
            Check(st.FindPairedByFingerprint("zzz") is null, "unknown fingerprint is not a paired device");
            st.Seen("d2", "Renamed", "laptop", "10.0.0.9", 1800);
            Check(st.Get("d2") is { Name: "Renamed", Address: "10.0.0.9", TcpPort: 1800, Paired: false }, "Seen() updates discovery fields only");
            Check(new DeviceStore(dir).Get("d2")?.Name == "Renamed", "changes are persisted");
            Check(File.ReadAllText(Path.Combine(dir, "devices.json")).Contains("\"pinnedCertSha256\""), "written with the desktop's field names");
        }

        Console.WriteLine("TLS identity");
        {
            var dir = Path.Combine(Path.GetTempPath(), "bc-test-id-" + Guid.NewGuid().ToString("N"));
            string f1, f2;
            using (var a = TlsIdentity.LoadOrCreate(dir)) f1 = a.Fingerprint;
            using (var b = TlsIdentity.LoadOrCreate(dir)) f2 = b.Fingerprint;
            Eq(f2, f1, "identity is persistent across restarts");
            Eq(f1.Length, 64, "identity fingerprint is a sha-256 hex");
        }

        PluginsUnit();
        await EndToEndAsync();

        Console.WriteLine();
        Console.WriteLine(_failed == 0 ? $"ALL {_passed} CHECKS PASSED" : $"{_failed} FAILED, {_passed} passed");
        return _failed == 0 ? 0 : 1;
    }

    static async Task EndToEndAsync()
    {
        Console.WriteLine("End to end (two real nodes over loopback, real mutual TLS)");
        string Tmp() => Path.Combine(Path.GetTempPath(), "bc-e2e-" + Guid.NewGuid().ToString("N"));
        var loop = new[] { IPAddress.Loopback };

        using var a = new ConnectService(new ConnectOptions { DataDir = Tmp(), DeviceName = "Node A", DeviceType = "phone",
            TcpPort = 41717, DiscoveryPort = 41716, BindAddress = IPAddress.Loopback });
        using var b = new ConnectService(new ConnectOptions { DataDir = Tmp(), DeviceName = "Node B", DeviceType = "laptop",
            TcpPort = 42717, DiscoveryPort = 42716, ScanTargetPort = 41716, DiscoveryTargets = loop, BindAddress = IPAddress.Loopback });

        IncomingPairingRequest? incoming = null;
        OutgoingPairingSas? outgoing = null;
        bool accept = true;
        a.ConfirmPairing = (req, _) => { incoming = req; return Task.FromResult(accept); };
        b.OutgoingPairingSas += s => outgoing = s;
        var pinged = new TaskCompletionSource<string?>();
        a.PingReceived += (_, m) => pinged.TrySetResult(m);
        a.Start(); b.Start();
        await Task.Delay(300);

        var found = await b.ScanAsync(TimeSpan.FromSeconds(1.5));
        var seenA = found.FirstOrDefault(d => d.Id == a.DeviceId);
        Check(seenA is not null && seenA.Name == "Node A" && seenA.DeviceType == "phone" && seenA.TcpPort == 41717, "B discovers A over UDP (A replied to B's source port)");

        accept = false;
        string? declined = null;
        try { await b.PairAsync(a.DeviceId); } catch (ConnectException e) { declined = e.Message; }
        Check(declined is not null && declined.Contains("declined"), "declined pairing is reported and nothing is pinned");
        Check(b.Devices.First(d => d.Id == a.DeviceId) is { Paired: false, PinnedCertSha256: null }, "after decline B has no pin");

        accept = true;
        await b.PairAsync(a.DeviceId);
        Check(incoming is not null && outgoing is not null && incoming.Sas == outgoing.Sas, "both people see the SAME 6-digit code");
        Check(incoming!.Sas == Sas.Compute(a.Fingerprint, b.Fingerprint), "SAS matches the fingerprints of the two identities");
        var aOnB = b.Devices.First(d => d.Id == a.DeviceId);
        Check(aOnB.Paired && aOnB.PinnedCertSha256 == a.Fingerprint, "B pinned A's real certificate fingerprint");
        var bOnA = a.Devices.FirstOrDefault(d => d.Id == b.DeviceId);
        Check(bOnA is { Paired: true } && bOnA.PinnedCertSha256 == b.Fingerprint, "A pinned B's real certificate fingerprint");

        await b.PingAsync(a.DeviceId, "hi from B");
        var got = await Task.WhenAny(pinged.Task, Task.Delay(3000)) == pinged.Task ? pinged.Task.Result : null;
        Eq(got, "hi from B", "ping travels over the authenticated channel");

        // ── plugin round trips over the real authenticated channel ──
        var host = new TestHost();
        a.PacketHandler = new PluginRouter(host).AsPacketHandler();
        var bat = await b.GetBatteryAsync(a.DeviceId);
        Check(bat is { Percent: 73, Charging: true }, "battery request → reply over mutual TLS");
        await b.FindDeviceAsync(a.DeviceId);
        await b.SendClipboardAsync(a.DeviceId, "zażółć gęślą jaźń 🙂");
        await b.ShareAsync(a.DeviceId, "https://example.org/a?b=c");
        await b.ShareAsync(a.DeviceId, "just some text");
        await b.ShareAsync(a.DeviceId, "javascript:alert(1)"); // not a safe URL → delivered as plain text, never opened
        for (int i = 0; i < 40 && host.Texts.Count < 2; i++) await Task.Delay(50);
        Check(host.Rings == 1, "find-my-device rings the other side once");
        Eq(host.Clipboard, "zażółć gęślą jaźń 🙂", "clipboard text (with Unicode) arrives intact");
        Eq(string.Join("|", host.Opened), "https://example.org/a?b=c", "safe link is opened");
        Eq(string.Join("|", host.Texts), "just some text|javascript:alert(1)", "other text is only received, never opened");
        host.Clipboard = "phone clipboard";
        Eq(await b.GetClipboardAsync(a.DeviceId), "phone clipboard", "clipboard request → reply");
        var notes = await b.GetNotificationsAsync(a.DeviceId);
        Check(notes.Count == 2 && notes[0].App == "Mail" && notes[1].Title == "Ping", "notification list request → reply");
        host.Battery = null;
        Check(await b.GetBatteryAsync(a.DeviceId) is null, "a device without a battery answers 'none'");

        // Another node that was never paired must be ignored on the plugin channel.
        using var c = new ConnectService(new ConnectOptions { DataDir = Tmp(), DeviceName = "Stranger", TcpPort = 43717, DiscoveryPort = 43716,
            ScanTargetPort = 41716, DiscoveryTargets = loop, BindAddress = IPAddress.Loopback });
        c.Start();
        await c.ScanAsync(TimeSpan.FromSeconds(1.2));
        string? err = null;
        try { await c.PingAsync(a.DeviceId); } catch (ConnectException e) { err = e.Message; }
        Check(err is not null && err.Contains("not paired"), "C cannot ping a device it never paired with");

        // Certificate-pin violation: A "is reset" (new identity, same id) → B must refuse to talk.
        var aDir = Path.Combine(Path.GetTempPath(), "bc-e2e-impostor-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(aDir);
        File.Copy(Path.Combine(((string?)typeof(ConnectOptions).GetProperty("DataDir")!.GetValue(GetOpt(a)))!, "device_id"), Path.Combine(aDir, "device_id"));
        a.Dispose();
        await Task.Delay(300);
        using var impostor = new ConnectService(new ConnectOptions { DataDir = aDir, DeviceName = "Node A", DeviceType = "phone",
            TcpPort = 41717, DiscoveryPort = 41716, BindAddress = IPAddress.Loopback });
        impostor.Start();
        await Task.Delay(300);
        Check(impostor.DeviceId == aOnB.Id && impostor.Fingerprint != a.Fingerprint, "impostor has A's id but a different certificate");
        string? pinErr = null;
        try { await b.PingAsync(a.DeviceId, "x"); } catch (ConnectException e) { pinErr = e.Message; }
        Check(pinErr is not null && pinErr.Contains("different certificate"), "B refuses a different certificate than the pinned one");
        string? repairErr = null;
        try { await b.PairAsync(a.DeviceId); } catch (ConnectException e) { repairErr = e.Message; }
        Check(repairErr is not null && repairErr.Contains("different certificate"), "re-pairing with a swapped certificate is refused");

        b.Forget(a.DeviceId);
        Check(b.Devices.All(d => d.Id != a.DeviceId), "Forget removes the device");
    }

    sealed class TestHost : IPluginHost
    {
        public BatteryStatus? Battery = new(73, true);
        public int Rings;
        public string? Clipboard;
        public List<string> Opened = new(), Texts = new();
        public BatteryStatus? GetBattery() => Battery;
        public void Ring() => Rings++;
        public void SetClipboard(string text) => Clipboard = text;
        public string? GetClipboard() => Clipboard;
        public void OpenUrl(string url) => Opened.Add(url);
        public void ReceiveText(string text, DeviceInfo from) => Texts.Add(text);
        public IReadOnlyList<NotificationInfo> GetNotifications() => new[]
        {
            new NotificationInfo("1", "Mail", "New message", "Hello", 1000), new NotificationInfo("2", "Chat", "Ping", "x", 2000),
        };
    }

    static void PluginsUnit()
    {
        Console.WriteLine("Plugins (pure)");
        Check(PluginPackets.IsSafeUrl("https://example.org/x?y=1"), "https url is safe");
        Check(PluginPackets.IsSafeUrl("mailto:a@b.c") && PluginPackets.IsSafeUrl("tel:+48123456789"), "mailto / tel are safe");
        Check(!PluginPackets.IsSafeUrl("javascript:alert(1)") && !PluginPackets.IsSafeUrl("file:///etc/passwd")
              && !PluginPackets.IsSafeUrl("ms-settings:") && !PluginPackets.IsSafeUrl("smb://x/y"), "script / file / app-protocol urls are refused");
        Check(!PluginPackets.IsSafeUrl("https://a b") && !PluginPackets.IsSafeUrl("") && !PluginPackets.IsSafeUrl(null) && !PluginPackets.IsSafeUrl("https://x/\n"), "whitespace / control characters / empty are refused");
        Check(!PluginPackets.IsSafeUrl("https://x/" + new string('a', 3000)), "overlong urls are refused");

        var cut = PluginPackets.FitUtf8("ą" + new string('x', 10), 2); // 'ą' is 2 bytes
        Eq(cut, "ą", "FitUtf8 keeps whole characters");
        Eq(PluginPackets.FitUtf8("🙂🙂", 5), "🙂", "FitUtf8 never splits a surrogate pair / multi-byte char");
        Eq(PluginPackets.FitUtf8("short", 100), "short", "FitUtf8 leaves short text alone");

        var big = Enumerable.Range(0, 200).Select(i => new NotificationInfo(i.ToString(), "App", new string('t', 200), new string('b', 400), i)).ToList();
        var frame = PluginPackets.NotificationList(big);
        Check(frame.Length <= Protocol.MaxSingleReadReplyBytes, "notification list is trimmed to fit one TLS record");
        var parsed = PluginPackets.ParseNotifications(Protocol.Parse(Encoding.UTF8.GetString(frame))!);
        Check(parsed.Count > 0 && parsed.Count < 200 && parsed[0].Id == "0", "…keeping the newest entries first");

        var bp = Protocol.Parse(Encoding.UTF8.GetString(PluginPackets.Battery(new BatteryStatus(250, false))))!;
        Eq(PluginPackets.ParseBattery(bp)?.Percent, 100, "battery percent is clamped");
        Check(PluginPackets.ParseBattery(Protocol.Parse(Encoding.UTF8.GetString(PluginPackets.Battery(null)))!) is null, "no battery is reported as -1 → null");

        var caps = Encoding.UTF8.GetString(Protocol.BuildIdentity("id", "n", "phone", 1717));
        Check(caps.Contains(Protocol.TypeClipboard) && caps.Contains(Protocol.TypeFindMyPhone), "identity advertises the plugin capabilities");
        Check(Protocol.ParseIdentity(Encoding.UTF8.GetBytes(caps)) is not null, "identity with the extra capabilities still parses (desktop compatibility)");
    }

    // ConnectService keeps its options private; the test only needs the data dir.
    static ConnectOptions GetOpt(ConnectService s) =>
        (ConnectOptions)typeof(ConnectService).GetField("_opt", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance)!.GetValue(s)!;
}
