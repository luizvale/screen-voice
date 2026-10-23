param([switch]$Setup)

$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'target\release\screen-voice-live.exe'
$keyDir = Join-Path $env:LOCALAPPDATA 'screen-voice'
$keyFile = Join-Path $keyDir 'openai-api-key.clixml'

if (-not (Test-Path -LiteralPath $exe)) {
    throw "Executavel nao encontrado: $exe. Execute cargo build --release --bin screen-voice-live."
}

if ($Setup -or -not (Test-Path -LiteralPath $keyFile)) {
    $secret = Read-Host 'Cole sua chave da API OpenAI (ela nao aparecera na tela)' -AsSecureString
    if ($secret.Length -eq 0) { throw 'Chave vazia; nada foi salvo.' }
    New-Item -ItemType Directory -Path $keyDir -Force | Out-Null
    $credential = New-Object System.Management.Automation.PSCredential('OpenAI', $secret)
    $credential | Export-Clixml -Path $keyFile
    Write-Host 'Chave salva com criptografia do Windows para esta conta e este computador.'
}

$credential = Import-Clixml -Path $keyFile
if ($credential -isnot [System.Management.Automation.PSCredential]) {
    throw 'Arquivo de chave invalido. Execute novamente com -Setup.'
}

$key = $credential.GetNetworkCredential().Password
if ([string]::IsNullOrWhiteSpace($key)) { throw 'Chave vazia. Execute novamente com -Setup.' }

$running = Get-Process -Name 'screen-voice-live' -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "Screen Voice Live ja esta rodando (PID $($running[0].Id))."
    exit 0
}

$oldKey = $env:OPENAI_API_KEY
try {
    $env:OPENAI_API_KEY = $key
    $process = Start-Process -FilePath $exe -WorkingDirectory $PSScriptRoot -WindowStyle Hidden -PassThru
    Write-Host "Screen Voice Live iniciado (PID $($process.Id))."
    Write-Host "Log: $keyDir\screen-voice-live.log"
} finally {
    if ($null -eq $oldKey) {
        Remove-Item Env:OPENAI_API_KEY -ErrorAction SilentlyContinue
    } else {
        $env:OPENAI_API_KEY = $oldKey
    }
    $key = $null
}
