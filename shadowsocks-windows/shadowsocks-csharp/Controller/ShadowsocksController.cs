﻿using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Web;
using System.Windows.Forms;
using NLog;
using Shadowsocks.Controller.Service;
using Shadowsocks.Controller.Strategy;
using Shadowsocks.Model;
using Shadowsocks.Util;
using WPFLocalizeExtension.Engine;

namespace Shadowsocks.Controller
{
    public class ShadowsocksController
    {
        private readonly Logger logger;
        private readonly HttpClient httpClient;

        // controller:
        // handle user actions
        // manipulates UI
        // interacts with low level logic
        #region Members definition
        private Thread _trafficThread;

        private Listener _listener;
        private PACDaemon _pacDaemon;
        private PACServer _pacServer;
        private Configuration _config;
        private StrategyManager _strategyManager;
        private PrivoxyRunner privoxyRunner;
        private readonly ConcurrentDictionary<Server, Sip003Plugin> _pluginsByServer;

        private long _inboundCounter = 0;
        private long _outboundCounter = 0;
        public long InboundCounter => Interlocked.Read(ref _inboundCounter);
        public long OutboundCounter => Interlocked.Read(ref _outboundCounter);
        public Queue<TrafficPerSecond> trafficPerSecondQueue;

        private bool stopped = false;

        public class PathEventArgs : EventArgs
        {
            public string Path;
        }

        public class UpdatedEventArgs : EventArgs
        {
            public string OldVersion;
            public string NewVersion;
        }

        public class TrafficPerSecond
        {
            public long inboundCounter;
            public long outboundCounter;
            public long inboundIncreasement;
            public long outboundIncreasement;
        }

        public event EventHandler ConfigChanged;
        public event EventHandler EnableStatusChanged;
        public event EventHandler EnableGlobalChanged;
        public event EventHandler ShareOverLANStatusChanged;
        public event EventHandler VerboseLoggingStatusChanged;
        public event EventHandler ShowPluginOutputChanged;
        public event EventHandler TrafficChanged;

        // when user clicked Edit PAC, and PAC file has already created
        public event EventHandler<PathEventArgs> PACFileReadyToOpen;
        public event EventHandler<PathEventArgs> UserRuleFileReadyToOpen;

        public event EventHandler<GeositeResultEventArgs> UpdatePACFromGeositeCompleted;

        public event ErrorEventHandler UpdatePACFromGeositeError;

        public event ErrorEventHandler Errored;

        // Invoked when controller.Start();
        public event EventHandler<UpdatedEventArgs> ProgramUpdated;
        #endregion

        public ShadowsocksController()
        {
            logger = LogManager.GetCurrentClassLogger();
            httpClient = new HttpClient();
            _config = Configuration.Load();
            Configuration.Process(ref _config);
            _strategyManager = new StrategyManager(this);
            _pluginsByServer = new ConcurrentDictionary<Server, Sip003Plugin>();
            StartTrafficStatistics(61);

            ProgramUpdated += (o, e) =>
            {
                // version update precedures
                if (e.OldVersion == "4.3.0.0" || e.OldVersion == "4.3.1.0")
                    _config.geositeDirectGroups.Add("private");

                logger.Info($"Updated from {e.OldVersion} to {e.NewVersion}");
            };
        }

        #region Basic

        public void Start(bool systemWakeUp = false)
        {
            if (_config.firstRunOnNewVersion && !systemWakeUp)
            {
                ProgramUpdated.Invoke(this, new UpdatedEventArgs()
                {
                    OldVersion = _config.version,
                    NewVersion = UpdateChecker.Version,
                });
                // delete pac.txt when regeneratePacOnUpdate is true
                if (_config.regeneratePacOnUpdate)
                    try
                    {
                        File.Delete(PACDaemon.PAC_FILE);
                        logger.Info("Deleted pac.txt from previous version.");
                    }
                    catch (Exception e)
                    {
                        logger.LogUsefulException(e);
                    }
                // finish up first run of new version
                _config.firstRunOnNewVersion = false;
                _config.version = UpdateChecker.Version;
                Configuration.Save(_config);
            }
            Reload();
            if (!systemWakeUp)
                HotkeyReg.RegAllHotkeys();
        }

