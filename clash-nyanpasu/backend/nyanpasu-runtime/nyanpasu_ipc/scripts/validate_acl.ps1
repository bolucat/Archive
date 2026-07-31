# validate_sddl.ps1
param(
    [Parameter(Mandatory = $true)]
    [string]$SddlString
)

try {
    # 尝试将 SDDL 转换为安全描述符对象
    $sd = ConvertFrom-SddlString -Sddl $SddlString
    
    Write-Host "SDDL 验证成功!" -ForegroundColor Green
    Write-Host "`n所有者 (Owner):" -ForegroundColor Yellow
    Write-Host $sd.Owner
    
    Write-Host "`n组 (Group):" -ForegroundColor Yellow
    Write-Host $sd.Group
    
    Write-Host "`n自由访问控制列表 (DACL):" -ForegroundColor Yellow
    $sd.DiscretionaryAcl | ForEach-Object {
        # Write-Host "  - 标识: $($_.IdentityReference)"
        # Write-Host "    权限: $($_.FileSystemRights)"
        # Write-Host "    类型: $($_.AccessControlType)"
        # Write-Host "    继承: $($_.IsInherited)"
        Write-Host ($obj | Format-List | Out-String)
        Write-Host ""
    }
    Write-Host "共 $($sd.DiscretionaryAcl.Count) 个 DACL 项"
    Write-Host ""

    Write-Host "`n系统访问控制列表 (SACL):" -ForegroundColor Yellow
    $sd.SystemAcl | ForEach-Object {
        Write-Host ($obj | Format-Table | Out-String)
        Write-Host ""
    }


    Write-Host "共 $($sd.SystemAcl.Count) 个 SACL 项"
    Write-Host ""



    Write-Host "`n安全描述符:" -ForegroundColor Yellow
    Write-Host ($sd | Format-List | Out-String)

    
    # 将 SDDL 应用到临时文件以进一步验证
    $tempFile = [System.IO.Path]::GetTempFileName()
    $acl = Get-Acl $tempFile
    $acl.SetSecurityDescriptorSddlForm($SddlString)
    Set-Acl -Path $tempFile -AclObject $acl
    
    Write-Host "SDDL 成功应用到文件!" -ForegroundColor Green
    
    # 清理
    Remove-Item $tempFile -Force
    
    exit 0
}
catch {
    Write-Host "SDDL 验证失败!" -ForegroundColor Red
    Write-Host "错误: $_" -ForegroundColor Red
    exit 1
}
