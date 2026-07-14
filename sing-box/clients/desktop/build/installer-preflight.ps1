param(
    [Parameter(Mandatory = $true)][string]$InstallationDirectory,
    [switch]$AllowUnsafeInstallationDirectory,
    [switch]$RepairInstallationAncestors,
    [switch]$ResetWorkingDirectory
)

$ErrorActionPreference = "Stop"
$reparsePoint = [System.IO.FileAttributes]::ReparsePoint
$trustedInstallationIdentities = @(
    "S-1-5-18",
    "S-1-5-32-544",
    "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"
)
$dangerousInstallationAccess = [uint32]0x500D0040

Add-Type -TypeDefinition @'
using Microsoft.Win32.SafeHandles;
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace Box.Installer
{
    public static class DirectoryAccessControl
    {
        private const uint MaximumAllowed = 0x02000000;
        private const uint ShareRead = 0x00000001;
        private const uint ShareWrite = 0x00000002;
        private const uint ShareDelete = 0x00000004;
        private const uint OpenExisting = 3;
        private const uint OpenReparsePoint = 0x00200000;
        private const uint BackupSemantics = 0x02000000;
        private const int FileObject = 1;
        private const uint DiscretionaryAccessControlListSecurityInformation = 0x00000004;

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFile(
            string fileName,
            uint desiredAccess,
            uint shareMode,
            IntPtr securityAttributes,
            uint creationDisposition,
            uint flagsAndAttributes,
            IntPtr templateFile
        );

        [DllImport("advapi32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetSecurityDescriptorDacl(
            IntPtr securityDescriptor,
            [MarshalAs(UnmanagedType.Bool)] out bool discretionaryAccessControlListPresent,
            out IntPtr discretionaryAccessControlList,
            [MarshalAs(UnmanagedType.Bool)] out bool discretionaryAccessControlListDefaulted
        );

        [DllImport("advapi32.dll")]
        private static extern uint SetSecurityInfo(
            SafeFileHandle handle,
            int objectType,
            uint securityInformation,
            IntPtr owner,
            IntPtr group,
            IntPtr discretionaryAccessControlList,
            IntPtr systemAccessControlList
        );

        public static void SetCurrentDirectoryAccessControl(string path, byte[] securityDescriptor)
        {
            using (SafeFileHandle handle = CreateFile(
                path,
                MaximumAllowed,
                ShareRead | ShareWrite | ShareDelete,
                IntPtr.Zero,
                OpenExisting,
                OpenReparsePoint | BackupSemantics,
                IntPtr.Zero
            ))
            {
                if (handle.IsInvalid)
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }
                IntPtr descriptor = Marshal.AllocHGlobal(securityDescriptor.Length);
                try
                {
                    Marshal.Copy(securityDescriptor, 0, descriptor, securityDescriptor.Length);
                    bool accessControlListPresent;
                    bool accessControlListDefaulted;
                    IntPtr accessControlList;
                    if (!GetSecurityDescriptorDacl(
                        descriptor,
                        out accessControlListPresent,
                        out accessControlList,
                        out accessControlListDefaulted
                    ))
                    {
                        throw new Win32Exception(Marshal.GetLastWin32Error());
                    }
                    if (!accessControlListPresent || accessControlList == IntPtr.Zero)
                    {
                        throw new InvalidOperationException("The directory has an empty access control list.");
                    }
                    uint result = SetSecurityInfo(
                        handle,
                        FileObject,
                        DiscretionaryAccessControlListSecurityInformation,
                        IntPtr.Zero,
                        IntPtr.Zero,
                        accessControlList,
                        IntPtr.Zero
                    );
                    if (result != 0)
                    {
                        throw new Win32Exception((int)result);
                    }
                }
                finally
                {
                    Marshal.FreeHGlobal(descriptor);
                }
            }
        }
    }

    public static class InstallationVolume
    {
        public const uint FixedDrive = 3;

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetVolumePathName(
            string fileName,
            StringBuilder volumePathName,
            int bufferLength
        );

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        private static extern uint GetDriveType(string rootPathName);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern uint QueryDosDevice(
            string deviceName,
            StringBuilder targetPath,
            int maximumLength
        );

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetVolumeInformation(
            string rootPathName,
            StringBuilder volumeNameBuffer,
            int volumeNameSize,
            out uint volumeSerialNumber,
            out uint maximumComponentLength,
            out uint fileSystemFlags,
            StringBuilder fileSystemNameBuffer,
            int fileSystemNameSize
        );

        public static string ResolveVolumePath(string path)
        {
            StringBuilder volumePath = new StringBuilder(32768);
            if (!GetVolumePathName(path, volumePath, volumePath.Capacity))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
            string resolvedVolumePath = volumePath.ToString();
            if (ResolveSubstitutedVolumePath(resolvedVolumePath) != null)
            {
                return Path.GetPathRoot(resolvedVolumePath);
            }
            return resolvedVolumePath;
        }

        public static uint ResolveDriveType(string volumePath)
        {
            return GetDriveType(volumePath);
        }

        public static string ResolveFileSystemName(string volumePath)
        {
            StringBuilder fileSystemName = new StringBuilder(32);
            uint volumeSerialNumber;
            uint maximumComponentLength;
            uint fileSystemFlags;
            bool resolved = GetVolumeInformation(
                volumePath,
                null,
                0,
                out volumeSerialNumber,
                out maximumComponentLength,
                out fileSystemFlags,
                fileSystemName,
                fileSystemName.Capacity
            );
            if (!resolved)
            {
                string resolvedVolumePath = ResolveSubstitutedVolumePath(volumePath);
                if (resolvedVolumePath == null)
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }
                resolved = GetVolumeInformation(
                    resolvedVolumePath,
                    null,
                    0,
                    out volumeSerialNumber,
                    out maximumComponentLength,
                    out fileSystemFlags,
                    fileSystemName,
                    fileSystemName.Capacity
                );
                if (!resolved)
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }
            }
            return fileSystemName.ToString();
        }

        private static string ResolveSubstitutedVolumePath(string volumePath)
        {
            string rootPath = Path.GetPathRoot(volumePath);
            if (String.IsNullOrEmpty(rootPath))
            {
                return null;
            }
            string deviceName = rootPath.TrimEnd('\\');
            if (deviceName.Length != 2 || deviceName[1] != ':')
            {
                return null;
            }
            StringBuilder targetPath = new StringBuilder(32768);
            if (QueryDosDevice(deviceName, targetPath, targetPath.Capacity) == 0)
            {
                return null;
            }
            string resolvedPath = targetPath.ToString();
            if (!resolvedPath.StartsWith("\\??\\", StringComparison.OrdinalIgnoreCase))
            {
                return null;
            }
            string resolvedRootPath = Path.GetPathRoot(resolvedPath.Substring(4));
            if (String.IsNullOrEmpty(resolvedRootPath))
            {
                return null;
            }
            return resolvedRootPath;
        }
    }
}
'@