        public void Stop()
        {
            if (stopped)
            {
                return;
            }
            stopped = true;
            if (_listener != null)
            {
                _listener.Stop();
            }
            StopPlugins();
            if (privoxyRunner != null)
            {
                privoxyRunner.Stop();
            }
            if (_config.enabled)
            {
                SystemProxy.Update(_config, true, null);
            }
            Encryption.RNG.Close();
        }

        protected void Reload()
        {
            Encryption.RNG.Reload();
            // some logic in configuration updated the config when saving, we need to read it again
            _config = Configuration.Load();
            Configuration.Process(ref _config);

            NLogConfig.LoadConfiguration();

            logger.Info($"WPF Localization Extension|Current culture: {LocalizeDictionary.CurrentCulture}");

            // set User-Agent for httpClient
            try
            {
                if (!string.IsNullOrWhiteSpace(_config.userAgentString))
                    httpClient.DefaultRequestHeaders.Add("User-Agent", _config.userAgentString);
            }
            catch
            {
                // reset userAgent to default and reapply
                Configuration.ResetUserAgent(_config);
                httpClient.DefaultRequestHeaders.Add("User-Agent", _config.userAgentString);
            }

            privoxyRunner = privoxyRunner ?? new PrivoxyRunner();

            _pacDaemon = _pacDaemon ?? new PACDaemon(_config);
            _pacDaemon.PACFileChanged += PacDaemon_PACFileChanged;
            _pacDaemon.UserRuleFileChanged += PacDaemon_UserRuleFileChanged;
            _pacServer = _pacServer ?? new PACServer(_pacDaemon);
            _pacServer.UpdatePACURL(_config); // So PACServer works when system proxy disabled.

            GeositeUpdater.ResetEvent();
            GeositeUpdater.UpdateCompleted += PacServer_PACUpdateCompleted;
            GeositeUpdater.Error += PacServer_PACUpdateError;

            _listener?.Stop();
            StopPlugins();

            // don't put PrivoxyRunner.Start() before pacServer.Stop()
            // or bind will fail when switching bind address from 0.0.0.0 to 127.0.0.1
            // though UseShellExecute is set to true now
            // http://stackoverflow.com/questions/10235093/socket-doesnt-close-after-application-exits-if-a-launched-process-is-open
            privoxyRunner.Stop();
            try
            {
                var strategy = GetCurrentStrategy();
                strategy?.ReloadServers();

                StartPlugin();
                privoxyRunner.Start(_config);

                TCPRelay tcpRelay = new TCPRelay(this, _config);
                tcpRelay.OnInbound += UpdateInboundCounter;
                tcpRelay.OnOutbound += UpdateOutboundCounter;
                tcpRelay.OnFailed += (o, e) => GetCurrentStrategy()?.SetFailure(e.server);

                UDPRelay udpRelay = new UDPRelay(this);
                List<Listener.IService> services = new List<Listener.IService>
                {
                    tcpRelay,
                    udpRelay,
                    _pacServer,
                    new PortForwarder(privoxyRunner.RunningPort)
                };
                _listener = new Listener(services);
                _listener.Start(_config);
            }
            catch (Exception e)
            {
                // translate Microsoft language into human language
                // i.e. An attempt was made to access a socket in a way forbidden by its access permissions => Port already in use
                if (e is SocketException se)
                {
                    if (se.SocketErrorCode == SocketError.AddressAlreadyInUse)
                    {
                        e = new Exception(I18N.GetString("Port {0} already in use", _config.localPort), e);
                    }
                    else if (se.SocketErrorCode == SocketError.AccessDenied)
                    {
                        e = new Exception(I18N.GetString("Port {0} is reserved by system", _config.localPort), e);
                    }
                }
                logger.LogUsefulException(e);
                ReportError(e);
            }

            ConfigChanged?.Invoke(this, new EventArgs());
            UpdateSystemProxy();
        }

        protected void SaveConfig(Configuration newConfig)
        {
            Configuration.Save(newConfig);
            Reload();
        }

        protected void ReportError(Exception e)
        {
            Errored?.Invoke(this, new ErrorEventArgs(e));
        }

