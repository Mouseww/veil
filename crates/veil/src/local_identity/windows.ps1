$ErrorActionPreference='SilentlyContinue'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$r=[ordered]@{}
if($collectContext) {
    $r.host=[Environment]::MachineName
    $r.user=[Environment]::UserName
}
if($collectHardware) {
    $r.sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $r.smbios=(Get-CimInstance Win32_ComputerSystemProduct).UUID
    $r.machine_guid=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Cryptography').MachineGuid
    $r.sqm=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\SQMClient').MachineId
    $r.product=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').ProductId
    $r.disks=@(Get-CimInstance Win32_DiskDrive | ForEach-Object SerialNumber) + @(Get-PhysicalDisk | ForEach-Object SerialNumber)
    $r.volumes=@(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3' | ForEach-Object VolumeSerialNumber)
    $r.macs=@(Get-NetAdapter -Physical | ForEach-Object MacAddress)
    $r.serials=@((Get-CimInstance Win32_BIOS).SerialNumber) + @((Get-CimInstance Win32_BaseBoard).SerialNumber) + @((Get-CimInstance Win32_SystemEnclosure).SerialNumber)
}
ConvertTo-Json -InputObject $r -Compress -Depth 4
