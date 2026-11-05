param(
    [string] $BlenderPath,
    [ValidateSet('siroco', 'oquedad', 'jovara')] [string] $Planet,
    [switch] $Overwrite,
    [ValidateRange(1, 1024)] [int] $Samples = 64,
    [ValidateRange(128, 4096)] [int] $Size = 960
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path

if (-not $BlenderPath) {
    $blenderCommand = Get-Command blender -ErrorAction SilentlyContinue
    if ($blenderCommand) {
        $BlenderPath = $blenderCommand.Source
    }
}

if (-not $BlenderPath) {
    $candidates = @(
        Get-ChildItem -Path (Join-Path $projectRoot 'tmp/blender-runtime/*/blender.exe') -ErrorAction SilentlyContinue
        Get-ChildItem -Path 'C:/Program Files/Blender Foundation/*/blender.exe' -ErrorAction SilentlyContinue
    )
    $latestBlender = $candidates | Sort-Object FullName -Descending | Select-Object -First 1
    if ($latestBlender) {
        $BlenderPath = $latestBlender.FullName
    }
}

if (-not $BlenderPath -or -not (Test-Path -LiteralPath $BlenderPath -PathType Leaf)) {
    throw 'No se encontro Blender. Indica su ejecutable mediante -BlenderPath.'
}

$scriptName = if ($Planet -eq 'jovara') { 'create_jovara.py' } else { 'create_planets.py' }
$generator = Join-Path $PSScriptRoot $scriptName
$arguments = @('--background', '--factory-startup', '--python-exit-code', '1',
    '--python', $generator, '--', '--samples', $Samples, '--size', $Size)
if ($Overwrite) {
    $arguments += '--overwrite'
}
if ($Planet -and $Planet -ne 'jovara') {
    $arguments += @('--only', $Planet)
}

# Keep generator caches within the project; settings are scoped to this process.
$previousConfig = $env:BLENDER_USER_CONFIG
$previousExtensions = $env:BLENDER_USER_EXTENSIONS
try {
    $env:BLENDER_USER_CONFIG = Join-Path $projectRoot 'tmp/blender-config'
    $env:BLENDER_USER_EXTENSIONS = Join-Path $projectRoot 'tmp/blender-extensions'
    & $BlenderPath @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Blender termino con codigo $LASTEXITCODE."
    }
} finally {
    $env:BLENDER_USER_CONFIG = $previousConfig
    $env:BLENDER_USER_EXTENSIONS = $previousExtensions
}
