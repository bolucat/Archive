using System.Runtime.InteropServices;

namespace GlobalHotKeys;

public class NativeFunctions
{
    [DllImport("user32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern bool RegisterHotKey(IntPtr hWnd, int id, Modifiers fsModifiers, VirtualKeyCode vk);

    [DllImport("user32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern bool UnregisterHotKey(IntPtr hWnd, int id);

    [DllImport("user32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern IntPtr DefWindowProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern int RegisterClassEx(ref WNDCLASSEX lpwcx);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto, CallingConvention = CallingConvention.StdCall)]
    public static extern IntPtr CreateWindowEx(int dwExStyle, uint regResult, string lpWindowName, WindowStyle dwStyle, int x, int y, int nWidth, int nHeight, IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);

    [DllImport("user32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern bool DestroyWindow(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern bool UnregisterClass(string lpClassName, IntPtr hInstance);

    [DllImport("user32.dll", CallingConvention = CallingConvention.StdCall)]
    public static extern int GetMessage(ref TagMSG lpMsg, IntPtr hWnd, uint wMsgFilterMin, uint wMsgFilterMax);

    [DllImport("user32.dll", CallingConvention = CallingConvention.StdCall)]
    public static extern bool TranslateMessage(ref TagMSG lpMsg);

    [DllImport("user32.dll", CallingConvention = CallingConvention.StdCall)]
    public static extern IntPtr DispatchMessage(ref TagMSG lpmsg);

    [DllImport("kernel32.dll", SetLastError = true, CallingConvention = CallingConvention.StdCall)]
    public static extern IntPtr GetModuleHandle(string? lpModuleName);

    [DllImport("user32.dll", CallingConvention = CallingConvention.StdCall)]
    public static extern IntPtr SendMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", CallingConvention = CallingConvention.StdCall)]
    public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
}
