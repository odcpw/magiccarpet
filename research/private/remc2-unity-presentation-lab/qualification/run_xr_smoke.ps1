[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectPath,

    [string]$UnityPath = "",

    [string]$OutputRoot = ""
)

$ErrorActionPreference = "Stop"

function Resolve-UnityEditor {
    param([string]$RequestedPath)

    if (-not [string]::IsNullOrWhiteSpace($RequestedPath)) {
        $resolved = (Resolve-Path -LiteralPath $RequestedPath).Path
        if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
            throw "Unity editor was not found: $resolved"
        }

        return $resolved
    }

    $hubRoot = Join-Path ${env:ProgramFiles} "Unity\Hub\Editor"
    $candidate = Get-ChildItem -LiteralPath $hubRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object -Property Name -Descending |
        ForEach-Object {
            $editor = Join-Path $_.FullName "Editor\Unity.exe"
            if (Test-Path -LiteralPath $editor -PathType Leaf) {
                Get-Item -LiteralPath $editor
            }
        } |
        Select-Object -First 1

    if ($null -eq $candidate) {
        throw "No Unity Hub editor was found under $hubRoot; pass -UnityPath explicitly"
    }

    return $candidate.FullName
}

function Invoke-CheckedProcess {
    param(
        [string]$Executable,
        [string[]]$Arguments,
        [string]$Name
    )

    Write-Host "[VR-B] $Name"
    & $Executable @Arguments
    # Some SSH-hosted Windows shells do not populate LASTEXITCODE when the
    # child process terminates during a redirected Unity batch run. Preserve a
    # deterministic harness failure instead of emitting a blank exit code.
    $exitCode = if ($null -eq $LASTEXITCODE) { 1 } else { [int]$LASTEXITCODE }
    if ($exitCode -ne 0) {
        throw "$Name failed with exit code $exitCode"
    }

    return $exitCode
}

function Invoke-SmokePlayer {
    param(
        [string]$PlayerPath,
        [string]$Profile,
        [string]$LogPath,
        [bool]$Headless
    )

    $arguments = @(
        "-batchmode",
        "-mc-xr-profile=$Profile",
        "-mc-xr-smoke-once",
        "-logFile",
        $LogPath
    )
    if ($Headless) {
        $arguments += "-nographics"
    }

    Write-Host "[VR-B] smoke profile=$Profile headless=$Headless"
    & $PlayerPath @arguments
    $exitCode = $LASTEXITCODE
    if (-not (Test-Path -LiteralPath $LogPath -PathType Leaf)) {
        throw "Smoke player did not produce a log: $LogPath"
    }

    return [pscustomobject]@{
        profile = $Profile
        exit_code = $exitCode
        log = $LogPath
        text = Get-Content -LiteralPath $LogPath -Raw
    }
}

try {
    $project = (Resolve-Path -LiteralPath $ProjectPath).Path
    $unity = Resolve-UnityEditor -RequestedPath $UnityPath
    if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
        $OutputRoot = Join-Path $env:TEMP "mc2-vr-b-smoke"
    }

    New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
    $player = Join-Path $OutputRoot "MagicCarpetXrSmoke.exe"
    $buildLog = Join-Path $OutputRoot "build.log"
    $flatLog = Join-Path $OutputRoot "flat.log"
    $pcLog = Join-Path $OutputRoot "pc-openxr.log"
    $resultPath = Join-Path $OutputRoot "qualification-result.json"

    Invoke-CheckedProcess -Executable $unity -Name "Win64 smoke build" -Arguments @(
        "-batchmode",
        "-nographics",
        "-quit",
        "-buildTarget",
        "Win64",
        "-projectPath",
        $project,
        "-executeMethod",
        "MagicCarpet.PresentationLab.Editor.CreatePresentationLabAssets.BuildOpenXrSmokePlayer",
        "-mc-build-path",
        $player,
        "-mc-xr-profile=pc-openxr",
        "-logFile",
        $buildLog
    ) | Out-Null

    $flat = Invoke-SmokePlayer -PlayerPath $player -Profile "off" -LogPath $flatLog -Headless $true
    if ($flat.exit_code -ne 0 -or
        $flat.text -notmatch "XR DISABLED: flat presentation has no XR loader or XR Origin" -or
        $flat.text -notmatch "smoke completed with exit-code=0") {
        throw "flat-disabled smoke did not prove a clean XR-free exit"
    }

    # PC OpenXR deliberately keeps graphics enabled. SteamVR/OpenXR needs the
    # logged-in Windows desktop and a real display/session for physical tests.
    $pc = Invoke-SmokePlayer -PlayerPath $player -Profile "pc-openxr" -LogPath $pcLog -Headless $false
    $controllerUnavailable = $pc.text -match "XR CONTROLLERS UNAVAILABLE"
    $runtimeUnavailable = $pc.text -match "XR UNAVAILABLE"
    $runtimeActive = $pc.text -match "PC OPENXR ACTIVE:"
    if (-not ($controllerUnavailable -or $runtimeUnavailable -or $runtimeActive)) {
        throw "PC OpenXR smoke produced no recognized runtime diagnostic"
    }

    if ($runtimeActive -and -not $controllerUnavailable) {
        if ($pc.exit_code -ne 0 -or $pc.text -notmatch "XR loader shutdown complete") {
            throw "PC OpenXR active smoke did not prove clean loader shutdown"
        }

        $pcStatus = "active"
    } elseif ($controllerUnavailable) {
        if ($pc.exit_code -eq 0 -or $pc.text -notmatch "XR loader shutdown complete") {
            throw "controller absence was not reported as a failed, torn-down smoke run"
        }

        $pcStatus = "controllers-unavailable"
    } else {
        if ($pc.exit_code -eq 0) {
            throw "missing OpenXR runtime was reported with a success exit code"
        }

        $pcStatus = "runtime-unavailable"
    }

    $result = [pscustomobject]@{
        schema_version = 1
        target = "StandaloneWindows64"
        project = $project
        player = $player
        flat_status = "passed"
        pc_status = $pcStatus
        pc_exit_code = $pc.exit_code
        logs = @{
            build = $buildLog
            flat = $flatLog
            pc_openxr = $pcLog
        }
        generated_utc = [DateTime]::UtcNow.ToString("o")
    }
    $result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $resultPath -Encoding UTF8
    $result | ConvertTo-Json -Depth 5

    if ($pcStatus -eq "active") {
        exit 0
    }

    exit 2
} catch {
    Write-Error $_
    exit 1
}
