using System.Text;

namespace BlueConnect.Core;

/// <summary>
/// Reads newline-terminated JSON packets from a stream. Keeps leftover bytes
/// between calls (a peer may send several packets back to back) and tolerates
/// a final packet that is not newline terminated before EOF.
/// </summary>
public sealed class PacketReader
{
    private readonly Stream _stream;
    private readonly List<byte> _pending = new();
    private readonly byte[] _chunk = new byte[4096];
    private bool _eof;

    public PacketReader(Stream stream) => _stream = stream;

    /// <returns>The next packet as raw JSON text, or null at end of stream.</returns>
    public async Task<string?> ReadAsync(CancellationToken ct)
    {
        while (true)
        {
            int nl = _pending.IndexOf((byte)'\n');
            if (nl >= 0)
            {
                var line = Encoding.UTF8.GetString(_pending.GetRange(0, nl).ToArray());
                _pending.RemoveRange(0, nl + 1);
                if (line.Trim().Length == 0) continue; // blank keep-alive line
                return line;
            }
            if (_eof)
            {
                if (_pending.Count == 0) return null;
                var rest = Encoding.UTF8.GetString(_pending.ToArray());
                _pending.Clear();
                return rest.Trim().Length == 0 ? null : rest;
            }
            if (_pending.Count > Protocol.MaxPacketBytes)
                throw new InvalidDataException("Packet too large");

            int n = await _stream.ReadAsync(_chunk.AsMemory(), ct).ConfigureAwait(false);
            if (n == 0) _eof = true;
            else _pending.AddRange(new ArraySegment<byte>(_chunk, 0, n));
        }
    }
}
