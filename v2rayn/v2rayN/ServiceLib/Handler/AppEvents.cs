using System.Reactive;
using System.Reactive.Subjects;

namespace ServiceLib.Handler;

public static class AppEvents
{
    public static readonly Subject<Unit> ProfilesRefreshRequested = new();

    public static readonly Subject<string> SendSnackMsgRequested = new();

    public static readonly Subject<string> SendMsgViewRequested = new();

    public static readonly Subject<Unit> AppExitRequested = new();
}
