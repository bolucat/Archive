using ReactiveUI.Fody.Helpers;

namespace NetBridge.Desktop.Models;

public sealed class ProxyConfig : ReactiveObject
{
    [Reactive] public uint ProxyConfigId { get; set; }
    [Reactive] public string ProxyType { get; set; } = "SOCKS5";
    [Reactive] public string ProxyHost { get; set; } = "127.0.0.1";
    [Reactive] public ushort ProxyPort { get; set; } = 10000;
    [Reactive] public string ProxyUsername { get; set; } = string.Empty;
    [Reactive] public string ProxyPassword { get; set; } = string.Empty;
}
