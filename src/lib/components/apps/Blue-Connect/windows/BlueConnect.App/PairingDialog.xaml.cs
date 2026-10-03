using System.Windows;
using BlueConnect.Core;

namespace BlueConnect.App;

/// <summary>Asks the person whether to accept an incoming pairing request; shows the SAS to compare.</summary>
public partial class PairingDialog : Window
{
    public bool Accepted { get; private set; }

    public PairingDialog(IncomingPairingRequest request)
    {
        InitializeComponent();
        TitleText.Text = $"Pair with “{request.DeviceName}”?";
        SubText.Text = $"A device at {request.Address} wants to pair with this computer.";
        SasText.Text = request.Sas.Length == 6 ? $"{request.Sas[..3]} {request.Sas[3..]}" : request.Sas;
    }

    private void OnAccept(object sender, RoutedEventArgs e)
    {
        Accepted = true;
        Close();
    }

    private void OnReject(object sender, RoutedEventArgs e)
    {
        Accepted = false;
        Close();
    }
}
