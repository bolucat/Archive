param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Prepare", "Commit", "Rollback", "Recover")]
    [string]$Operation,

    [Parameter(Mandatory = $true)][string]$StatePath,
    [string]$PreviousApplicationDataDirectory,
    [string]$ApplicationDataDirectory,
    [string]$PreviousDaemonDataDirectory,
    [string]$DaemonDataDirectory,
    [string]$PreviousInstallationID,
    [string]$InstallationID,
    [switch]$MigrateLegacyApplicationData
)

$ErrorActionPreference = "Stop"
$reparsePoint = [System.IO.FileAttributes]::ReparsePoint

function Initialize-TransitionStateStorage() {
    $commonApplicationData = [Environment]::GetFolderPath(
        [Environment+SpecialFolder]::CommonApplicationData
    )
    $expectedPath = Join-Path `
        (Join-Path $commonApplicationData "sing-box-installer") `
        "data-transition.json"
    if (-not [System.IO.Path]::GetFullPath($StatePath).Equals(
            [System.IO.Path]::GetFullPath($expectedPath),
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "The data migration state path is invalid."
    }
    $stateDirectory = Split-Path -Parent $expectedPath
    if (Test-Path -LiteralPath $stateDirectory) {
        $stateDirectoryItem = Get-Item -LiteralPath $stateDirectory -Force
        if (-not $stateDirectoryItem.PSIsContainer -or
            ($stateDirectoryItem.Attributes -band $reparsePoint)) {
            throw "The data migration state directory is invalid."
        }
    } else {
        [void][System.IO.Directory]::CreateDirectory($stateDirectory)
    }

    $system = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-18")
    $administrators = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-32-544")
    & "$env:SystemRoot\System32\icacls.exe" `
        $stateDirectory `
        /setowner `
        "*S-1-5-32-544" `
        /Q | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Could not secure the data migration state directory: $LASTEXITCODE"
    }
    $security = [System.Security.AccessControl.DirectorySecurity]::new()
    $security.SetOwner($administrators)
    $security.SetAccessRuleProtection($true, $false)
    $inheritance = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
        [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
    foreach ($identity in @($system, $administrators)) {
        $rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
            $identity,
            [System.Security.AccessControl.FileSystemRights]::FullControl,
            $inheritance,
            [System.Security.AccessControl.PropagationFlags]::None,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
        [void]$security.AddAccessRule($rule)
    }
    Set-Acl -LiteralPath $stateDirectory -AclObject $security

    if ([System.IO.File]::Exists($StatePath)) {
        $stateItem = Get-Item -LiteralPath $StatePath -Force
        if ($stateItem.Attributes -band $reparsePoint) {
            throw "The data migration state file is invalid."
        }
        $stateSecurity = Get-Acl -LiteralPath $StatePath
        $trustedIdentities = @(
            "S-1-5-18",
            "S-1-5-32-544",
            [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        )
        $owner = $stateSecurity.GetOwner(
            [System.Security.Principal.SecurityIdentifier]
        ).Value
        if ($owner -notin $trustedIdentities) {
            throw "The data migration state file has an untrusted owner."
        }
        foreach ($rule in $stateSecurity.GetAccessRules(
                $true,
                $true,
                [System.Security.Principal.SecurityIdentifier]
            )) {
            if ($rule.IdentityReference.Value -notin $trustedIdentities) {
                throw "The data migration state file has unsafe permissions."
            }
        }
    }
}

function Test-SamePath([string]$FirstPath, [string]$SecondPath) {
    if ([string]::IsNullOrWhiteSpace($FirstPath) -or
        [string]::IsNullOrWhiteSpace($SecondPath)) {
        return [string]::IsNullOrWhiteSpace($FirstPath) -and
            [string]::IsNullOrWhiteSpace($SecondPath)
    }
    $first = [System.IO.Path]::GetFullPath($FirstPath).TrimEnd("\")
    $second = [System.IO.Path]::GetFullPath($SecondPath).TrimEnd("\")
    return $first.Equals($second, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-PathOverlap([string]$FirstPath, [string]$SecondPath) {
    $first = [System.IO.Path]::GetFullPath($FirstPath).TrimEnd("\")
    $second = [System.IO.Path]::GetFullPath($SecondPath).TrimEnd("\")
    return $first.Equals($second, [System.StringComparison]::OrdinalIgnoreCase) -or
        $first.StartsWith($second + "\", [System.StringComparison]::OrdinalIgnoreCase) -or
        $second.StartsWith($first + "\", [System.StringComparison]::OrdinalIgnoreCase)
}

function Assert-NormalDirectoryTree([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    $root = Get-Item -LiteralPath $Path -Force
    if (-not $root.PSIsContainer -or ($root.Attributes -band $reparsePoint)) {
        throw "The data path is not a normal directory: $Path"
    }
    $reparseEntry = Get-ChildItem -LiteralPath $Path -Force -Recurse |
        Where-Object { $_.Attributes -band $reparsePoint } |
        Select-Object -First 1
    if ($null -ne $reparseEntry) {
        throw "The data directory contains a reparse point: $($reparseEntry.FullName)"
    }
}

function Test-DirectoryEmpty([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return $true
    }
    $entry = Get-ChildItem -LiteralPath $Path -Force | Select-Object -First 1
    return $null -eq $entry
}

function Get-RelativePath([string]$RootPath, [string]$Path) {
    return $Path.Substring($RootPath.TrimEnd("\").Length).TrimStart("\")
}

function Copy-VerifiedDirectoryTree([string]$SourcePath, [string]$TargetPath) {
    $source = [System.IO.Path]::GetFullPath($SourcePath).TrimEnd("\")
    $target = [System.IO.Path]::GetFullPath($TargetPath).TrimEnd("\")
    Assert-NormalDirectoryTree $source
    if (-not (Test-DirectoryEmpty $target)) {
        throw "The migration target is not empty: $target"
    }
    [void][System.IO.Directory]::CreateDirectory($target)

    $sourceDirectories = @(Get-ChildItem -LiteralPath $source -Force -Directory -Recurse)
    foreach ($sourceDirectory in $sourceDirectories) {
        $relativePath = Get-RelativePath $source $sourceDirectory.FullName
        [void][System.IO.Directory]::CreateDirectory((Join-Path $target $relativePath))
    }
    $sourceFiles = @(Get-ChildItem -LiteralPath $source -Force -File -Recurse)
    foreach ($sourceFile in $sourceFiles) {
        $relativePath = Get-RelativePath $source $sourceFile.FullName
        $targetFile = Join-Path $target $relativePath
        [System.IO.File]::Copy($sourceFile.FullName, $targetFile, $false)
    }

    foreach ($sourceFile in $sourceFiles) {
        $relativePath = Get-RelativePath $source $sourceFile.FullName
        Set-Acl -LiteralPath (Join-Path $target $relativePath) -AclObject (
            Get-Acl -LiteralPath $sourceFile.FullName
        )
    }
    foreach ($sourceDirectory in ($sourceDirectories | Sort-Object { $_.FullName.Length } -Descending)) {
        $relativePath = Get-RelativePath $source $sourceDirectory.FullName
        Set-Acl -LiteralPath (Join-Path $target $relativePath) -AclObject (
            Get-Acl -LiteralPath $sourceDirectory.FullName
        )
    }
    Set-Acl -LiteralPath $target -AclObject (Get-Acl -LiteralPath $source)

    $targetFiles = @(Get-ChildItem -LiteralPath $target -Force -File -Recurse)
    if ($sourceFiles.Count -ne $targetFiles.Count) {
        throw "The migrated file count does not match for: $source"
    }
    $targetFileMap = @{}
    foreach ($targetFile in $targetFiles) {
        $relativePath = (Get-RelativePath $target $targetFile.FullName).ToLowerInvariant()
        $targetFileMap[$relativePath] = $targetFile.FullName
    }
    foreach ($sourceFile in $sourceFiles) {
        $relativePath = (Get-RelativePath $source $sourceFile.FullName).ToLowerInvariant()
        if (-not $targetFileMap.ContainsKey($relativePath)) {
            throw "A migrated file is missing: $relativePath"
        }
        $targetFile = $targetFileMap[$relativePath]
        if ($sourceFile.Length -ne (Get-Item -LiteralPath $targetFile -Force).Length) {
            throw "A migrated file has a different size: $relativePath"
        }
        $sourceHash = (Get-FileHash -LiteralPath $sourceFile.FullName -Algorithm SHA256).Hash
        $targetHash = (Get-FileHash -LiteralPath $targetFile -Algorithm SHA256).Hash
        if ($sourceHash -ne $targetHash) {
            throw "A migrated file has a different hash: $relativePath"
        }
    }
}

function Set-UserApplicationDataItemAccessControl(
    [string]$Path,
    [bool]$Directory,
    [System.Security.Principal.SecurityIdentifier]$User
) {
    if ($Directory) {
        $security = [System.Security.AccessControl.DirectorySecurity]::new()
        $inheritance = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
            [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
    } else {
        $security = [System.Security.AccessControl.FileSecurity]::new()
        $inheritance = [System.Security.AccessControl.InheritanceFlags]::None
    }
    $security.SetOwner($User)
    $security.SetAccessRuleProtection($true, $false)
    $system = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-18")
    $administrators = [System.Security.Principal.SecurityIdentifier]::new("S-1-5-32-544")
    foreach ($identity in @($User, $system, $administrators)) {
        $rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
            $identity,
            [System.Security.AccessControl.FileSystemRights]::FullControl,
            $inheritance,
            [System.Security.AccessControl.PropagationFlags]::None,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
        [void]$security.AddAccessRule($rule)
    }
    if ($Directory) {
        [System.IO.Directory]::SetAccessControl($Path, $security)
    } else {
        [System.IO.File]::SetAccessControl($Path, $security)
    }
}

function Set-UserApplicationDataAccessControl([string]$Path, [string]$UserID) {
    $user = [System.Security.Principal.SecurityIdentifier]::new($UserID)
    $items = @(Get-ChildItem -LiteralPath $Path -Force -Recurse)
    foreach ($item in ($items | Sort-Object { $_.FullName.Length } -Descending)) {
        Set-UserApplicationDataItemAccessControl $item.FullName $item.PSIsContainer $user
    }
    Set-UserApplicationDataItemAccessControl $Path $true $user
}

function Remove-NormalDirectoryTree([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    Assert-NormalDirectoryTree $Path
    [System.IO.Directory]::Delete($Path, $true)
}

function Get-WindowsProfiles() {
    $profiles = @{}
    $profileListPath = "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList"
    foreach ($profileKey in Get-ChildItem -LiteralPath $profileListPath) {
        $profileImagePath = (Get-ItemProperty -LiteralPath $profileKey.PSPath).ProfileImagePath
        if ([string]::IsNullOrWhiteSpace($profileImagePath)) {
            continue
        }
        $profilePath = [Environment]::ExpandEnvironmentVariables($profileImagePath)
        $profiles[$profileKey.PSChildName] = [PSCustomObject]@{
            UserID = $profileKey.PSChildName
            DataPath = Join-Path $profilePath "AppData\Roaming\sing-box"
            LegacyDataPath = Join-Path $profilePath "AppData\Roaming\sing-box-for-desktop"
        }
    }
    return $profiles
}

function Get-ActiveWindowsProfile([hashtable]$Profiles) {
    $computerSystem = Get-CimInstance -ClassName Win32_ComputerSystem
    if ([string]::IsNullOrWhiteSpace($computerSystem.UserName)) {
        return $null
    }
    try {
        $account = [System.Security.Principal.NTAccount]::new($computerSystem.UserName)
        $userID = $account.Translate(
            [System.Security.Principal.SecurityIdentifier]
        ).Value
    } catch {
        return $null
    }
    if (-not $Profiles.ContainsKey($userID)) {
        return $null
    }
    return $Profiles[$userID]
}

function Read-ApplicationDataDirectoryID([string]$Path) {
    $directory = [System.IO.Path]::GetFullPath($Path).TrimEnd("\")
    try {
        return [string](Get-Content `
            -LiteralPath $directory `
            -Stream "sing-box.installation-id" `
            -Raw `
            -ErrorAction Stop)
    } catch [System.IO.FileNotFoundException] {
        return $null
    } catch [System.IO.DirectoryNotFoundException] {
        return $null
    }
}

function Assert-ApplicationDataDirectory([string]$Path, [string]$ExpectedID) {
    if ([string]::IsNullOrWhiteSpace($ExpectedID) -or
        (Read-ApplicationDataDirectoryID $Path) -ne $ExpectedID) {
        throw "The application data directory does not belong to this installation: $Path"
    }
}

function Resolve-DefaultApplicationDataProfile(
    [hashtable]$Profiles,
    [string]$SourcePath
) {
    if (-not [string]::IsNullOrWhiteSpace($SourcePath)) {
        $owner = (Get-Acl -LiteralPath $SourcePath).GetOwner(
            [System.Security.Principal.SecurityIdentifier]
        ).Value
        if ($Profiles.ContainsKey($owner)) {
            return $Profiles[$owner]
        }
    }
    $activeProfile = Get-ActiveWindowsProfile $Profiles
    if ($null -ne $activeProfile -and
        (-not [string]::IsNullOrWhiteSpace($SourcePath) -or
            (Test-Path -LiteralPath $activeProfile.DataPath))) {
        return $activeProfile
    }
    $profilesWithData = @($Profiles.Values | Where-Object {
        Test-Path -LiteralPath $_.DataPath
    })
    if ($profilesWithData.Count -eq 1) {
        return $profilesWithData[0]
    }
    if ($profilesWithData.Count -eq 0) {
        return $null
    }
    throw "The Windows profile for the application data directory is unavailable."
}

function Resolve-LegacyApplicationDataProfile([hashtable]$Profiles) {
    $activeProfile = Get-ActiveWindowsProfile $Profiles
    if ($null -ne $activeProfile) {
        if ((Test-Path -LiteralPath $activeProfile.DataPath) -or
            -not (Test-Path -LiteralPath $activeProfile.LegacyDataPath)) {
            return $null
        }
        return $activeProfile
    }
    $profilesWithLegacyData = @($Profiles.Values | Where-Object {
        -not (Test-Path -LiteralPath $_.DataPath) -and
            (Test-Path -LiteralPath $_.LegacyDataPath)
    })
    if ($profilesWithLegacyData.Count -eq 1) {
        return $profilesWithLegacyData[0]
    }
    if ($profilesWithLegacyData.Count -eq 0) {
        return $null
    }
    throw "The Windows profile for the legacy application data directory is unavailable."
}

function Get-ApplicationDataCopy(
    [string]$PreviousDirectory,
    [string]$NewDirectory,
    [string]$PreviousID,
    [bool]$MigrateLegacyData
) {
    $profiles = Get-WindowsProfiles
    $profile = $null
    if ($MigrateLegacyData) {
        $profile = Resolve-LegacyApplicationDataProfile $profiles
        if ($null -eq $profile) {
            return $null
        }
        $source = $profile.LegacyDataPath
    } elseif ([string]::IsNullOrWhiteSpace($PreviousDirectory)) {
        $profile = Resolve-DefaultApplicationDataProfile $profiles ""
        if ($null -eq $profile) {
            return $null
        }
        $source = $profile.DataPath
        if (-not (Test-Path -LiteralPath $source)) {
            return $null
        }
    } else {
        Assert-ApplicationDataDirectory $PreviousDirectory $PreviousID
        $source = $PreviousDirectory
    }
    if ([string]::IsNullOrWhiteSpace($NewDirectory)) {
        if ($null -eq $profile) {
            $profile = Resolve-DefaultApplicationDataProfile $profiles $source
        }
        $target = $profile.DataPath
        $targetUserID = $profile.UserID
    } else {
        $target = $NewDirectory
        $targetUserID = ""
    }
    return [PSCustomObject]@{
        Source = $source
        Target = $target
        TargetUserID = $targetUserID
    }
}

function Write-TransitionState([object]$State) {
    $temporaryPath = "$StatePath.new"
    $backupPath = "$StatePath.old"
    [System.IO.File]::WriteAllText(
        $temporaryPath,
        ($State | ConvertTo-Json -Depth 8),
        [System.Text.UTF8Encoding]::new($false)
    )
    if ([System.IO.File]::Exists($StatePath)) {
        [System.IO.File]::Delete($backupPath)
        [System.IO.File]::Replace($temporaryPath, $StatePath, $backupPath)
        [System.IO.File]::Delete($backupPath)
    } else {
        [System.IO.File]::Move($temporaryPath, $StatePath)
    }
}

function Read-TransitionState() {
    if (-not [System.IO.File]::Exists($StatePath)) {
        return $null
    }
    return Get-Content -LiteralPath $StatePath -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Remove-TransitionState() {
    Remove-Item -LiteralPath $StatePath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$StatePath.new" -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$StatePath.old" -Force -ErrorAction SilentlyContinue
    $stateDirectory = Split-Path -Parent $StatePath
    if (Test-DirectoryEmpty $stateDirectory) {
        [System.IO.Directory]::Delete($stateDirectory)
    }
}

function Complete-Transition([object]$State) {
    if ($State.Phase -ne "Committed") {
        $State.Phase = "Committed"
        Write-TransitionState $State
    }
    if ($State.ApplicationChanged) {
        foreach ($copy in @($State.ApplicationCopies)) {
            if (-not [string]::IsNullOrWhiteSpace($State.PreviousApplicationDataDirectory)) {
                Assert-ApplicationDataDirectory `
                    $copy.Source `
                    $State.PreviousInstallationID
            }
            Remove-NormalDirectoryTree $copy.Source
        }
    }
    if ($State.DaemonChanged) {
        Remove-NormalDirectoryTree $State.PreviousDaemonDataDirectory
    }
    Remove-TransitionState
}

function Undo-Transition([object]$State) {
    if ($State.Phase -eq "Committed") {
        throw "A committed data migration cannot be rolled back."
    }
    if ($State.ApplicationChanged) {
        foreach ($copy in @($State.ApplicationCopies)) {
            if (-not [string]::IsNullOrWhiteSpace($State.ApplicationDataDirectory)) {
                Assert-ApplicationDataDirectory `
                    $copy.Target `
                    $State.InstallationID
            }
            Remove-NormalDirectoryTree $copy.Target
        }
    }
    if ($State.DaemonChanged) {
        Remove-NormalDirectoryTree $State.DaemonDataDirectory
    }
    Remove-TransitionState
}

try {
    Initialize-TransitionStateStorage
    if ($Operation -eq "Recover") {
        $state = Read-TransitionState
        if ($null -eq $state) {
            exit 0
        }
        if ($state.Phase -eq "Committed") {
            Complete-Transition $state
        } else {
            Undo-Transition $state
        }
        exit 0
    }
    if ($Operation -eq "Commit") {
        $state = Read-TransitionState
        if ($null -ne $state) {
            Complete-Transition $state
        }
        exit 0
    }
    if ($Operation -eq "Rollback") {
        $state = Read-TransitionState
        if ($null -ne $state) {
            Undo-Transition $state
        }
        exit 0
    }

    if ([System.IO.File]::Exists($StatePath)) {
        throw "An unfinished data migration already exists."
    }
    $applicationChanged = (
        $MigrateLegacyApplicationData -or
        -not (Test-SamePath `
            $PreviousApplicationDataDirectory `
            $ApplicationDataDirectory)
    )
    $daemonChanged = -not (Test-SamePath `
        $PreviousDaemonDataDirectory `
        $DaemonDataDirectory)
    if ($applicationChanged -and
        -not [string]::IsNullOrWhiteSpace($PreviousApplicationDataDirectory) -and
        -not [string]::IsNullOrWhiteSpace($ApplicationDataDirectory) -and
        (Test-PathOverlap $PreviousApplicationDataDirectory $ApplicationDataDirectory)) {
        throw "The old and new application data directories overlap."
    }
    if ($daemonChanged -and
        (Test-PathOverlap $PreviousDaemonDataDirectory $DaemonDataDirectory)) {
        throw "The old and new daemon data directories overlap."
    }

    $state = [PSCustomObject]@{
        Phase = "Preparing"
        PreviousApplicationDataDirectory = $PreviousApplicationDataDirectory
        ApplicationDataDirectory = $ApplicationDataDirectory
        PreviousDaemonDataDirectory = $PreviousDaemonDataDirectory
        DaemonDataDirectory = $DaemonDataDirectory
        PreviousInstallationID = $PreviousInstallationID
        InstallationID = $InstallationID
        ApplicationChanged = $applicationChanged
        DaemonChanged = $daemonChanged
        ApplicationCopies = @()
    }
    Write-TransitionState $state

    if ($applicationChanged) {
        $copy = Get-ApplicationDataCopy `
            $PreviousApplicationDataDirectory `
            $ApplicationDataDirectory `
            $PreviousInstallationID `
            $MigrateLegacyApplicationData
        if ($null -ne $copy -and -not (Test-SamePath $copy.Source $copy.Target)) {
            $state.ApplicationCopies = @($copy)
        }
        Write-TransitionState $state
        foreach ($copy in $state.ApplicationCopies) {
            Copy-VerifiedDirectoryTree $copy.Source $copy.Target
            if (-not [string]::IsNullOrWhiteSpace($copy.TargetUserID)) {
                Set-UserApplicationDataAccessControl $copy.Target $copy.TargetUserID
            }
        }
    }
    if ($daemonChanged -and (Test-Path -LiteralPath $PreviousDaemonDataDirectory)) {
        Copy-VerifiedDirectoryTree $PreviousDaemonDataDirectory $DaemonDataDirectory
    }
    $state.Phase = "Prepared"
    Write-TransitionState $state
    exit 0
} catch {
    [Console]::Error.WriteLine(
        "$($_.Exception.Message)$([Environment]::NewLine)$($_.ScriptStackTrace)"
    )
    exit 40
}
