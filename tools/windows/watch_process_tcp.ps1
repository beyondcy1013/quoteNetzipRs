param(
    [Parameter(Mandatory = $true)]
    [int]$Pid,

    [string]$LogPath = ".\\process_tcp_log.csv",

    [int]$IntervalMs = 200
)

$ErrorActionPreference = "Stop"

$logDir = Split-Path -Parent $LogPath
if ($logDir -and -not (Test-Path -LiteralPath $logDir)) {
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
}

"timestamp,state,local_address,local_port,remote_address,remote_port,pid" | Set-Content -LiteralPath $LogPath -Encoding UTF8

$seen = @{}

Write-Host "watching pid=$Pid interval_ms=$IntervalMs log=$LogPath"
Write-Host "press Ctrl+C to stop"

while ($true) {
    $rows = @()
    try {
        $rows = Get-NetTCPConnection -OwningProcess $Pid -ErrorAction SilentlyContinue
    } catch {
        $rows = @()
    }

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"

    foreach ($row in $rows) {
        $key = "{0}|{1}|{2}|{3}|{4}" -f $row.State, $row.LocalAddress, $row.LocalPort, $row.RemoteAddress, $row.RemotePort
        if (-not $seen.ContainsKey($key)) {
            $seen[$key] = $true
            $line = '"{0}","{1}","{2}","{3}","{4}","{5}","{6}"' -f `
                $timestamp, $row.State, $row.LocalAddress, $row.LocalPort, $row.RemoteAddress, $row.RemotePort, $Pid
            Add-Content -LiteralPath $LogPath -Value $line -Encoding UTF8
            Write-Host $line
        }
    }

    Start-Sleep -Milliseconds $IntervalMs
}
