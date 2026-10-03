using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace BlueConnect.App;

/// <summary>Draws the Blue sparkle logo at runtime, so no binary icon asset has to live in the repo.</summary>
internal static class TrayIconFactory
{
    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(IntPtr handle);

    public static Icon Create()
    {
        const int size = 32;
        using var bmp = new Bitmap(size, size);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Color.Transparent);
            using var path = new GraphicsPath();
            int r = 7, d = r * 2;
            var rect = new Rectangle(1, 1, size - 2, size - 2);
            path.AddArc(rect.X, rect.Y, d, d, 180, 90);
            path.AddArc(rect.Right - d, rect.Y, d, d, 270, 90);
            path.AddArc(rect.Right - d, rect.Bottom - d, d, d, 0, 90);
            path.AddArc(rect.X, rect.Bottom - d, d, d, 90, 90);
            path.CloseFigure();
            using (var bg = new LinearGradientBrush(rect, Color.FromArgb(59, 130, 246), Color.FromArgb(67, 56, 202), 45f))
                g.FillPath(bg, path);

            // four-point sparkle
            var c = size / 2f;
            PointF[] star =
            {
                new(c, 6), new(c + 2.6f, c - 2.6f), new(size - 6, c), new(c + 2.6f, c + 2.6f),
                new(c, size - 6), new(c - 2.6f, c + 2.6f), new(6, c), new(c - 2.6f, c - 2.6f),
            };
            g.FillPolygon(Brushes.White, star);
        }

        var hIcon = bmp.GetHicon();
        try
        {
            using var temp = Icon.FromHandle(hIcon);
            return (Icon)temp.Clone(); // own copy, so the GDI handle can be released
        }
        finally
        {
            DestroyIcon(hIcon);
        }
    }
}
