param(
    [Parameter(Mandatory = $true)]
    [string]$ExecutablePath,

    [Parameter(Mandatory = $true)]
    [ValidateSet("install", "uninstall")]
    [string]$ServiceAction,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [switch]$AllowUnsafeInstallationDirectoryPermissions
)

$ErrorActionPreference = "Stop"
$commandArguments = "service $ServiceAction"
if ($AllowUnsafeInstallationDirectoryPermissions) {
    $commandArguments += " --allow-unsafe-installation-directory-permissions"
}

try {
    $processStartInformation = New-Object System.Diagnostics.ProcessStartInfo
    $processStartInformation.FileName = $ExecutablePath
    $processStartInformation.Arguments = $commandArguments
    $processStartInformation.WorkingDirectory = Split-Path -Parent $ExecutablePath
    $processStartInformation.UseShellExecute = $false
    $processStartInformation.CreateNoWindow = $true
    $processStartInformation.RedirectStandardOutput = $true
    $processStartInformation.RedirectStandardError = $true

    $process = [System.Diagnostics.Process]::Start($processStartInformation)
    if ($null -eq $process) {
        throw "The sing-box service command did not start."
    }
    $standardOutputTask = $process.StandardOutput.ReadToEndAsync()
    $standardErrorTask = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $standardOutput = $standardOutputTask.Result.TrimEnd()
    $standardError = $standardErrorTask.Result.TrimEnd()
    $processExitCode = $process.ExitCode
    $process.Dispose()

    $diagnosticOutput = $standardOutput
    if ($standardError.Length -gt 0) {
        if ($diagnosticOutput.Length -gt 0) {
            $diagnosticOutput += [Environment]::NewLine
        }
        $diagnosticOutput += $standardError
    }
    if ($processExitCode -ne 0 -and $diagnosticOutput.Length -eq 0) {
        $diagnosticOutput = "sing-box service $ServiceAction exited with code $processExitCode without diagnostic output."
    }
    [System.IO.File]::WriteAllText($OutputPath, $diagnosticOutput, [System.Text.Encoding]::Unicode)
    exit $processExitCode
} catch {
    $exception = $_.Exception
    $nativeErrorCode = $null
    $currentException = $exception
    while ($null -ne $currentException) {
        if ($currentException -is [System.ComponentModel.Win32Exception]) {
            $nativeErrorCode = $currentException.NativeErrorCode
            break
        }
        $currentException = $currentException.InnerException
    }
    if ($null -ne $nativeErrorCode) {
        $diagnosticOutput = "Failed to start sing-box service $ServiceAction (Win32 error $nativeErrorCode): $($exception.Message)"
    } else {
        $diagnosticOutput = "Failed to start sing-box service $ServiceAction`: $($exception.Message)"
    }
    [System.IO.File]::WriteAllText($OutputPath, $diagnosticOutput, [System.Text.Encoding]::Unicode)
    if ($null -ne $nativeErrorCode -and $nativeErrorCode -gt 0) {
        exit $nativeErrorCode
    }
    exit 255
}