        public HttpClient GetHttpClient() => httpClient;
        public Server GetCurrentServer() => _config.GetCurrentServer();
        public Configuration GetCurrentConfiguration() => _config;

        public Server GetAServer(IStrategyCallerType type, IPEndPoint localIPEndPoint, EndPoint destEndPoint)
        {
            IStrategy strategy = GetCurrentStrategy();
            if (strategy != null)
            {
                return strategy.GetAServer(type, localIPEndPoint, destEndPoint);
            }
            if (_config.index < 0)
            {
                _config.index = 0;
            }
            return GetCurrentServer();
        }

        public void SaveServers(List<Server> servers, int localPort, bool portableMode)
        {
            _config.configs = servers;
            _config.localPort = localPort;
            _config.portableMode = portableMode;
            Configuration.Save(_config);
        }

        public void SelectServerIndex(int index)
        {
            _config.index = index;
            _config.strategy = null;
            SaveConfig(_config);
        }

        public void ToggleShareOverLAN(bool enabled)
        {
            _config.shareOverLan = enabled;
            SaveConfig(_config);

            ShareOverLANStatusChanged?.Invoke(this, new EventArgs());
        }

        #endregion

        #region OS Proxy

        public void ToggleEnable(bool enabled)
        {
            _config.enabled = enabled;
            SaveConfig(_config);

            EnableStatusChanged?.Invoke(this, new EventArgs());
        }

        public void ToggleGlobal(bool global)
        {
            _config.global = global;
            SaveConfig(_config);

            EnableGlobalChanged?.Invoke(this, new EventArgs());
        }

        public void SaveProxy(ForwardProxyConfig proxyConfig)
        {
            _config.proxy = proxyConfig;
            SaveConfig(_config);
        }

        private void UpdateSystemProxy()
        {
            SystemProxy.Update(_config, false, _pacServer);
        }

        #endregion

        #region PAC

        private void PacDaemon_PACFileChanged(object sender, EventArgs e)
        {
            UpdateSystemProxy();
        }

        private void PacServer_PACUpdateCompleted(object sender, GeositeResultEventArgs e)
        {
            UpdatePACFromGeositeCompleted?.Invoke(this, e);
        }

        private void PacServer_PACUpdateError(object sender, ErrorEventArgs e)
        {
            UpdatePACFromGeositeError?.Invoke(this, e);
        }

        private static readonly IEnumerable<char> IgnoredLineBegins = new[] { '!', '[' };
        private void PacDaemon_UserRuleFileChanged(object sender, EventArgs e)
        {
            GeositeUpdater.MergeAndWritePACFile(_config.geositeDirectGroups, _config.geositeProxiedGroups, _config.geositePreferDirect);
            UpdateSystemProxy();
        }

        public void CopyPacUrl()
        {
            Clipboard.SetDataObject(_pacServer.PacUrl);
        }

        public void SavePACUrl(string pacUrl)
        {
            _config.pacUrl = pacUrl;
            SaveConfig(_config);

            ConfigChanged?.Invoke(this, new EventArgs());
        }

        public void UseOnlinePAC(bool useOnlinePac)
        {
            _config.useOnlinePac = useOnlinePac;
            SaveConfig(_config);

            ConfigChanged?.Invoke(this, new EventArgs());
        }

        public void TouchPACFile()
        {
            string pacFilename = _pacDaemon.TouchPACFile();

            PACFileReadyToOpen?.Invoke(this, new PathEventArgs() { Path = pacFilename });
        }

        public void TouchUserRuleFile()
        {
            string userRuleFilename = _pacDaemon.TouchUserRuleFile();

            UserRuleFileReadyToOpen?.Invoke(this, new PathEventArgs() { Path = userRuleFilename });
        }

        public void ToggleSecureLocalPac(bool enabled)
        {
            _config.secureLocalPac = enabled;
            SaveConfig(_config);

            ConfigChanged?.Invoke(this, new EventArgs());
        }

        public void ToggleRegeneratePacOnUpdate(bool enabled)
        {
            _config.regeneratePacOnUpdate = enabled;
            SaveConfig(_config);
            ConfigChanged?.Invoke(this, new EventArgs());
        }

        #endregion

