namespace NetBridge.Desktop;

internal class Program
{
    public static EventWaitHandle ProgramStarted;

    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        if (OnStartup(args) == false)
        {
            Environment.Exit(0);
            return;
        }

        BuildAvaloniaApp()
            .StartWithClassicDesktopLifetime(args);
    }

    private static bool OnStartup(string[]? Args)
    {
        _ = new Mutex(true, "NetBridge.Desktop", out var bOnlyOneInstance);
        if (!bOnlyOneInstance)
        {
            return false;
        }

        return true;
    }

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
    {
        var builder = AppBuilder.Configure<App>()
           .UsePlatformDetect()
           .LogToTrace()
           .UseReactiveUI(_ => { });

        return builder;
    }
}
