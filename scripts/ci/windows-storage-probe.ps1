# Run on Windows before accepting an EPRFS DirectoryStorage repair.
# This measures API behavior, not resistance to an actual power loss.
# Example: pwsh -File windows-storage-probe.ps1 -ReportPath probe-result.json
param([Parameter(Mandatory = $true)][string] $ReportPath)
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This probe requires Windows; emulation is not platform evidence.'
}

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
public static class StorageFlushProbe {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern SafeFileHandle CreateFileW(string path, uint access,
        uint sharing, IntPtr security, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool FlushFileBuffers(SafeFileHandle handle);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetVolumeInformationByHandleW(SafeFileHandle handle,
        IntPtr volumeName, uint volumeNameSize, IntPtr serial, IntPtr maximumName,
        IntPtr flags, StringBuilder filesystem, uint filesystemSize);
}
'@

$read = [uint32]2147483648
$readWrite = [uint32]3221225472
$backup = [uint32]33554432
$rows = [Collections.Generic.List[object]]::new()
$root = Join-Path ([IO.Path]::GetTempPath()) ('eprfs-flush-probe-' + [Guid]::NewGuid().ToString('N'))

function Measure-Flush([string] $Name, [string] $Path, [uint32] $Access, [uint32] $Flags) {
    # OPEN_EXISTING + read/write/delete sharing; never creates or overwrites a path.
    $handle = [StorageFlushProbe]::CreateFileW($Path, $Access, 7, [IntPtr]::Zero,
        3, $Flags, [IntPtr]::Zero)
    $openError = if ($handle.IsInvalid) { [Runtime.InteropServices.Marshal]::GetLastWin32Error() } else { 0 }
    $row = [ordered]@{
        name = $Name; access = ('0x{0:X8}' -f $Access); flags = ('0x{0:X8}' -f $Flags)
        opened = (-not $handle.IsInvalid); openError = $openError
        filesystem = $null; volumeQueryError = $null; flushed = $false; flushError = $null
    }
    try {
        if (-not $handle.IsInvalid) {
            $fs = [Text.StringBuilder]::new(261)
            $queried = [StorageFlushProbe]::GetVolumeInformationByHandleW($handle,
                [IntPtr]::Zero, 0, [IntPtr]::Zero, [IntPtr]::Zero, [IntPtr]::Zero, $fs, 261)
            $row.volumeQueryError = if ($queried) { 0 } else { [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
            if ($queried) { $row.filesystem = $fs.ToString() }
            $row.flushed = [StorageFlushProbe]::FlushFileBuffers($handle)
            $row.flushError = if ($row.flushed) { 0 } else { [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
        }
    } finally {
        $handle.Dispose()
    }
    $rows.Add([pscustomobject]$row)
}

$failure = $null
try {
    [void][IO.Directory]::CreateDirectory($root)
    $store = Join-Path $root 'store'
    [void][IO.Directory]::CreateDirectory($store)
    $object = Join-Path $store 'object'
    [IO.File]::WriteAllBytes($object, [Text.Encoding]::UTF8.GetBytes('verified object bytes'))

    Measure-Flush 'object-read-only' $object $read 0
    Measure-Flush 'object-read-write' $object $readWrite 0
    Measure-Flush 'existing-object-read-write' $object $readWrite 0
    Measure-Flush 'directory-without-backup' $store $read 0
    Measure-Flush 'directory-backup-read-only' $store $read $backup
    Measure-Flush 'directory-backup-read-write' $store $readWrite $backup
    # Secondary comparison only: FILE_LIST_DIRECTORY | FILE_ADD_FILE | FILE_READ_ATTRIBUTES.
    Measure-Flush 'directory-backup-add-file' $store 131 $backup
    Measure-Flush 'parent-backup-read-write' $root $readWrite $backup

    # Exercise namespace publication followed by object, directory, and parent flushes.
    $temporary = Join-Path $store 'candidate'
    [IO.File]::WriteAllBytes($temporary, [Text.Encoding]::UTF8.GetBytes('published object bytes'))
    Measure-Flush 'candidate-before-publication' $temporary $readWrite 0
    $published = Join-Path $store 'published'
    [IO.File]::Move($temporary, $published)
    Measure-Flush 'published-object' $published $readWrite 0
    Measure-Flush 'directory-after-publication' $store $readWrite $backup
    Measure-Flush 'parent-after-publication' $root $readWrite $backup
    if ([IO.File]::ReadAllText($published) -ne 'published object bytes') {
        throw 'Published content changed.'
    }
} catch {
    $failure = $_.Exception.ToString()
} finally {
    # Retain the unique fixture for inspection. The path is reported below.
    $required = @('object-read-write', 'existing-object-read-write', 'directory-backup-read-write', 'parent-backup-read-write',
        'candidate-before-publication', 'published-object', 'directory-after-publication', 'parent-after-publication')
    $requiredRows = @($rows | Where-Object { $_.name -in $required })
    $apiBridgePassed = ($null -eq $failure -and $requiredRows.Count -eq $required.Count -and
        @($requiredRows | Where-Object { -not $_.flushed -or $_.filesystem -ne 'NTFS' }).Count -eq 0)
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $report = [ordered]@{
        schema = 'eprfs-windows-flush-probe/v1'
        utc = [DateTime]::UtcNow.ToString('o')
        os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
        processArchitecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
        elevatedAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        privileges = @(& whoami.exe /priv)
        fixture = $root
        apiBridgePassed = $apiBridgePassed
        physicalPowerLossTested = $false
        productionRustTestsRun = $false
        error = $failure
        observations = @($rows.ToArray())
    }
    $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ReportPath -Encoding utf8
    $report | ConvertTo-Json -Depth 6
}
if (-not $apiBridgePassed) { exit 1 }
