param(
    [string]$OutputRoot = 'Z:\quoteNetzipRs\.tmp\auth7100-forensics-20260806-real1522',
    [string]$Executable = 'D:\Soft\_Stock\飞狐2020\网际风.exe',
    [string]$WorkingDirectory = 'D:\Soft\_Stock\飞狐2020',
    [string]$ConfigPath = 'D:\Soft\_Stock\飞狐2020\用户\配置文件.ini',
    [string]$CredentialTarget = 'quoteNetzipWine/tdx-account-1522'
)

$ErrorActionPreference = 'Stop'
$account = '1522'
$ports = 7100, 6100, 5188, 7709, 7708, 7719, 14017
$etl = Join-Path $OutputRoot 'login_raw.etl'
$pcap = Join-Path $OutputRoot 'login_raw.pcapng'
$timeline = Join-Path $OutputRoot 'process_timeline.jsonl'
$originalConfigBytes = [IO.File]::ReadAllBytes($ConfigPath)
$originalConfigHash = (Get-FileHash -LiteralPath $ConfigPath -Algorithm SHA256).Hash
$credentialPointer = [IntPtr]::Zero
$credentialBlob = $null
$password = $null
$vendorProcess = $null

$credentialSource = @'
using System;
using System.Runtime.InteropServices;
public static class NativeCredentialReader7100 {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct CREDENTIAL {
        public UInt32 Flags;
        public UInt32 Type;
        public IntPtr TargetName;
        public IntPtr Comment;
        public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
        public UInt32 CredentialBlobSize;
        public IntPtr CredentialBlob;
        public UInt32 Persist;
        public UInt32 AttributeCount;
        public IntPtr Attributes;
        public IntPtr TargetAlias;
        public IntPtr UserName;
    }

    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool CredRead(string target, UInt32 type, UInt32 flags, out IntPtr credential);

    [DllImport("advapi32.dll", EntryPoint = "CredFree")]
    public static extern void CredFree(IntPtr credential);
}
'@

Add-Type -TypeDefinition $credentialSource -ErrorAction Stop

function Add-TimelineEvent {
    param([object]$Event)

    ($Event | ConvertTo-Json -Compress -Depth 6) |
        Add-Content -LiteralPath $timeline -Encoding UTF8
}

function Stop-TestProcesses {
    if ($null -eq $vendorProcess) {
        return
    }

    $children = @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $_.ParentProcessId -eq $vendorProcess.Id -and
                $_.ExecutablePath -eq 'D:\Soft\_Stock\飞狐2020\飞狐交易师\FoxTrader.exe'
            }
    )
    foreach ($child in $children) {
        Stop-Process -Id $child.ProcessId -Force -ErrorAction SilentlyContinue
    }
    Stop-Process -Id $vendorProcess.Id -Force -ErrorAction SilentlyContinue
}

$null = New-Item -ItemType Directory -Path $OutputRoot -Force