        #region  SIP002

        public bool AskAddServerBySSURL(string ssURL)
        {
            var dr = MessageBox.Show(I18N.GetString("Import from URL: {0} ?", ssURL), I18N.GetString("Shadowsocks"), MessageBoxButtons.YesNo);
            if (dr == DialogResult.Yes)
            {
                if (AddServerBySSURL(ssURL))
                {
                    MessageBox.Show(I18N.GetString("Successfully imported from {0}", ssURL));
                    return true;
                }
                else
                {
                    MessageBox.Show(I18N.GetString("Failed to import. Please check if the link is valid."));
                }
            }
            return false;
        }

        public bool AddServerBySSURL(string ssURL)
        {
            try
            {
                if (string.IsNullOrWhiteSpace(ssURL))
                    return false;

                var servers = Server.GetServers(ssURL);
                if (servers == null || servers.Count == 0)
                    return false;

                foreach (var server in servers)
                {
                    _config.configs.Add(server);
                    if (server.warnLegacyUrl)
                        MessageBox.Show(I18N.GetString("Warning: importing {0} from a legacy ss:// link. Support for legacy ss:// links will be dropped in version 5. Make sure to update your ss:// links.", server.ToString()));
                }
                _config.index = _config.configs.Count - 1;
                SaveConfig(_config);
                return true;
            }
            catch (Exception e)
            {
                logger.LogUsefulException(e);
                return false;
            }
        }

        public string GetServerURLForCurrentServer()
        {
            return GetCurrentServer().GetURL(_config.generateLegacyUrl);
        }

        #endregion

        #region Misc

        public void ToggleVerboseLogging(bool enabled)
        {
            _config.isVerboseLogging = enabled;
            SaveConfig(_config);
            NLogConfig.LoadConfiguration(); // reload nlog

            VerboseLoggingStatusChanged?.Invoke(this, new EventArgs());
        }

        public void ToggleCheckingUpdate(bool enabled)
        {
            _config.autoCheckUpdate = enabled;
            Configuration.Save(_config);

            ConfigChanged?.Invoke(this, new EventArgs());
        }

        public void ToggleCheckingPreRelease(bool enabled)
        {
            _config.checkPreRelease = enabled;
            Configuration.Save(_config);
            ConfigChanged?.Invoke(this, new EventArgs());
        }

        public void SaveSkippedUpdateVerion(string version)
        {
            _config.skippedUpdateVersion = version;
            Configuration.Save(_config);
        }

        public void SaveLogViewerConfig(LogViewerConfig newConfig)
        {
            _config.logViewer = newConfig;
            newConfig.SaveSize();
            Configuration.Save(_config);

            ConfigChanged?.Invoke(this, new EventArgs());
        }

        public void SaveHotkeyConfig(HotkeyConfig newConfig)
        {
            _config.hotkey = newConfig;
            SaveConfig(_config);

            ConfigChanged?.Invoke(this, new EventArgs());
        }

        #endregion

        #region Strategy

        public void SelectStrategy(string strategyID)
        {
            _config.index = -1;
            _config.strategy = strategyID;
            SaveConfig(_config);
        }

        public IList<IStrategy> GetStrategies()
        {
            return _strategyManager.GetStrategies();
        }

        public IStrategy GetCurrentStrategy()
        {
            foreach (var strategy in _strategyManager.GetStrategies())
            {
                if (strategy.ID == _config.strategy)
                {
                    return strategy;
                }
            }
            return null;
        }

        public void UpdateInboundCounter(object sender, SSTransmitEventArgs args)
        {
            GetCurrentStrategy()?.UpdateLastRead(args.server);
            Interlocked.Add(ref _inboundCounter, args.length);
        }

        public void UpdateOutboundCounter(object sender, SSTransmitEventArgs args)
        {
            GetCurrentStrategy()?.UpdateLastWrite(args.server);
            Interlocked.Add(ref _outboundCounter, args.length);
        }

        #endregion

        #region SIP003

        private void StartPlugin()
        {
            var server = _config.GetCurrentServer();
            GetPluginLocalEndPointIfConfigured(server);
        }

