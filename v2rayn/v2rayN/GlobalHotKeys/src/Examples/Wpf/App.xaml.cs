using GlobalHotKeys;
using System;
using System.Reactive.Linq;
using System.Threading;
using System.Windows;

namespace Wpf
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        private HotKeyManager _hotKeyManager;
        private IDisposable _shift1;
        private IDisposable _subscription;

        private void App_OnStartup(object sender, StartupEventArgs e)
        {
            _hotKeyManager = new HotKeyManager();
            _shift1 = _hotKeyManager.Register(VirtualKeyCode.KEY_1, Modifiers.Shift);

            var mainViewModel = new MainViewModel();
            this.MainWindow = new MainWindow { DataContext = mainViewModel };
            this.MainWindow.Show();

            _subscription = _hotKeyManager.HotKeyPressed
              .ObserveOn(SynchronizationContext.Current)
              .Subscribe(hotKey =>
                mainViewModel.Text += $"hotKey: Id = {hotKey.Id}, Key = {hotKey.Key}, Modifiers = {hotKey.Modifiers}{Environment.NewLine}");
        }

        private void App_OnExit(object sender, ExitEventArgs e)
        {
            _subscription.Dispose();
            _shift1.Dispose();
            ((IDisposable)_hotKeyManager).Dispose();
        }
    }
}