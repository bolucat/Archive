using System.Security.Principal;
using Avalonia.Input.Platform;

namespace NetBridge.Desktop.Common;

internal class AvaUtils
{
    public static async Task<string?> GetClipboardData(Window owner)
    {
        try
        {
            var clipboard = TopLevel.GetTopLevel(owner)?.Clipboard;
            if (clipboard == null)
            {
                return null;
            }

            return await clipboard.TryGetTextAsync();
        }
        catch
        {
            return null;
        }
    }

    public static async Task SetClipboardData(Visual? visual, string strData)
    {
        try
        {
            var clipboard = TopLevel.GetTopLevel(visual)?.Clipboard;
            if (clipboard == null)
            {
                return;
            }

            await clipboard.SetTextAsync(strData);
        }
        catch
        {
        }
    }

    public static bool IsAdministrator()
    {
        return new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);
    }
}
