using NetBridgeLib.Enums;

namespace NetBridgeLib.Services;

public class NetBridgeService : IDisposable
{
    private readonly NetBridgeNative.LogCallback? _logCallback;
    private readonly NetBridgeNative.ConnectionCallback? _connectionCallback;
    private bool _isRunning;

    public event Action<string>? LogReceived;

    public event Action<string, uint, string, ushort, string>? ConnectionReceived;

    public NetBridgeService()
    {
        _logCallback = OnLogReceived;
        _connectionCallback = OnConnectionReceived;

        NetBridgeNative.ProxyBridge_SetLogCallback(_logCallback);
        NetBridgeNative.ProxyBridge_SetConnectionCallback(_connectionCallback);
    }

    private void OnLogReceived(string message)
    {
        LogReceived?.Invoke(message);
    }

    private void OnConnectionReceived(string processName, uint pid, string destIp, ushort destPort, string proxyInfo)
    {
        ConnectionReceived?.Invoke(processName, pid, destIp, destPort, proxyInfo);
    }

    public bool Start()
    {
        if (_isRunning)
        {
            return true;
        }

        _isRunning = NetBridgeNative.ProxyBridge_Start();
        return _isRunning;
    }

    public bool Stop()
    {
        if (!_isRunning)
        {
            return true;
        }

        _isRunning = !NetBridgeNative.ProxyBridge_Stop();
        return !_isRunning;
    }

    public uint AddProxyConfig(string type, string ip, ushort port, string username, string password)
    {
        var proxyType = type.Equals("HTTP", StringComparison.CurrentCultureIgnoreCase) ? NetProxyType.HTTP : NetProxyType.SOCKS5;

        return NetBridgeNative.ProxyBridge_AddProxyConfig(proxyType, ip, port, username, password);
    }

    public bool EditProxyConfig(uint configId, string type, string ip, ushort port, string username, string password)
    {
        var proxyType = type.Equals("HTTP", StringComparison.CurrentCultureIgnoreCase) ? NetProxyType.HTTP : NetProxyType.SOCKS5;

        return NetBridgeNative.ProxyBridge_EditProxyConfig(configId, proxyType, ip, port, username, password);
    }

    public bool DeleteProxyConfig(uint configId)
    {
        return NetBridgeNative.ProxyBridge_DeleteProxyConfig(configId);
    }

    public string TestProxyConfig(uint configId, string targetHost, ushort targetPort)
    {
        var buffer = new System.Text.StringBuilder(4096);
        NetBridgeNative.ProxyBridge_TestProxyConfig(configId, targetHost, targetPort, buffer, (UIntPtr)buffer.Capacity);
        return buffer.ToString();
    }

    public uint AddRule(string processName, string targetHosts, string targetPorts, string protocol, string action, uint proxyConfigId = 0)
    {
        var ruleAction = action.Equals("DIRECT", StringComparison.CurrentCultureIgnoreCase) ? NetRuleAction.DIRECT :
                         action.Equals("BLOCK", StringComparison.CurrentCultureIgnoreCase) ? NetRuleAction.BLOCK :
                         NetRuleAction.PROXY;

        var ruleProtocol = protocol.Equals("UDP", StringComparison.CurrentCultureIgnoreCase) ? NetRuleProtocol.UDP :
                           protocol.Equals("BOTH", StringComparison.CurrentCultureIgnoreCase) ? NetRuleProtocol.BOTH :
                           protocol.Equals("TCP+UDP", StringComparison.CurrentCultureIgnoreCase) ? NetRuleProtocol.BOTH :
                           NetRuleProtocol.TCP;

        return NetBridgeNative.ProxyBridge_AddRule(processName, targetHosts, targetPorts, ruleProtocol, ruleAction, proxyConfigId);
    }

    public bool EnableRule(uint ruleId)
    {
        return NetBridgeNative.ProxyBridge_EnableRule(ruleId);
    }

    public bool DisableRule(uint ruleId)
    {
        return NetBridgeNative.ProxyBridge_DisableRule(ruleId);
    }

    public bool DeleteRule(uint ruleId)
    {
        return NetBridgeNative.ProxyBridge_DeleteRule(ruleId);
    }

    public bool EditRule(uint ruleId, string processName, string targetHosts, string targetPorts, string protocol, string action, uint proxyConfigId = 0)
    {
        var ruleAction = action.Equals("DIRECT", StringComparison.CurrentCultureIgnoreCase) ? NetRuleAction.DIRECT :
                         action.Equals("BLOCK", StringComparison.CurrentCultureIgnoreCase) ? NetRuleAction.BLOCK :
                         NetRuleAction.PROXY;

        var ruleProtocol = protocol.Equals("UDP", StringComparison.CurrentCultureIgnoreCase) ? NetRuleProtocol.UDP :
                           protocol.Equals("BOTH", StringComparison.CurrentCultureIgnoreCase) ? NetRuleProtocol.BOTH :
                           protocol.Equals("TCP+UDP", StringComparison.CurrentCultureIgnoreCase) ? NetRuleProtocol.BOTH :
                           NetRuleProtocol.TCP;

        return NetBridgeNative.ProxyBridge_EditRule(ruleId, processName, targetHosts, targetPorts, ruleProtocol, ruleAction, proxyConfigId);
    }

    public uint GetRulePosition(uint ruleId)
    {
        return NetBridgeNative.ProxyBridge_GetRulePosition(ruleId);
    }

    public bool MoveRuleToPosition(uint ruleId, uint newPosition)
    {
        return NetBridgeNative.ProxyBridge_MoveRuleToPosition(ruleId, newPosition);
    }

    public void SetDnsViaProxy(bool enable)
    {
        NetBridgeNative.ProxyBridge_SetDnsViaProxy(enable);
    }

    public void SetLocalhostViaProxy(bool enable)
    {
        NetBridgeNative.ProxyBridge_SetLocalhostViaProxy(enable);
    }

    public static void SetTrafficLoggingEnabled(bool enable)
    {
        NetBridgeNative.ProxyBridge_SetTrafficLoggingEnabled(enable);
    }

    public void Dispose()
    {
        if (_isRunning)
        {
            Stop(); // removing the threads, C code handle close no need to manually handle drives
        }
        GC.SuppressFinalize(this);
    }
}
