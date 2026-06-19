using System.Collections.ObjectModel;

namespace NetBridge.Desktop.Models;

public sealed class AppSettings
{
    public ProxyConfig ProxyConfig { get; set; } = new();
    public ObservableCollection<RuleConfig> Rules { get; set; } = [];
}
