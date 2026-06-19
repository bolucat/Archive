namespace NetBridge.Desktop.Base;

public class WindowBase<TViewModel> : ReactiveWindow<TViewModel> where TViewModel : class
{
    public WindowBase()
    {
        Loaded += OnLoaded;
    }

    protected virtual void OnLoaded(object? sender, RoutedEventArgs e)
    {
    }

    protected override void OnClosed(EventArgs e)
    {
        base.OnClosed(e);
    }
}
