# Read-only GUI smoke; never invokes Save or calls a firewall write API.
param([string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\enableLoopback.exe'))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class LoopbackSmokeNative {
 [StructLayout(LayoutKind.Sequential)] struct Array { public uint Count; public IntPtr Values; }
 [StructLayout(LayoutKind.Sequential)] struct App { public IntPtr Sid, User, Name, Display, Description; public Array Capabilities, Binaries; public IntPtr Directory, Package; }
 [StructLayout(LayoutKind.Sequential)] struct SidAttributes { public IntPtr Sid; public uint Attributes; }
 [DllImport("FirewallAPI.dll")] static extern uint NetworkIsolationEnumAppContainers(uint flags, out uint count, out IntPtr apps);
 [DllImport("FirewallAPI.dll")] static extern uint NetworkIsolationFreeAppContainers(IntPtr apps);
 [DllImport("FirewallAPI.dll")] static extern uint NetworkIsolationGetAppContainerConfig(out uint count, out IntPtr sids);
 [DllImport("kernel32.dll")] static extern IntPtr GetProcessHeap();
 [DllImport("kernel32.dll")] static extern bool HeapFree(IntPtr heap, uint flags, IntPtr memory);
 [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("advapi32.dll")] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("advapi32.dll")] static extern bool GetTokenInformation(IntPtr token,int kind,out int value,int size,out int required);
 public static bool IsElevated(int pid) {
   const uint PROCESS_QUERY_LIMITED_INFORMATION=0x1000, TOKEN_QUERY=0x0008; // winnt.h
   const int TokenElevation=20; // TOKEN_INFORMATION_CLASS, winnt.h
   IntPtr process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid), token=IntPtr.Zero;
   try { if(process==IntPtr.Zero || !OpenProcessToken(process,TOKEN_QUERY,out token)) throw new Exception("Cannot read process token"); int value,required; if(!GetTokenInformation(token,TokenElevation,out value,4,out required)) throw new Exception("Cannot read TokenElevation"); return value!=0; }
   finally { if(token!=IntPtr.Zero) CloseHandle(token); if(process!=IntPtr.Zero) CloseHandle(process); }
 }
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, UIntPtr wparam, IntPtr lparam);
 const uint ERROR_SUCCESS=0; // winerror.h
 const uint WM_CLOSE = 0x0010; // winuser.h
 public static void Close(IntPtr hwnd) { if (!PostMessage(hwnd,WM_CLOSE,UIntPtr.Zero,IntPtr.Zero)) throw new Exception("PostMessage(WM_CLOSE) failed"); }
 public static int EnumerateCount() {
   uint count; IntPtr apps; uint code=NetworkIsolationEnumAppContainers(0,out count,out apps);
   try {
     if (code!=ERROR_SUCCESS) throw new Exception("Enumeration 0x"+code.ToString("x"));
     var unique=new HashSet<string>(StringComparer.Ordinal);
     for(int i=0;i<count;i++) { var app=(App)Marshal.PtrToStructure(IntPtr.Add(apps,i*Marshal.SizeOf(typeof(App))),typeof(App)); unique.Add(new System.Security.Principal.SecurityIdentifier(app.Sid).Value); }
     return unique.Count;
   } finally { if(apps!=IntPtr.Zero) NetworkIsolationFreeAppContainers(apps); }
 }
 public static string Exemptions() {
   uint count; IntPtr data; uint code=NetworkIsolationGetAppContainerConfig(out count,out data);
   if(code!=ERROR_SUCCESS) throw new Exception("Config read 0x"+code.ToString("x"));
   var values=new List<string>(); var heap=GetProcessHeap();
   try {
     for(int i=0;i<count;i++) { var entry=(SidAttributes)Marshal.PtrToStructure(IntPtr.Add(data,i*Marshal.SizeOf(typeof(SidAttributes))),typeof(SidAttributes)); try { values.Add(new System.Security.Principal.SecurityIdentifier(entry.Sid).Value); } finally { HeapFree(heap,0,entry.Sid); } }
   } finally { if(data!=IntPtr.Zero) HeapFree(heap,0,data); }
   values.Sort(StringComparer.Ordinal); return String.Join("\n",values);
 }
}
"@
function Assert($condition, $message) { if (!$condition) { throw $message } }
function Wait-Until([scriptblock]$condition, [string]$description) {
 $deadline = [DateTime]::UtcNow.AddSeconds(20)
 do { if (& $condition) { return }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $deadline)
 throw "Timeout: $description"
}
function Find-Id($id) { $script:window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::AutomationIdProperty,$id)) }
function Find-Name($name) { $script:window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty,$name)) }
function Invoke-Id($id) { $element=Find-Id $id; Assert ($null -ne $element) "Missing $id"; $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
function Counter { (Find-Id 'counter').Current.Name }
function Save-Enabled { (Find-Id 'save').Current.IsEnabled }
$before = [LoopbackSmokeNative]::Exemptions()
$expected = [LoopbackSmokeNative]::EnumerateCount()
"enumerate(false) unique SIDs=$expected"
$env:__COMPAT_LAYER = 'RunAsInvoker'
$process = Start-Process -FilePath (Resolve-Path $Executable) -ArgumentList '--lang en' -WorkingDirectory (Split-Path $PSScriptRoot) -WindowStyle Hidden -PassThru
"Started PID=$($process.Id) with __COMPAT_LAYER=RunAsInvoker"
$script:window=$null
try {
 Wait-Until {
  $script:window=[System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$process.Id))
  $null -ne $script:window -and $null -ne (Find-Id 'apps') -and (Counter) -match " / $expected`$" -and (Find-Id 'refresh').Current.IsEnabled
 } 'loaded window'
 "Title=$($window.Current.Name)"
 $elevated=[LoopbackSmokeNative]::IsElevated($process.Id)
 "Token elevated=$elevated"
 Assert (!$elevated) 'Smoke must run unelevated'
 Assert ($window.Current.Name -eq 'AppContainer Loopback Exemption Tool') 'Wrong title'
 $initial = Counter
 "Loaded: counter=$initial; Save=$(Save-Enabled)"
 Assert (!(Save-Enabled)) 'Save must be disabled after load'
 $apps=Find-Id 'apps'
 $container=$apps.GetCurrentPattern([System.Windows.Automation.ItemContainerPattern]::Pattern)
 $next=$null; $count=0
 do { $next=$container.FindItemByProperty($next,$null,$null); if($null -ne $next) { $count++ } } while ($null -ne $next -and $count -lt 10000)
 Assert ($count -eq $expected) "UIA count $count differs from enumerate(false) $expected"
 "UIA ItemContainerPattern count=$count"
 foreach ($header in @('Display name ▲','Description','Package','AC name','AC SID','Users','Binaries')) { Assert ($null -ne (Find-Name $header)) "Missing header $header" }
 'Seven header buttons present'
 # Exercise native container rebuilding and offscreen SID restoration, in both directions.
 $middle=$null
 for($i=0;$i -le [int]($count/2);$i++) { $middle=$container.FindItemByProperty($middle,$null,$null) }
 $virtualized=$null
 if($middle.TryGetCurrentPattern([System.Windows.Automation.VirtualizedItemPattern]::Pattern,[ref]$virtualized)) { $virtualized.Realize() }
 $middle.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
 function Row-Sid($row) { ($row.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -match '^S-1-15-' } | Select-Object -First 1).Current.Name }
 $selectedSid=Row-Sid $middle
 Assert (![string]::IsNullOrEmpty($selectedSid)) 'Selected row SID missing'
 foreach($header in @('Display name ▲','Display name ▼')) {
  (Find-Name $header).GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
  Wait-Until {
   $newList=Find-Id 'apps'
   if(!$newList) { return $false }
   $selection=$newList.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection()
   $selection.Count -eq 1 -and !$selection[0].Current.IsOffscreen -and (Row-Sid $selection[0]) -eq $selectedSid
  } 'sort restores and reveals the selected SID'
 }
 'Sort round trip preserves the selected SID and reveals its row'
 $apps=Find-Id 'apps'

 $checkbox=$apps.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::CheckBox))
 $toggle=$checkbox.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
 $toggle.Toggle()
 Wait-Until { Save-Enabled } 'dirty Save enabled'
 "Toggle: counter=$(Counter); Save=$(Save-Enabled); status=$((Find-Id 'status').Current.Name)"
 Assert ((Counter) -ne $initial) 'Counter did not change'
 Invoke-Id 'refresh'
 Wait-Until { $null -ne (Find-Id 'CloseButton') } 'refresh confirmation'
 "Refresh confirmation=$((Find-Name 'Discard changes?').Current.Name)"
 Invoke-Id 'CloseButton'
 Wait-Until { $null -eq (Find-Id 'CloseButton') } 'cancel refresh'
 Assert (Save-Enabled) 'Cancel refresh lost dirty edits'
 $toggle.Toggle()
 Wait-Until { !(Save-Enabled) -and (Counter) -eq $initial } 'reverted clean'
 "Toggle back: counter=$(Counter); Save=$(Save-Enabled)"
 Invoke-Id 'refresh'
 Wait-Until { (Find-Id 'refresh').Current.IsEnabled -and (Find-Id 'status').Current.Name -like 'Refreshed AppContainer*' } 'refresh completion'
 "Refresh completed: counter=$(Counter); status=$((Find-Id 'status').Current.Name)"
 $apps=Find-Id 'apps'
 $row=$apps.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::ListItem))
 $row.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
 Assert ($row.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Current.IsSelected) 'SelectionPattern failed'
 'UIA SelectionItemPattern.Select works'
 $checkbox=$apps.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::CheckBox))
 $toggle=$checkbox.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
 $toggle.Toggle(); Wait-Until { Save-Enabled } 'dirty close setup'
 [LoopbackSmokeNative]::Close([IntPtr]$window.Current.NativeWindowHandle)
 Wait-Until { $null -ne (Find-Name 'Exit without saving?') } 'WM_CLOSE confirmation'
 'Dirty WM_CLOSE: Exit without saving? visible; window remains open'
 Invoke-Id 'CloseButton'
 Wait-Until { $null -eq (Find-Id 'CloseButton') } 'cancel close'
 Assert (Save-Enabled) 'Cancel close lost dirty edits'
 'Cancel close preserves edits'
 $toggle.Toggle(); Wait-Until { !(Save-Enabled) } 'clean close setup'
 [LoopbackSmokeNative]::Close([IntPtr]$window.Current.NativeWindowHandle)
 Assert ($process.WaitForExit(10000)) 'Clean close did not exit'
 'Clean WM_CLOSE exits without a dialog'
 $bitmap=[System.Drawing.Bitmap]::new([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width,[System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height)
 $graphics=[System.Drawing.Graphics]::FromImage($bitmap)
 try { $graphics.CopyFromScreen(0,0,0,0,$bitmap.Size); $bitmap.Save((Join-Path $PSScriptRoot '..\target\stage3\gui-screen.png')); 'CopyFromScreen succeeded; inspect target/stage3/gui-screen.png' }
 catch { 'CopyFromScreen failed (locked desktop): ' + $_.Exception.Message }
 finally { $graphics.Dispose(); $bitmap.Dispose() }
 Assert ([LoopbackSmokeNative]::Exemptions() -ceq $before) 'Exemption table changed during smoke'
 'PASS: firewall exemption table unchanged; Save was never invoked'
} finally {
 if (!$process.HasExited) { Stop-Process -Id $process.Id; "Stopped only owned PID=$($process.Id)" }
}
