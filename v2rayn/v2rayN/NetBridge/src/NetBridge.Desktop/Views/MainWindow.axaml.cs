using NetBridge.Desktop.Base;
using NetBridge.Desktop.ViewModels;

namespace NetBridge.Desktop.Views;

public partial class MainWindow : WindowBase<MainWindowViewModel>
{
    public MainWindow()
    {
        InitializeComponent();

        ViewModel = new ViewModels.MainWindowViewModel();

        this.WhenActivated(disposables =>
        {
            this.BindCommand(ViewModel, vm => vm.ToggleProxyCmd, v => v.btnToggleProxy).DisposeWith(disposables);
            this.BindCommand(ViewModel, vm => vm.ApplyProxyConfigCmd, v => v.btnApplyProxyConfig).DisposeWith(disposables);
            this.BindCommand(ViewModel, vm => vm.SaveRuleCmd, v => v.btnSaveRule).DisposeWith(disposables);

            this.Bind(ViewModel, vm => vm.ToggleServiceButtonText, v => v.btnToggleProxy.Content).DisposeWith(disposables);

            this.Bind(ViewModel, vm => vm.ProxyConfigSource.ProxyType, v => v.txtProxyType.Text).DisposeWith(disposables);
            this.Bind(ViewModel, vm => vm.ProxyConfigSource.ProxyHost, v => v.txtProxyHost.Text).DisposeWith(disposables);
            this.Bind(ViewModel, vm => vm.ProxyConfigSource.ProxyPort, v => v.txtProxyPort.Text, port => port.ToString(), text => ushort.TryParse(text, out var p) ? p : (ushort)0).DisposeWith(disposables);
            this.Bind(ViewModel, vm => vm.ProxyConfigSource.ProxyUsername, v => v.txtProxyUsername.Text).DisposeWith(disposables);
            this.Bind(ViewModel, vm => vm.ProxyConfigSource.ProxyPassword, v => v.txtProxyPassword.Text).DisposeWith(disposables);
            this.Bind(ViewModel, vm => vm.RuleProcessName, v => v.txtRuleProcessName.Text).DisposeWith(disposables);
        });
    }
}