function Get-ExistingInstallationPath {
    $currentPath = [System.IO.Path]::GetFullPath($InstallationDirectory)
    while (-not (Test-Path -LiteralPath $currentPath)) {
        $parentPath = [System.IO.Directory]::GetParent($currentPath)
        if ($null -eq $parentPath) {
            throw "The installation directory has no existing ancestor."
        }
        $currentPath = $parentPath.FullName
    }
    return $currentPath
}

function Get-InstallationAncestorPaths([string]$VolumeRoot) {
    $paths = [System.Collections.Generic.List[string]]::new()
    $currentPath = [System.IO.Directory]::GetParent(
        [System.IO.Path]::GetFullPath($InstallationDirectory)
    ).FullName
    while (-not (Test-Path -LiteralPath $currentPath)) {
        $parentPath = [System.IO.Directory]::GetParent($currentPath)
        if ($null -eq $parentPath) {
            throw "The installation directory is outside its resolved volume."
        }
        $currentPath = $parentPath.FullName
    }
    $normalizedVolumeRoot = $VolumeRoot.TrimEnd("\")
    while ($true) {
        [void]$paths.Add($currentPath)
        if ([string]::Equals(
                $currentPath.TrimEnd("\"),
                $normalizedVolumeRoot,
                [System.StringComparison]::OrdinalIgnoreCase
            )) {
            break
        }
        $parentPath = [System.IO.Directory]::GetParent($currentPath)
        if ($null -eq $parentPath -or $parentPath.FullName -eq $currentPath) {
            throw "The installation directory is outside its resolved volume."
        }
        $currentPath = $parentPath.FullName
    }
    return $paths
}

function Get-RawDirectorySecurity([string]$Path) {
    $security = Get-Acl -LiteralPath $Path
    $binary = $security.GetSecurityDescriptorBinaryForm()
    return [System.Security.AccessControl.RawSecurityDescriptor]::new($binary, 0)
}

function Get-ServiceSid([string]$ServiceName) {
    $serviceNameBytes = [System.Text.Encoding]::Unicode.GetBytes($ServiceName.ToUpperInvariant())
    $hashAlgorithm = [System.Security.Cryptography.SHA1]::Create()
    try {
        $serviceNameHash = $hashAlgorithm.ComputeHash($serviceNameBytes)
    } finally {
        $hashAlgorithm.Dispose()
    }
    $subAuthorities = [System.Collections.Generic.List[uint32]]::new()
    for ($offset = 0; $offset -lt $serviceNameHash.Length; $offset += 4) {
        [void]$subAuthorities.Add([System.BitConverter]::ToUInt32($serviceNameHash, $offset))
    }
    return "S-1-5-80-$($subAuthorities -join '-')"
}

function Get-UnsafeInstallationAncestor([string[]]$Paths) {
    $repairableAncestor = $null
    foreach ($path in $Paths) {
        $item = Get-Item -LiteralPath $path -Force
        if (-not $item.PSIsContainer -or ($item.Attributes -band $reparsePoint)) {
            return [PSCustomObject]@{ Path = $path; ExitCode = 11 }
        }
        $security = Get-RawDirectorySecurity $path
        if ($null -eq $security.Owner -or $security.Owner.Value -notin $trustedInstallationIdentities) {
            return [PSCustomObject]@{ Path = $path; ExitCode = 12 }
        }
        if ($null -eq $security.DiscretionaryAcl) {
            return [PSCustomObject]@{ Path = $path; ExitCode = 12 }
        }
        foreach ($entry in $security.DiscretionaryAcl) {
            if ([int]$entry.AceType -ne [int][System.Security.AccessControl.AceType]::AccessAllowed -or
                (([int]$entry.AceFlags -band [int][System.Security.AccessControl.AceFlags]::InheritOnly) -ne 0) -or
                $entry.SecurityIdentifier.Value -in $trustedInstallationIdentities) {
                continue
            }
            $accessMask = [System.BitConverter]::ToUInt32(
                [System.BitConverter]::GetBytes([int]$entry.AccessMask),
                0
            )
            if (($accessMask -band $dangerousInstallationAccess) -ne 0) {
                if ($null -eq $repairableAncestor) {
                    $repairableAncestor = [PSCustomObject]@{ Path = $path; ExitCode = 13 }
                }
            }
        }
    }
    return $repairableAncestor
}

function Repair-InstallationAncestor([string]$Path) {
    $accessControl = Get-Acl -LiteralPath $Path
    $binary = $accessControl.GetSecurityDescriptorBinaryForm()
    $security = [System.Security.AccessControl.RawSecurityDescriptor]::new($binary, 0)
    $changed = $false
    for ($index = $security.DiscretionaryAcl.Count - 1; $index -ge 0; $index--) {
        $entry = $security.DiscretionaryAcl[$index]
        if ([int]$entry.AceType -ne [int][System.Security.AccessControl.AceType]::AccessAllowed -or
            (([int]$entry.AceFlags -band [int][System.Security.AccessControl.AceFlags]::InheritOnly) -ne 0) -or
            (([int]$entry.AceFlags -band [int][System.Security.AccessControl.AceFlags]::Inherited) -ne 0) -or
            $entry.SecurityIdentifier.Value -in $trustedInstallationIdentities) {
            continue
        }
        $accessMask = [System.BitConverter]::ToUInt32(
            [System.BitConverter]::GetBytes([int]$entry.AccessMask),
            0
        )
        if (($accessMask -band $dangerousInstallationAccess) -eq 0) {
            continue
        }
        $safeAccessMask = $accessMask -band (-bnot $dangerousInstallationAccess)
        if ($safeAccessMask -eq 0) {
            $security.DiscretionaryAcl.RemoveAce($index)
        } else {
            $entry.AccessMask = [System.BitConverter]::ToInt32(
                [System.BitConverter]::GetBytes([uint32]$safeAccessMask),
                0
            )
        }
        $changed = $true
    }
    if (-not $changed) {
        return
    }
    $updatedBinary = [byte[]]::new($security.BinaryLength)
    $security.GetBinaryForm($updatedBinary, 0)
    $accessControl.SetSecurityDescriptorBinaryForm(
        $updatedBinary,
        [System.Security.AccessControl.AccessControlSections]::Access
    )
    [Box.Installer.DirectoryAccessControl]::SetCurrentDirectoryAccessControl(
        $Path,
        $updatedBinary
    )
}

function Remove-WorkingDirectoryTree([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.Attributes -band $reparsePoint) {
        if ($item.PSIsContainer) {
            [System.IO.Directory]::Delete($item.FullName)
        } else {
            [System.IO.File]::Delete($item.FullName)
        }
        return
    }
    if (-not $item.PSIsContainer) {
        [System.IO.File]::Delete($item.FullName)
        return
    }
    foreach ($child in Get-ChildItem -LiteralPath $item.FullName -Force) {
        Remove-WorkingDirectoryTree $child.FullName
    }
    [System.IO.Directory]::Delete($item.FullName)
}

if ($MyInvocation.InvocationName -eq ".") {
    return
}

try {
    if (-not $AllowUnsafeInstallationDirectory) {
        try {
            $existingInstallationPath = Get-ExistingInstallationPath
            $installationVolume = [Box.Installer.InstallationVolume]::ResolveVolumePath(
                $existingInstallationPath
            )
            $installationDriveType = [Box.Installer.InstallationVolume]::ResolveDriveType(
                $installationVolume
            )
            if ($installationDriveType -ne [Box.Installer.InstallationVolume]::FixedDrive) {
                Write-Output $installationVolume
                exit 14
            }
            $installationFileSystem = [Box.Installer.InstallationVolume]::ResolveFileSystemName(
                $installationVolume
            )
            if (-not [string]::Equals(
                    $installationFileSystem,
                    "NTFS",
                    [System.StringComparison]::OrdinalIgnoreCase
                )) {
                Write-Output $installationFileSystem
                exit 15
            }
        } catch {
            Write-Output "$installationVolume`: $($_.Exception.Message)"
            exit 16
        }

        if (Test-Path -LiteralPath $InstallationDirectory) {
            $installationItem = Get-Item -LiteralPath $InstallationDirectory -Force
            if (-not $installationItem.PSIsContainer -or ($installationItem.Attributes -band $reparsePoint)) {
                exit 10
            }
            $installationReparsePoint = Get-ChildItem -LiteralPath $InstallationDirectory -Force -Recurse |
                Where-Object { $_.Attributes -band $reparsePoint } |
                Select-Object -First 1
            if ($null -ne $installationReparsePoint) {
                exit 10
            }
        }

        $installationAncestorPaths = @(Get-InstallationAncestorPaths $installationVolume)
        $unsafeInstallationAncestor = Get-UnsafeInstallationAncestor $installationAncestorPaths
        if ($RepairInstallationAncestors) {
            [array]::Reverse($installationAncestorPaths)
            foreach ($installationAncestorPath in $installationAncestorPaths) {
                Repair-InstallationAncestor $installationAncestorPath
            }
            $unsafeInstallationAncestor = Get-UnsafeInstallationAncestor $installationAncestorPaths
            if ($null -ne $unsafeInstallationAncestor) {
                Write-Output $unsafeInstallationAncestor.Path
                exit 32
            }
        }
        if ($null -ne $unsafeInstallationAncestor) {
            Write-Output $unsafeInstallationAncestor.Path
            exit ([int]$unsafeInstallationAncestor.ExitCode)
        }
    }

    $commonApplicationData = [Environment]::GetFolderPath(
        [Environment+SpecialFolder]::CommonApplicationData
    )
    $workingDirectory = Join-Path $commonApplicationData "sing-box-daemon"
    if ($ResetWorkingDirectory) {
        Remove-WorkingDirectoryTree $workingDirectory
        if (Test-Path -LiteralPath $workingDirectory) {
            exit 31
        }
        exit 0
    }
    if (-not (Test-Path -LiteralPath $workingDirectory)) {
        exit 0
    }

    $workingItem = Get-Item -LiteralPath $workingDirectory -Force
    if (-not $workingItem.PSIsContainer -or ($workingItem.Attributes -band $reparsePoint)) {
        exit 20
    }
    $workingReparsePoint = Get-ChildItem -LiteralPath $workingDirectory -Force -Recurse |
        Where-Object { $_.Attributes -band $reparsePoint } |
        Select-Object -First 1
    if ($null -ne $workingReparsePoint) {
        exit 20
    }

    $expectedIdentities = @(
        "S-1-5-18",
        "S-1-5-32-544",
        (Get-ServiceSid "sing-box-daemon")
    )
    $accessControl = Get-Acl -LiteralPath $workingDirectory
    $owner = $accessControl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
    if ($owner -ne "S-1-5-18") {
        exit 21
    }
    if (-not $accessControl.AreAccessRulesProtected) {
        exit 22
    }

    $rules = @($accessControl.GetAccessRules(
        $true,
        $false,
        [System.Security.Principal.SecurityIdentifier]
    ))
    if ($rules.Count -ne $expectedIdentities.Count) {
        exit 22
    }
    foreach ($rule in $rules) {
        if ($rule.IdentityReference.Value -notin $expectedIdentities -or
            $rule.AccessControlType -ne [System.Security.AccessControl.AccessControlType]::Allow -or
            [int]$rule.FileSystemRights -ne [int][System.Security.AccessControl.FileSystemRights]::FullControl -or
            $rule.InheritanceFlags -ne (
                [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor
                [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
            ) -or
            $rule.PropagationFlags -ne [System.Security.AccessControl.PropagationFlags]::None) {
            exit 22
        }
    }
    exit 0
} catch {
    [Console]::Error.WriteLine(
        "$($_.Exception.Message)$([Environment]::NewLine)$($_.ScriptStackTrace)"
    )
    exit 30
}
