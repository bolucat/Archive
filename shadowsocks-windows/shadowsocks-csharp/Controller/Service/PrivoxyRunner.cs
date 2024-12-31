﻿using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Windows.Forms;
using NLog;
using Shadowsocks.Model;
using Shadowsocks.Properties;
using Shadowsocks.Util;
using Shadowsocks.Util.ProcessManagement;

namespace Shadowsocks.Controller
{
    class PrivoxyRunner
    {
        private static Logger logger = LogManager.GetCurrentClassLogger();

        private static int _uid;
        private static string _uniqueConfigFile;
        private static Job _privoxyJob;
        private Process _process;
        private int _runningPort;

        static PrivoxyRunner()
        {
            try
            {
                _uid = Program.WorkingDirectory.GetHashCode(); // Currently we use ss's StartupPath to identify different Privoxy instance.
                _uniqueConfigFile = $"privoxy_{_uid}.conf";
                _privoxyJob = new Job();

                FileManager.UncompressFile(Utils.GetTempPath("ss_privoxy.exe"), Resources.privoxy_exe);
            }
            catch (IOException e)
            {
                logger.LogUsefulException(e);
            }
        }

        public int RunningPort => _runningPort;

        public void Start(Configuration configuration)
        {
            if (_process == null)
            {
                Process[] existingPrivoxy = Process.GetProcessesByName("ss_privoxy");
                foreach (Process p in existingPrivoxy.Where(IsChildProcess))
                {
                    KillProcess(p);
                }
                string privoxyConfig = Resources.privoxy_conf;
                _runningPort = GetFreePort(configuration.isIPv6Enabled);
                privoxyConfig = privoxyConfig.Replace("__SOCKS_PORT__", configuration.localPort.ToString());
                privoxyConfig = privoxyConfig.Replace("__PRIVOXY_BIND_PORT__", _runningPort.ToString());
                privoxyConfig = configuration.isIPv6Enabled
                    ? privoxyConfig.Replace("__PRIVOXY_BIND_IP__", configuration.shareOverLan ? "[::]" : "[::1]")
                    .Replace("__SOCKS_HOST__", "[::1]")
                    : privoxyConfig.Replace("__PRIVOXY_BIND_IP__", configuration.shareOverLan ? "0.0.0.0" : "127.0.0.1")
                    .Replace("__SOCKS_HOST__", "127.0.0.1");
                FileManager.ByteArrayToFile(Utils.GetTempPath(_uniqueConfigFile), Encoding.UTF8.GetBytes(privoxyConfig));

                _process = new Process
                {
                    // Configure the process using the StartInfo properties.
                    StartInfo =
                    {
                        FileName = "ss_privoxy.exe",
                        Arguments = _uniqueConfigFile,
                        WorkingDirectory = Utils.GetTempPath(),
                        WindowStyle = ProcessWindowStyle.Hidden,
                        UseShellExecute = true,
                        CreateNoWindow = true
                    }
                };
                _process.Start();

                /*
                 * Add this process to job obj associated with this ss process, so that
                 * when ss exit unexpectedly, this process will be forced killed by system.
                 */
                _privoxyJob.AddProcess(_process.Handle);
            }
        }

        public void Stop()
        {
            if (_process != null)
            {
                KillProcess(_process);
                _process.Dispose();
                _process = null;
            }
        }

        private static void KillProcess(Process p)
        {
            try
            {
                p.CloseMainWindow();
                p.WaitForExit(100);
                if (!p.HasExited)
                {
                    p.Kill();
                    p.WaitForExit();
                }
            }
            catch (Exception e)
            {
                logger.LogUsefulException(e);
            }
        }

        /*
         * We won't like to kill other ss instances' ss_privoxy.exe.
         * This function will check whether the given process is created
         * by this process by checking the module path or command line.
         * 
         * Since it's required to put ss in different dirs to run muti instances,
         * different instance will create their unique "privoxy_UID.conf" where
         * UID is hash of ss's location.
         */

        private static bool IsChildProcess(Process process)
        {
            try
            {
                /*
                 * Under PortableMode, we could identify it by the path of ss_privoxy.exe.
                 */
                var path = process.MainModule.FileName;

                return Utils.GetTempPath("ss_privoxy.exe").Equals(path);

            }
            catch (Exception ex)
            {
                /*
                 * Sometimes Process.GetProcessesByName will return some processes that
                 * are already dead, and that will cause exceptions here.
                 * We could simply ignore those exceptions.
                 */
                logger.LogUsefulException(ex);
                return false;
            }
        }

        private int GetFreePort(bool isIPv6 = false)
        {
            int defaultPort = 8123;
            try
            {
                // TCP stack please do me a favor
                TcpListener l = new TcpListener(isIPv6 ? IPAddress.IPv6Loopback : IPAddress.Loopback, 0);
                l.Start();
                var port = ((IPEndPoint)l.LocalEndpoint).Port;
                l.Stop();
                return port;
            }
            catch (Exception e)
            {
                // in case access denied
                logger.LogUsefulException(e);
                return defaultPort;
            }
        }
    }
}