        private void StopPlugins()
        {
            foreach (var serverAndPlugin in _pluginsByServer)
            {
                serverAndPlugin.Value?.Dispose();
            }
            _pluginsByServer.Clear();
        }

        public EndPoint GetPluginLocalEndPointIfConfigured(Server server)
        {
            var plugin = _pluginsByServer.GetOrAdd(
                server,
                x => Sip003Plugin.CreateIfConfigured(x, _config.showPluginOutput));

            if (plugin == null)
            {
                return null;
            }

            try
            {
                if (plugin.StartIfNeeded())
                {
                    logger.Info(
                        $"Started SIP003 plugin for {server.Identifier()} on {plugin.LocalEndPoint} - PID: {plugin.ProcessId}");
                }
            }
            catch (Exception ex)
            {
                logger.Error("Failed to start SIP003 plugin: " + ex.Message);
                throw;
            }

            return plugin.LocalEndPoint;
        }

        public void ToggleShowPluginOutput(bool enabled)
        {
            _config.showPluginOutput = enabled;
            SaveConfig(_config);

            ShowPluginOutputChanged?.Invoke(this, new EventArgs());
        }

        #endregion

        #region Traffic Statistics

        private void StartTrafficStatistics(int queueMaxSize)
        {
            trafficPerSecondQueue = new Queue<TrafficPerSecond>();
            for (int i = 0; i < queueMaxSize; i++)
            {
                trafficPerSecondQueue.Enqueue(new TrafficPerSecond());
            }
            _trafficThread = new Thread(new ThreadStart(() => TrafficStatistics(queueMaxSize)))
            {
                IsBackground = true
            };
            _trafficThread.Start();
        }

        private void TrafficStatistics(int queueMaxSize)
        {
            TrafficPerSecond previous, current;
            while (true)
            {
                previous = trafficPerSecondQueue.Last();
                current = new TrafficPerSecond
                {
                    inboundCounter = InboundCounter,
                    outboundCounter = OutboundCounter
                };
                current.inboundIncreasement = current.inboundCounter - previous.inboundCounter;
                current.outboundIncreasement = current.outboundCounter - previous.outboundCounter;

                trafficPerSecondQueue.Enqueue(current);
                if (trafficPerSecondQueue.Count > queueMaxSize)
                    trafficPerSecondQueue.Dequeue();

                TrafficChanged?.Invoke(this, new EventArgs());

                Thread.Sleep(1000);
            }
        }

        #endregion

        #region SIP008


        public async Task<int> UpdateOnlineConfigInternal(string url)
        {
            var onlineServer = await OnlineConfigResolver.GetOnline(url);
            _config.configs = Configuration.SortByOnlineConfig(
                _config.configs
                .Where(c => c.group != url)
                .Concat(onlineServer)
                );
            logger.Info($"updated {onlineServer.Count} server from {url}");
            return onlineServer.Count;
        }

        public async Task<bool> UpdateOnlineConfig(string url)
        {
            var selected = GetCurrentServer();
            try
            {
                int count = await UpdateOnlineConfigInternal(url);
            }
            catch (Exception e)
            {
                logger.LogUsefulException(e);
                return false;
            }
            _config.index = _config.configs.IndexOf(selected);
            SaveConfig(_config);
            return true;
        }

        public async Task<List<string>> UpdateAllOnlineConfig()
        {
            var selected = GetCurrentServer();
            var failedUrls = new List<string>();
            foreach (var url in _config.onlineConfigSource)
            {
                try
                {
                    await UpdateOnlineConfigInternal(url);
                }
                catch (Exception e)
                {
                    logger.LogUsefulException(e);
                    failedUrls.Add(url);
                }
            }

            _config.index = _config.configs.IndexOf(selected);
            SaveConfig(_config);
            return failedUrls;
        }

        public void SaveOnlineConfigSource(List<string> sources)
        {
            _config.onlineConfigSource = sources;
            SaveConfig(_config);
        }

        public void RemoveOnlineConfig(string url)
        {
            _config.onlineConfigSource.RemoveAll(v => v == url);
            _config.configs = Configuration.SortByOnlineConfig(
                _config.configs.Where(c => c.group != url)
                );
            SaveConfig(_config);
        }

        #endregion
    }
}
