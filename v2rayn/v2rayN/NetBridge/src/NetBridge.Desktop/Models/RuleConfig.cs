namespace NetBridge.Desktop.Models;

public sealed class RuleConfig
{
    public uint RuleId { get; set; }
    public string ProcessName { get; set; } = "Chrome.exe";
    public string TargetHosts { get; set; } = "*";
    public string TargetPorts { get; set; } = "*";
    public string Protocol { get; set; } = "TCP";
    public string Action { get; set; } = "PROXY";
    public uint ProxyConfigId { get; set; }
}
