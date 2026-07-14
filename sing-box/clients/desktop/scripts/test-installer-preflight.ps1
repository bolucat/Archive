param(
    [Parameter(Mandatory = $true)][string]$PreflightScript
)

$ErrorActionPreference = "Stop"
$temporaryRoot = Join-Path $env:TEMP "sing-box-installer-preflight-$PID"
$temporaryDrive = $null

function Get-SecurityDescriptor([string]$Path) {
    $security = Get-Acl -LiteralPath $Path
    return [Convert]::ToBase64String($security.GetSecurityDescriptorBinaryForm())
}

try {
    foreach ($driveLetter in @("Z", "Y", "X", "W", "V")) {
        if (-not (Test-Path -LiteralPath "$driveLetter`:\")) {
            $temporaryDrive = "$driveLetter`:"
            break
        }
    }
    if ($null -eq $temporaryDrive) {
        throw "No temporary drive letter is available."
    }

    $parentDirectory = Join-Path $temporaryRoot "parent"
    $installationDirectory = Join-Path $parentDirectory "sing-box"
    $childDirectory = Join-Path $temporaryRoot "existing-child"
    $siblingDirectory = Join-Path $temporaryRoot "existing-sibling"
    [void](New-Item -ItemType Directory -Path $installationDirectory -Force)
    [void](New-Item -ItemType Directory -Path $childDirectory -Force)
    [void](New-Item -ItemType Directory -Path $siblingDirectory -Force)

    & "$env:SystemRoot\System32\icacls.exe" $temporaryRoot /setowner "*S-1-5-32-544" /T /C /Q
    if ($LASTEXITCODE -ne 0) {
        throw "Could not set the test directory owner: $LASTEXITCODE"
    }

    $administrators = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-32-544")
    $system = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-18")
    $rootAccessControl = Get-Acl -LiteralPath $temporaryRoot
    $rootAccessControl.SetAccessRuleProtection($true, $false)
    foreach ($identity in @($administrators, $system)) {
        $fullControlRule = [System.Security.AccessControl.FileSystemAccessRule]::new(
            $identity,
            [System.Security.AccessControl.FileSystemRights]::FullControl,
            [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
                [System.Security.AccessControl.InheritanceFlags]::ObjectInherit,
            [System.Security.AccessControl.PropagationFlags]::None,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
        [void]$rootAccessControl.AddAccessRule($fullControlRule)
    }
    Set-Acl -LiteralPath $temporaryRoot -AclObject $rootAccessControl

    & "$env:SystemRoot\System32\subst.exe" $temporaryDrive $temporaryRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Could not create the temporary drive: $LASTEXITCODE"
    }

    $mappedInstallationDirectory = "$temporaryDrive\parent\sing-box"
    $unchangedPaths = @(
        "$temporaryDrive\parent",
        $mappedInstallationDirectory,
        "$temporaryDrive\existing-child",
        "$temporaryDrive\existing-sibling"
    )
    $securityBeforeDirectoryOnlyWrite = @{}
    foreach ($path in $unchangedPaths) {
        $securityBeforeDirectoryOnlyWrite[$path] = Get-SecurityDescriptor $path
    }

    . $PreflightScript -InstallationDirectory $mappedInstallationDirectory

    $rootAccessControl = Get-Acl -LiteralPath "$temporaryDrive\"
    $authenticatedUsers = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-11")
    $safeInheritableRule = [System.Security.AccessControl.FileSystemAccessRule]::new(
        $authenticatedUsers,
        [System.Security.AccessControl.FileSystemRights]::ReadAndExecute,
        [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
            [System.Security.AccessControl.InheritanceFlags]::ObjectInherit,
        [System.Security.AccessControl.PropagationFlags]::None,
        [System.Security.AccessControl.AccessControlType]::Allow
    )
    $dangerousRule = [System.Security.AccessControl.FileSystemAccessRule]::new(
        $authenticatedUsers,
        [System.Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles,
        [System.Security.AccessControl.InheritanceFlags]::None,
        [System.Security.AccessControl.PropagationFlags]::None,
        [System.Security.AccessControl.AccessControlType]::Allow
    )
    [void]$rootAccessControl.AddAccessRule($safeInheritableRule)
    [void]$rootAccessControl.AddAccessRule($dangerousRule)
    $rootSecurityDescriptor = $rootAccessControl.GetSecurityDescriptorBinaryForm()
    [Box.Installer.DirectoryAccessControl]::SetCurrentDirectoryAccessControl(
        "$temporaryDrive\",
        $rootSecurityDescriptor
    )

    $securityBeforeRepair = @{}
    foreach ($path in $unchangedPaths) {
        $securityBeforeRepair[$path] = Get-SecurityDescriptor $path
        if ($securityBeforeRepair[$path] -ne $securityBeforeDirectoryOnlyWrite[$path]) {
            throw "The directory-only write changed a descendant or sibling security descriptor: $path"
        }
    }

    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -NoProfile `
        -NonInteractive `
        -ExecutionPolicy Bypass `
        -File $PreflightScript `
        -InstallationDirectory $mappedInstallationDirectory
    if ($LASTEXITCODE -ne 13) {
        throw "Expected unsafe ancestor exit code 13, received $LASTEXITCODE"
    }

    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -NoProfile `
        -NonInteractive `
        -ExecutionPolicy Bypass `
        -File $PreflightScript `
        -InstallationDirectory $mappedInstallationDirectory `
        -AllowUnsafeInstallationDirectory
    if ($LASTEXITCODE -ne 0) {
        throw "Explicit unsafe installation failed with exit code $LASTEXITCODE"
    }

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -NoProfile `
        -NonInteractive `
        -ExecutionPolicy Bypass `
        -File $PreflightScript `
        -InstallationDirectory $mappedInstallationDirectory `
        -RepairInstallationAncestors
    $stopwatch.Stop()
    if ($LASTEXITCODE -ne 0) {
        throw "Ancestor repair failed with exit code $LASTEXITCODE"
    }

    foreach ($path in $unchangedPaths) {
        $securityAfterRepair = Get-SecurityDescriptor $path
        if ($securityAfterRepair -ne $securityBeforeRepair[$path]) {
            throw "The repair changed a descendant or sibling security descriptor: $path"
        }
    }

    $safeRulePreserved = $false
    $repairedRootAccessControl = Get-Acl -LiteralPath "$temporaryDrive\"
    foreach ($rule in $repairedRootAccessControl.Access) {
        if ($rule.IdentityReference.Translate(
                [System.Security.Principal.SecurityIdentifier]
            ).Value -eq $authenticatedUsers.Value -and
            ($rule.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::ReadAndExecute) -eq
                [System.Security.AccessControl.FileSystemRights]::ReadAndExecute -and
            $rule.InheritanceFlags -eq (
                [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
                [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
            )) {
            $safeRulePreserved = $true
        }
    }
    if (-not $safeRulePreserved) {
        throw "The repair removed the safe inheritable access rule."
    }

    "repairMilliseconds=$($stopwatch.ElapsedMilliseconds)"
    "unchangedDescriptors=$($unchangedPaths.Count)"
} finally {
    if ($null -ne $temporaryDrive) {
        & "$env:SystemRoot\System32\subst.exe" $temporaryDrive /D 2>$null
    }
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
