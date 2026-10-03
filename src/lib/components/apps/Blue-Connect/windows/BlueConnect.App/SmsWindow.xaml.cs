using System;
using System.Windows;
using BlueConnect.Core;

namespace BlueConnect.App;

/// <summary>Uses a paired Android phone as an SMS gateway (the same bridge Blue Messages uses).</summary>
public partial class SmsWindow : Window
{
    private readonly ConnectService _service;
    private readonly DeviceInfo _phone;

    public SmsWindow(ConnectService service, DeviceInfo phone)
    {
        InitializeComponent();
        _service = service;
        _phone = phone;
        HeaderText.Text = $"SMS via {phone.Name}";
    }

    private async void OnSend(object sender, RoutedEventArgs e)
    {
        var number = NumberBox.Text.Trim();
        var body = BodyBox.Text;
        if (number.Length == 0 || body.Length == 0)
        {
            StatusText.Text = "Enter a phone number and a message.";
            return;
        }
        SendButton.IsEnabled = false;
        try
        {
            await _service.SendSmsAsync(_phone.Id, number, body);
            StatusText.Text = $"Asked {_phone.Name} to send the message to {number}.";
            BodyBox.Clear();
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
        finally
        {
            SendButton.IsEnabled = true;
        }
    }

    private async void OnLoad(object sender, RoutedEventArgs e)
    {
        LoadButton.IsEnabled = false;
        StatusText.Text = "Asking the phone…";
        try
        {
            var msgs = await _service.RequestSmsAsync(_phone.Id, TimeSpan.FromSeconds(10));
            MessagesList.ItemsSource = msgs;
            StatusText.Text = msgs.Count == 0
                ? "The phone returned no messages (is SMS access allowed in Blue Connect on the phone?)."
                : $"{msgs.Count} message(s), newest first.";
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
        finally
        {
            LoadButton.IsEnabled = true;
        }
    }
}
