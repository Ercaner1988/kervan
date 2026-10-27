param(
  [string]$Kaynak = "$env:USERPROFILE\.config\opencode\opencode.json",
  [string]$Hedef  = "$env:TEMP\opencode-kervan-test.json",
  [switch]$Gercek
)
$ErrorActionPreference = "Stop"

# Model listesi ÖLÇÜMDEN gelir: elle yazılmış katalog kabul edilmez.
$m = Invoke-WebRequest -Uri "http://127.0.0.1:20129/v1/models" -UseBasicParsing -TimeoutSec 30
$j = [System.Text.Encoding]::UTF8.GetString($m.RawContentStream.ToArray()) | ConvertFrom-Json

$modeller = [ordered]@{}
foreach ($d in $j.data) {
    $id = $d.id
    if (-not $id.StartsWith("kervan/")) { continue }
    # OpenCode "saglayici/model" ayirir; model anahtari kervan/ SONRASIDIR.
    $anahtar = $id.Substring("kervan/".Length)
    $ad = $anahtar
    if ($d.kilitli) { $ad = "$anahtar [kilitli]" }
    $modeller[$anahtar] = [ordered]@{ name = $ad; modelID = $id }
}

$cfg = Get-Content $Kaynak -Raw -Encoding utf8 | ConvertFrom-Json
$kervan = [ordered]@{
    package  = "aisdk:@ai-sdk/openai-compatible"
    name     = "Kervan (Tarayici Hesaplari)"
    settings = [ordered]@{
        baseURL = "http://127.0.0.1:20129/v1"
        apiKey  = "kervan-yerel"
    }
    models   = $modeller
}

if ($Gercek) {
    $yedek = "$Kaynak.yedek-$(Get-Date -Format yyyyMMdd-HHmmss)"
    Copy-Item $Kaynak $yedek
    "YEDEK=$yedek"
}
$cfg.providers | Add-Member -NotePropertyName kervan -NotePropertyValue $kervan -Force
$metin = $cfg | ConvertTo-Json -Depth 40
[System.IO.File]::WriteAllText($Hedef, $metin, (New-Object System.Text.UTF8Encoding($false)))
"HEDEF=$Hedef MODEL_SAYISI=$($modeller.Count)"
