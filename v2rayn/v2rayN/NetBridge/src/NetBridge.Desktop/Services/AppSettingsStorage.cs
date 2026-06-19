using System.Text.Json;
using NetBridge.Desktop.Models;

namespace NetBridge.Desktop.Services;

public sealed class AppSettingsStorage
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };

    private readonly string _configFilePath;

    public AppSettingsStorage()
    {
        var appDataDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "NetBridge.Desktop");
        Directory.CreateDirectory(appDataDir);
        _configFilePath = Path.Combine(appDataDir, "proxy-config.json");
    }

    public AppSettings LoadAll()
    {
        try
        {
            if (!File.Exists(_configFilePath))
            {
                return new AppSettings();
            }

            var json = File.ReadAllText(_configFilePath);
            return JsonSerializer.Deserialize<AppSettings>(json, JsonOptions) ?? new AppSettings();
        }
        catch
        {
            return new AppSettings();
        }
    }

    public void SaveAll(AppSettings settings)
    {
        var json = JsonSerializer.Serialize(settings, JsonOptions);
        File.WriteAllText(_configFilePath, json);
    }

    public ProxyConfig LoadProxyConfig()
    {
        return LoadAll().ProxyConfig;
    }

    public void SaveProxyConfig(ProxyConfig config)
    {
        var settings = LoadAll();
        settings.ProxyConfig = config;
        SaveAll(settings);
    }

    public List<RuleConfig> LoadRules()
    {
        return [.. LoadAll().Rules];
    }

    public void SaveRules(IEnumerable<RuleConfig> rules)
    {
        var settings = LoadAll();
        settings.Rules.Clear();
        foreach (var rule in rules)
        {
            settings.Rules.Add(rule);
        }

        SaveAll(settings);
    }
}