try {
    $read = [NativeCredentialReader7100]::CredRead(
        $CredentialTarget,
        1,
        0,
        [ref]$credentialPointer
    )
    if (-not $read) {
        $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "Credential Manager read failed: $errorCode"
    }

    $credential = [Runtime.InteropServices.Marshal]::PtrToStructure(
        $credentialPointer,
        [type][NativeCredentialReader7100+CREDENTIAL]
    )
    $credentialBlob = New-Object byte[] $credential.CredentialBlobSize
    [Runtime.InteropServices.Marshal]::Copy(
        $credential.CredentialBlob,
        $credentialBlob,
        0,
        $credentialBlob.Length
    )
    $password = [Text.Encoding]::Unicode.GetString($credentialBlob)
    if ([string]::IsNullOrEmpty($password)) {
        throw 'Credential Manager returned an empty password'
    }

    $configText = [Text.Encoding]::Unicode.GetString(
        $originalConfigBytes,
        2,
        $originalConfigBytes.Length - 2
    )
    $configLines = [regex]::Split($configText, '\r\n|\n')
    $accountReplaced = 0
    $passwordReplaced = 0
    for ($index = 0; $index -lt $configLines.Length; $index++) {
        if ($configLines[$index] -match '^账号\s*=') {
            $separator = $configLines[$index].IndexOf('=')
            $configLines[$index] = $configLines[$index].Substring(0, $separator + 1) + $account
            $accountReplaced++
        } elseif ($configLines[$index] -match '^密码\s*=') {
            $separator = $configLines[$index].IndexOf('=')
            $configLines[$index] = $configLines[$index].Substring(0, $separator + 1) + $password
            $passwordReplaced++
        }
    }
    if ($accountReplaced -ne 1 -or $passwordReplaced -ne 1) {
        throw "Unexpected credential field counts: account=$accountReplaced password=$passwordReplaced"
    }

    $newLine = ([string][char]13) + ([string][char]10)
    [IO.File]::WriteAllText(
        $ConfigPath,
        [string]::Join($newLine, $configLines),
        [Text.Encoding]::Unicode
    )
    $testConfigHash = (Get-FileHash -LiteralPath $ConfigPath -Algorithm SHA256).Hash
    Add-TimelineEvent ([PSCustomObject]@{
            event = 'credential_config_applied'
            timestamp = (Get-Date).ToUniversalTime().ToString('o')
            account = $account
            password_length = $password.Length
            config_sha256 = $testConfigHash
        })

    pktmon filter remove | Out-Null
    foreach ($port in $ports) {
        pktmon filter add "real1522-$port" -p $port | Out-Null
    }
    pktmon start --capture --comp nics --pkt-size 0 --file-name $etl --file-size 256 |
        Out-Null
    Add-TimelineEvent ([PSCustomObject]@{
            event = 'capture_start'
            timestamp = (Get-Date).ToUniversalTime().ToString('o')
            ports = ($ports -join ',')
            etl = $etl
        })

    Start-Sleep -Seconds 30
    $vendorProcess = Start-Process -FilePath $Executable -WorkingDirectory $WorkingDirectory -PassThru
    Add-TimelineEvent ([PSCustomObject]@{
            event = 'vendor_process_start'
            timestamp = (Get-Date).ToUniversalTime().ToString('o')
            pid = $vendorProcess.Id
            exe = $Executable
        })

    Start-Sleep -Seconds 3
    $modules = @(
        Get-Process -Id $vendorProcess.Id -ErrorAction SilentlyContinue |
            Select-Object -ExpandProperty Modules |
            Select-Object ModuleName, FileName, BaseAddress, ModuleMemorySize
    )
    Add-TimelineEvent ([PSCustomObject]@{
            event = 'module_snapshot'
            timestamp = (Get-Date).ToUniversalTime().ToString('o')
            pid = $vendorProcess.Id
            modules = $modules
        })

    for ($sample = 0; $sample -lt 24; $sample++) {
        $present = $null -ne (
            Get-CimInstance Win32_Process -Filter "ProcessId=$($vendorProcess.Id)" `
                -ErrorAction SilentlyContinue
        )
        $connections = @(
            netstat.exe -ano -p TCP |
                Select-String -Pattern ("\s" + $vendorProcess.Id + "\s*$") |
                ForEach-Object { $_.Line.Trim() }
        )
        $marketFiles = @(
            'D:\Soft\_Stock\飞狐2020\飞狐交易师\DATA\SH\StkData.sif',
            'D:\Soft\_Stock\飞狐2020\飞狐交易师\DATA\SZ\StkData.sif'
        ) | ForEach-Object {
            if (Test-Path -LiteralPath $_) {
                $file = Get-Item -LiteralPath $_
                [PSCustomObject]@{
                    path = $_.ToString()
                    length = $file.Length
                    last_write = $file.LastWriteTimeUtc.ToString('o')
                }
            }
        }
        Add-TimelineEvent ([PSCustomObject]@{
                event = 'process_sample'
                timestamp = (Get-Date).ToUniversalTime().ToString('o')
                sample = $sample
                pid = $vendorProcess.Id
                process_present = $present
                netstat = $connections
                market_files = $marketFiles
            })
        Start-Sleep -Seconds 5
    }

    pktmon stop | Out-Null
    pktmon filter remove | Out-Null
    pktmon etl2pcap $etl --out $pcap | Out-Null
    Add-TimelineEvent ([PSCustomObject]@{
            event = 'capture_stop'
            timestamp = (Get-Date).ToUniversalTime().ToString('o')
            pcap = $pcap
        })
} finally {
    try {
        if ((pktmon status | Out-String) -notmatch '没有运行') {
            pktmon stop | Out-Null
        }
    } catch {
    }
    try {
        pktmon filter remove | Out-Null
    } catch {
    }
    Stop-TestProcesses
    [IO.File]::WriteAllBytes($ConfigPath, $originalConfigBytes)
    if ($null -ne $credentialBlob) {
        [Array]::Clear($credentialBlob, 0, $credentialBlob.Length)
    }
    $password = $null
    if ($credentialPointer -ne [IntPtr]::Zero) {
        [NativeCredentialReader7100]::CredFree($credentialPointer)
    }
}

$configRestored = (
    (Get-FileHash -LiteralPath $ConfigPath -Algorithm SHA256).Hash -eq $originalConfigHash
)
$hashes = Get-FileHash -LiteralPath $etl, $pcap, $timeline -Algorithm SHA256 |
    ForEach-Object { [PSCustomObject]@{ path = $_.Path; sha256 = $_.Hash.ToLowerInvariant() } }
$remaining = @(
    Get-CimInstance Win32_Process |
        Where-Object {
            $_.ExecutablePath -eq $Executable -or
            $_.ExecutablePath -eq 'D:\Soft\_Stock\飞狐2020\飞狐交易师\FoxTrader.exe'
        }
).Count

[PSCustomObject]@{
    config_restored = $configRestored
    target_processes_remaining = $remaining
    hashes = $hashes
} | ConvertTo-Json -Depth 4
