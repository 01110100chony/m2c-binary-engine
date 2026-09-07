<#
.SYNOPSIS
    M2C Reproducible Benchmarking Harness
.DESCRIPTION
    Executes defensible local benchmarks for M3, M4, M5, and microbenchmarks.
    Separates warm-up from measured runs, captures process working set, verifies
    outputs outside the timed region, and emits machine-readable JSON.
.PARAMETER Profile
    Benchmark profile: 'Smoke' (fast verification) or 'Full' (publication quality).
.PARAMETER OutputJson
    Optional explicit path for the machine-readable benchmark JSON output.
#>
[CmdletBinding()]
param(
    [ValidateSet('Smoke', 'Full')][string]$Profile = 'Smoke',
    [string]$OutputJson = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $repo

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " M2C Benchmark Harness - Profile: $Profile" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# Define safe run output directory under target/benchmarks/
$benchRoot = Join-Path $repo 'target/benchmarks'
if (-not (Test-Path -LiteralPath $benchRoot)) {
    New-Item -ItemType Directory -Path $benchRoot -Force | Out-Null
}
$runId = (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
$runDir = Join-Path $benchRoot $runId
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
Write-Host "Run working directory: $runDir"

# Cleanup helper that refuses to delete outside $runDir or delete reparse points
function Remove-Safe([string]$Path) {
    if (-not $Path -or -not (Test-Path -LiteralPath $Path)) { return }
    $resolved = [IO.Path]::GetFullPath($Path)
    if (-not $resolved.StartsWith($runDir + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -and
        -not ($resolved -eq $runDir)) {
        throw "Refusing to delete path outside run directory: $resolved"
    }
    $item = Get-Item -LiteralPath $resolved -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing to delete reparse point: $resolved"
    }
    if ($item.PSIsContainer) {
        $children = @(Get-ChildItem -LiteralPath $resolved -Recurse -Force)
        foreach ($child in $children) {
            if ($child.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Refusing to delete directory containing reparse point: $($child.FullName)"
            }
        }
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

# 1. Capture Environment (without username or hostname)
Write-Host "Capturing environment metadata..."
$gitCommit = (& git rev-parse HEAD).Trim()
$gitBranch = (& git rev-parse --abbrev-ref HEAD).Trim()
$gitStatus = @(& git status --short)
$gitDirty = ($gitStatus.Count -gt 0)

$harnessPath = if ($PSCommandPath) { $PSCommandPath } else { Join-Path $PSScriptRoot 'benchmark.ps1' }
$harnessSha256 = if (Test-Path -LiteralPath $harnessPath) { (Get-FileHash -LiteralPath $harnessPath).Hash } else { $null }

$renderScript = Join-Path $PSScriptRoot 'render_benchmarks.py'
$renderSha256 = if (Test-Path -LiteralPath $renderScript) { (Get-FileHash -LiteralPath $renderScript).Hash } else { $null }

$rustcVer = (& rustc -Vv) -join "`n"
$cargoVer = (& cargo --version).Trim()

# Query Windows Hardware & OS via CIM (excluding usernames, hostnames, and UUIDs)
$proc = Get-CimInstance Win32_Processor | Select-Object -First 1
$cpuName = if ($proc -and $proc.Name) { $proc.Name.Trim() } else { $env:PROCESSOR_IDENTIFIER }
$numCores = if ($proc -and $proc.NumberOfCores) { [int]$proc.NumberOfCores } else { $null }
$logicalCpus = if ($proc -and $proc.NumberOfLogicalProcessors) { [int]$proc.NumberOfLogicalProcessors } else { [Environment]::ProcessorCount }

$cs = Get-CimInstance Win32_ComputerSystem
$totalPhysicalRamBytes = if ($cs -and $cs.TotalPhysicalMemory) { [long]$cs.TotalPhysicalMemory } else { [GC]::GetGCMemoryInfo().TotalAvailableMemoryBytes }
$totalPhysicalRamGib = [Math]::Round($totalPhysicalRamBytes / 1GB, 2)

$os = Get-CimInstance Win32_OperatingSystem
$osCaption = if ($os -and $os.Caption) { $os.Caption.Trim() } else { [Environment]::OSVersion.ToString() }
$osVersion = if ($os -and $os.Version) { $os.Version.Trim() } else { [Environment]::OSVersion.Version.ToString() }
$osBuild = if ($os -and $os.BuildNumber) { $os.BuildNumber.Trim() } else { $null }

# Storage information
$repoDrive = (Get-Item -LiteralPath $repo).PSDrive.Name
$vol = try { Get-Volume -DriveLetter $repoDrive -ErrorAction SilentlyContinue } catch { $null }
$fs = if ($vol -and $vol.FileSystem) { $vol.FileSystem } else {
    try { (Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$($repoDrive):'").FileSystem } catch { 'NTFS' }
}
$driveType = if ($vol -and $vol.DriveType) { "$($vol.DriveType)" } else { 'Fixed' }
$physDisks = try { @(Get-PhysicalDisk -ErrorAction SilentlyContinue) } catch { @() }
$mediaType = if ($physDisks.Count -gt 0 -and $physDisks[0].MediaType) { "$($physDisks[0].MediaType)" } else { $null }

$envMetadata = [ordered]@{
    git = [ordered]@{
        commit = $gitCommit
        branch = $gitBranch
        dirty = $gitDirty
        benchmark_harness_sha256 = $harnessSha256
        report_generator_sha256 = $renderSha256
    }
    host = [ordered]@{
        os_caption = $osCaption
        os_version = $osVersion
        os_build_number = $osBuild
        architecture = $env:PROCESSOR_ARCHITECTURE
        cpu_name = $cpuName
        number_of_cores = $numCores
        number_of_logical_processors = $logicalCpus
        total_physical_memory_bytes = $totalPhysicalRamBytes
        total_physical_memory_gib = $totalPhysicalRamGib
        filesystem = $fs
        volume_type = $driveType
        storage_media_type = $mediaType
    }
    toolchain = [ordered]@{
        rustc_verbose = $rustcVer
        cargo = $cargoVer
        target = "x86_64-pc-windows-msvc"
        build_profile = "release"
        lto = $false
        opt_level = 3
        codegen_units = "default"
        note = "Standard Cargo release optimization, no LTO configured in Cargo.toml"
        features = @("pqc")
    }
}

# 2. Build Release Binaries Before Any Timing
Write-Host "Building release binaries with --all-features..."
$buildStart = [Diagnostics.Stopwatch]::StartNew()
& cargo build --release --all-features --bin m2c-pipeline --example m6_verify --locked
if ($LASTEXITCODE -ne 0) {
    throw "Cargo release build failed with exit code $LASTEXITCODE"
}
$buildStart.Stop()
Write-Host "Release binaries compiled in $([Math]::Round($buildStart.Elapsed.TotalSeconds, 2))s"

# Snapshot release executables into run directory to avoid rebuild collisions
$binDir = Join-Path $runDir 'bin'
New-Item -ItemType Directory -Path $binDir -Force | Out-Null
$m2cBin = Join-Path $binDir 'm2c-pipeline.exe'
$verifyBin = Join-Path $binDir 'm6_verify.exe'

Copy-Item -LiteralPath (Join-Path $repo 'target/release/m2c-pipeline.exe') -Destination $m2cBin
Copy-Item -LiteralPath (Join-Path $repo 'target/release/examples/m6_verify.exe') -Destination $verifyBin

$envMetadata.toolchain.binary_sha256 = (Get-FileHash $m2cBin).Hash
$envMetadata.toolchain.verifier_sha256 = (Get-FileHash $verifyBin).Hash

# Process Execution Helper with Peak Working Set Monitoring
function Invoke-BenchProcess {
    param(
        [string]$Program,
        [string[]]$Arguments,
        [int]$TimeoutSeconds = 600
    )
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Program
    $info.WorkingDirectory = $repo
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in $Arguments) {
        $info.ArgumentList.Add($arg)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $peakWs = $null
    $timedOut = $false

    $watch = [Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw "Failed to start process: $Program"
    }

    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()

    while (-not $process.WaitForExit(10)) {
        try {
            $process.Refresh()
            $ws = $process.PeakWorkingSet64
            if ($ws -gt 0 -and ($null -eq $peakWs -or $ws -gt $peakWs)) {
                $peakWs = $ws
            }
        } catch { }
        if ($watch.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
            $timedOut = $true
            $process.Kill($true)
            break
        }
    }
    $process.WaitForExit()
    $watch.Stop()

    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    $code = $process.ExitCode
    $process.Dispose()

    if ($timedOut) {
        throw "Process timed out after $TimeoutSeconds seconds: $Program $($Arguments -join ' ')"
    }
    if ($code -ne 0) {
        throw "Process exited with code $($code): $stderr"
    }

    # Attempt to parse CLI --report-json output from stdout if available
    $reportData = $null
    if ($stdout) {
        foreach ($line in ($stdout -split "`n")) {
            $trimmed = $line.Trim()
            if ($trimmed.StartsWith('{') -and $trimmed.EndsWith('}')) {
                try {
                    $parsed = ConvertFrom-Json $trimmed
                    if ($parsed.report_version) {
                        $reportData = $parsed
                        break
                    }
                } catch { }
            }
        }
    }

    return [pscustomobject]@{
        wall_clock_elapsed_ms = $watch.Elapsed.TotalMilliseconds
        observed_peak_working_set_bytes = $peakWs
        exit_code = $code
        report = $reportData
        stdout = $stdout
        stderr = $stderr
    }
}

# Dataset Generator (35 bytes/record, repeating the project's fixture)
function Generate-Dataset([string]$Path, [long]$Records) {
    $recordBytes = [IO.File]::ReadAllBytes((Join-Path $repo 'tests/fixtures/sample_fixed.bin')) # 105 bytes (3 records)
    $block = [byte[]]::new(105 * 1024) # ~107.5 KB chunk
    for ($i = 0; $i -lt 1024; $i++) {
        [Array]::Copy($recordBytes, 0, $block, $i * 105, 105)
    }
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew)
    try {
        $remaining = $Records * 35
        while ($remaining -gt 0) {
            $n = [int][Math]::Min($remaining, $block.Length)
            $stream.Write($block, 0, $n)
            $remaining -= $n
        }
    } finally {
        $stream.Dispose()
    }
}

# Post-timing verification
function Verify-M3Output([string]$DataFile, [string]$OutputFile, [long]$Records, [long]$Batch) {
    $res = Invoke-BenchProcess -Program $verifyBin -Arguments @(
        '--kind', 'm3',
        '--input', $DataFile,
        '--output', $OutputFile,
        '--records', "$Records",
        '--batch-records', "$Batch"
    )
    if ($res.stdout -notmatch '"verified":\s*true') {
        throw "M3 verification failed for output $OutputFile"
    }
}

function Verify-M4Output([string]$DataFile, [string]$OutputDir, [long]$Records, [long]$Batch) {
    $res = Invoke-BenchProcess -Program $verifyBin -Arguments @(
        '--kind', 'm4',
        '--input', $DataFile,
        '--output', $OutputDir,
        '--records', "$Records",
        '--batch-records', "$Batch"
    )
    if ($res.stdout -notmatch '"verified":\s*true') {
        throw "M4 verification failed for output $OutputDir"
    }
}

function Verify-RoundtripOutput([string]$Left, [string]$Right) {
    $res = Invoke-BenchProcess -Program $verifyBin -Arguments @(
        '--kind', 'roundtrip',
        '--input', $Left,
        '--output', $Right
    )
    if ($res.stdout -notmatch '"verified":\s*true') {
        throw "Roundtrip byte equality failed between $Left and $Right"
    }
}

# Statistical Reduction Helper
function Compute-ScenarioStats($Runs, [long]$Records, [long]$InputBytes) {
    $sortedWall = @($Runs | ForEach-Object { $_.wall_clock_elapsed_ms } | Sort-Object)
    $n = $sortedWall.Count
    $medianWall = if ($n % 2 -eq 1) {
        $sortedWall[[int][Math]::Floor($n / 2)]
    } else {
        ($sortedWall[($n / 2) - 1] + $sortedWall[$n / 2]) / 2.0
    }
    $minWall = $sortedWall[0]
    $maxWall = $sortedWall[-1]
    $meanWall = ($sortedWall | Measure-Object -Average).Average

    # Internal CLI report times if available
    $internalTimes = @($Runs | Where-Object { $null -ne $_.internal_elapsed_ms } | ForEach-Object { $_.internal_elapsed_ms } | Sort-Object)
    $medianInternal = if ($internalTimes.Count -gt 0) {
        $internalTimes[[int][Math]::Floor($internalTimes.Count / 2)]
    } else {
        $null
    }

    $peakWsBytes = ($Runs | ForEach-Object { $_.observed_peak_working_set_bytes } | Measure-Object -Maximum).Maximum

    $recordsPerSec = if ($Records -gt 0 -and $medianWall -gt 0) {
        [Math]::Round($Records * 1000.0 / $medianWall, 2)
    } else { $null }

    # Distinction: MiB (1024^2) vs MB (10^6)
    $inputMibPerSec = if ($InputBytes -gt 0 -and $medianWall -gt 0) {
        [Math]::Round(($InputBytes / 1MB) * 1000.0 / $medianWall, 2)
    } else { $null }

    $inputMbPerSec = if ($InputBytes -gt 0 -and $medianWall -gt 0) {
        [Math]::Round(($InputBytes / 1000000.0) * 1000.0 / $medianWall, 2)
    } else { $null }

    return [pscustomobject]@{
        median_wall_clock_elapsed_ms = [Math]::Round($medianWall, 2)
        min_wall_clock_elapsed_ms = [Math]::Round($minWall, 2)
        max_wall_clock_elapsed_ms = [Math]::Round($maxWall, 2)
        mean_wall_clock_elapsed_ms = [Math]::Round($meanWall, 2)
        median_internal_elapsed_ms = $medianInternal
        records_per_second = $recordsPerSec
        input_mib_per_second = $inputMibPerSec
        input_mb_per_second = $inputMbPerSec
        observed_peak_working_set_bytes = $peakWsBytes
        observed_peak_working_set_mib = if ($peakWsBytes) { [Math]::Round($peakWsBytes / 1MB, 2) } else { $null }
    }
}

$benchmarkResults = [Collections.Generic.List[object]]::new()

# Define Scenarios according to Profile
if ($Profile -eq 'Smoke') {
    $warmupCount = 1
    $measuredCount = 1
    $m3Scenarios = @(
        @{ records = 10000; batch = 256 },
        @{ records = 10000; batch = 4096 }
    )
    $m4Scenarios = @(
        @{ records = 10000; batch = 4096 }
    )
    $m5Sizes = @(1MB)
} else {
    $warmupCount = 1
    $measuredCount = 5
    $m3Scenarios = @(
        @{ records = 1000000; batch = 256 },
        @{ records = 1000000; batch = 4096 },
        @{ records = 1000000; batch = 65536 },
        @{ records = 3000000; batch = 256 },
        @{ records = 3000000; batch = 4096 },
        @{ records = 3000000; batch = 65536 }
    )
    $m4Scenarios = @(
        @{ records = 3000000; batch = 4096 },
        @{ records = 3000000; batch = 65536 }
    )
    $m5Sizes = @(64MB)
}

$fixturePath = Join-Path $repo 'tests/fixtures/sample_fixed.bin'
$fixtureSha256 = (Get-FileHash -LiteralPath $fixturePath).Hash
$copybookPath = Join-Path $repo 'tests/fixtures/sample_fixed.cpy'
$copybookSha256 = (Get-FileHash -LiteralPath $copybookPath).Hash

# =========================================================================
# 3. M3 End-to-End Conversion Benchmark
# =========================================================================
Write-Host "`n--- Running M3 Conversion Benchmarks ---" -ForegroundColor Yellow

foreach ($sc in $m3Scenarios) {
    $recs = [long]$sc.records
    $batch = [long]$sc.batch
    $inputBytes = $recs * 35

    Write-Host "M3: $recs records, batch=$batch ($([Math]::Round($inputBytes / 1MB, 2)) MiB input)..."

    $dataFile = Join-Path $runDir "m3-input-$recs-$batch.bin"
    Generate-Dataset $dataFile $recs
    $inputSha256 = (Get-FileHash -LiteralPath $dataFile).Hash

    $warmupRuns = @()
    $measuredRuns = @()

    # Warm-up runs
    for ($w = 1; $w -le $warmupCount; $w++) {
        $outFile = Join-Path $runDir "m3-warmup-$recs-$batch-$w.parquet"
        $runRes = Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'convert',
            '--copybook', $copybookPath,
            '--input', $dataFile,
            '--output', $outFile,
            '--batch-records', "$batch",
            '--report-json'
        )
        Verify-M3Output $dataFile $outFile $recs $batch
        Remove-Safe $outFile
    }

    # Measured runs
    for ($m = 1; $m -le $measuredCount; $m++) {
        $outFile = Join-Path $runDir "m3-run-$recs-$batch-$m.parquet"
        $runRes = Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'convert',
            '--copybook', $copybookPath,
            '--input', $dataFile,
            '--output', $outFile,
            '--batch-records', "$batch",
            '--report-json'
        )
        Verify-M3Output $dataFile $outFile $recs $batch
        Remove-Safe $outFile

        $measuredRuns += [pscustomobject]@{
            iteration = $m
            wall_clock_elapsed_ms = $runRes.wall_clock_elapsed_ms
            internal_elapsed_ms = if ($runRes.report) { $runRes.report.elapsed_ms } else { $null }
            observed_peak_working_set_bytes = $runRes.observed_peak_working_set_bytes
            exit_code = $runRes.exit_code
            verified = $true
            verification_method = "independent-oracle-parquet"
        }
    }

    Remove-Safe $dataFile

    $stats = Compute-ScenarioStats $measuredRuns $recs $inputBytes
    Write-Host "  -> Median: $($stats.median_wall_clock_elapsed_ms) ms | Throughput: $($stats.records_per_second) rec/s ($($stats.input_mib_per_second) MiB/s) | Peak WS: $($stats.observed_peak_working_set_mib) MiB" -ForegroundColor Green

    $benchmarkResults.Add([pscustomobject]@{
        benchmark = "m3-convert"
        command = "convert"
        records = $recs
        record_length = 35
        batch_records = $batch
        input_bytes = $inputBytes
        input_sha256 = $inputSha256
        copybook_sha256 = $copybookSha256
        source_fixture_sha256 = $fixtureSha256
        dataset_design = "deterministic repetition of tests/fixtures/sample_fixed.bin"
        warmup_runs = $warmupCount
        measured_runs = $measuredCount
        runs = $measuredRuns
        median_wall_clock_elapsed_ms = $stats.median_wall_clock_elapsed_ms
        min_wall_clock_elapsed_ms = $stats.min_wall_clock_elapsed_ms
        max_wall_clock_elapsed_ms = $stats.max_wall_clock_elapsed_ms
        mean_wall_clock_elapsed_ms = $stats.mean_wall_clock_elapsed_ms
        median_internal_elapsed_ms = $stats.median_internal_elapsed_ms
        records_per_second = $stats.records_per_second
        input_mib_per_second = $stats.input_mib_per_second
        input_mb_per_second = $stats.input_mb_per_second
        observed_peak_working_set_bytes = $stats.observed_peak_working_set_bytes
        observed_peak_working_set_mib = $stats.observed_peak_working_set_mib
        verified = $true
        verification_method = "independent-oracle-parquet"
    })
}

# =========================================================================
# 4. M4 Recoverable Multipart Conversion Benchmark
# =========================================================================
Write-Host "`n--- Running M4 Multipart Recoverable Benchmarks ---" -ForegroundColor Yellow

foreach ($sc in $m4Scenarios) {
    $recs = [long]$sc.records
    $batch = [long]$sc.batch
    $inputBytes = $recs * 35

    Write-Host "M4: $recs records, batch=$batch ($([Math]::Round($inputBytes / 1MB, 2)) MiB input)..."

    $dataFile = Join-Path $runDir "m4-input-$recs-$batch.bin"
    Generate-Dataset $dataFile $recs
    $inputSha256 = (Get-FileHash -LiteralPath $dataFile).Hash

    $measuredRuns = @()

    # Warm-up run
    for ($w = 1; $w -le $warmupCount; $w++) {
        $outDir = Join-Path $runDir "m4-warmup-$recs-$batch-$w"
        $runRes = Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'convert-parts',
            '--copybook', $copybookPath,
            '--input', $dataFile,
            '--output-dir', $outDir,
            '--batch-records', "$batch",
            '--report-json'
        )
        Verify-M4Output $dataFile $outDir $recs $batch
        Remove-Safe $outDir
    }

    # Measured runs
    for ($m = 1; $m -le $measuredCount; $m++) {
        $outDir = Join-Path $runDir "m4-run-$recs-$batch-$m"
        $runRes = Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'convert-parts',
            '--copybook', $copybookPath,
            '--input', $dataFile,
            '--output-dir', $outDir,
            '--batch-records', "$batch",
            '--report-json'
        )
        Verify-M4Output $dataFile $outDir $recs $batch
        Remove-Safe $outDir

        $measuredRuns += [pscustomobject]@{
            iteration = $m
            wall_clock_elapsed_ms = $runRes.wall_clock_elapsed_ms
            internal_elapsed_ms = if ($runRes.report) { $runRes.report.elapsed_ms } else { $null }
            observed_peak_working_set_bytes = $runRes.observed_peak_working_set_bytes
            exit_code = $runRes.exit_code
            parts = if ($runRes.report) { $runRes.report.dataset_parts } else { [Math]::Ceiling($recs / $batch) }
            verified = $true
            verification_method = "independent-oracle-m4-manifest-receipts"
        }
    }

    Remove-Safe $dataFile

    $stats = Compute-ScenarioStats $measuredRuns $recs $inputBytes
    $partCount = [Math]::Ceiling($recs / $batch)
    Write-Host "  -> Median: $($stats.median_wall_clock_elapsed_ms) ms | Throughput: $($stats.records_per_second) rec/s ($($stats.input_mib_per_second) MiB/s) | Parts: $partCount | Peak WS: $($stats.observed_peak_working_set_mib) MiB" -ForegroundColor Green

    $benchmarkResults.Add([pscustomobject]@{
        benchmark = "m4-convert-parts"
        command = "convert-parts"
        records = $recs
        record_length = 35
        batch_records = $batch
        parts = $partCount
        input_bytes = $inputBytes
        input_sha256 = $inputSha256
        copybook_sha256 = $copybookSha256
        source_fixture_sha256 = $fixtureSha256
        dataset_design = "deterministic repetition of tests/fixtures/sample_fixed.bin"
        warmup_runs = $warmupCount
        measured_runs = $measuredCount
        runs = $measuredRuns
        median_wall_clock_elapsed_ms = $stats.median_wall_clock_elapsed_ms
        min_wall_clock_elapsed_ms = $stats.min_wall_clock_elapsed_ms
        max_wall_clock_elapsed_ms = $stats.max_wall_clock_elapsed_ms
        mean_wall_clock_elapsed_ms = $stats.mean_wall_clock_elapsed_ms
        median_internal_elapsed_ms = $stats.median_internal_elapsed_ms
        records_per_second = $stats.records_per_second
        input_mib_per_second = $stats.input_mib_per_second
        input_mb_per_second = $stats.input_mb_per_second
        observed_peak_working_set_bytes = $stats.observed_peak_working_set_bytes
        observed_peak_working_set_mib = $stats.observed_peak_working_set_mib
        verified = $true
        verification_method = "independent-oracle-m4-manifest-receipts"
    })
}

# =========================================================================
# 5. M5 Post-Quantum Protection Benchmarks (protect & unprotect)
# =========================================================================
Write-Host "`n--- Running M5 Protect / Unprotect Benchmarks ---" -ForegroundColor Yellow

$keysDir = Join-Path $runDir "m5-keys"
Invoke-BenchProcess -Program $m2cBin -Arguments @('keygen', '--output-dir', $keysDir) | Out-Null
$pubKey = Join-Path $keysDir 'public.key'
$secKey = Join-Path $keysDir 'secret.key'

foreach ($size in $m5Sizes) {
    $sizeMib = [Math]::Round($size / 1MB, 2)
    Write-Host "M5: Payload size $sizeMib MiB ($size bytes)..."

    $payloadFile = Join-Path $runDir "m5-payload-$size.bin"
    $fileStream = [IO.File]::Open($payloadFile, [IO.FileMode]::CreateNew)
    $buf = [byte[]]::new(1MB)
    [Array]::Fill[byte]($buf, 0x42)
    try {
        for ($written = 0; $written -lt $size; $written += $buf.Length) {
            $fileStream.Write($buf, 0, $buf.Length)
        }
    } finally {
        $fileStream.Dispose()
    }
    $payloadSha256 = (Get-FileHash $payloadFile).Hash

    # 5.1 Protect
    $protectRuns = @()
    # Warm-up run with roundtrip verification outside timing
    for ($w = 1; $w -le $warmupCount; $w++) {
        $enc = Join-Path $runDir "m5-enc-warmup-$size-$w.m5"
        Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'protect',
            '--input', $payloadFile,
            '--public-key', $pubKey,
            '--output', $enc,
            '--report-json'
        ) | Out-Null
        
        # Verify outside timing
        $tempPlain = Join-Path $runDir "m5-plain-warmup-verify-$size-$w.bin"
        Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'unprotect',
            '--input', $enc,
            '--secret-key', $secKey,
            '--output', $tempPlain
        ) | Out-Null
        Verify-RoundtripOutput $payloadFile $tempPlain
        Remove-Safe $tempPlain
        Remove-Safe $enc
    }

    $lastEncFile = $null
    for ($m = 1; $m -le $measuredCount; $m++) {
        $enc = Join-Path $runDir "m5-enc-run-$size-$m.m5"
        $runRes = Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'protect',
            '--input', $payloadFile,
            '--public-key', $pubKey,
            '--output', $enc,
            '--report-json'
        )

        # Independent roundtrip verification for EACH measured protect run outside the timed region
        $tempPlain = Join-Path $runDir "m5-plain-verify-$size-$m.bin"
        Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'unprotect',
            '--input', $enc,
            '--secret-key', $secKey,
            '--output', $tempPlain
        ) | Out-Null
        Verify-RoundtripOutput $payloadFile $tempPlain
        Remove-Safe $tempPlain

        $protectRuns += [pscustomobject]@{
            iteration = $m
            wall_clock_elapsed_ms = $runRes.wall_clock_elapsed_ms
            internal_elapsed_ms = if ($runRes.report) { $runRes.report.elapsed_ms } else { $null }
            observed_peak_working_set_bytes = $runRes.observed_peak_working_set_bytes
            exit_code = $runRes.exit_code
            verified = $true
            verification_method = "byte-for-byte"
        }
        if ($m -eq $measuredCount) {
            $lastEncFile = $enc
        } else {
            Remove-Safe $enc
        }
    }

    $protectStats = Compute-ScenarioStats $protectRuns 0 $size
    Write-Host "  -> Protect: Median: $($protectStats.median_wall_clock_elapsed_ms) ms | Throughput: $($protectStats.input_mib_per_second) MiB/s | Peak WS: $($protectStats.observed_peak_working_set_mib) MiB" -ForegroundColor Green

    $benchmarkResults.Add([pscustomobject]@{
        benchmark = "m5-protect"
        command = "protect"
        payload_bytes = $size
        payload_sha256 = $payloadSha256
        warmup_runs = $warmupCount
        measured_runs = $measuredCount
        runs = $protectRuns
        median_wall_clock_elapsed_ms = $protectStats.median_wall_clock_elapsed_ms
        min_wall_clock_elapsed_ms = $protectStats.min_wall_clock_elapsed_ms
        max_wall_clock_elapsed_ms = $protectStats.max_wall_clock_elapsed_ms
        mean_wall_clock_elapsed_ms = $protectStats.mean_wall_clock_elapsed_ms
        median_internal_elapsed_ms = $protectStats.median_internal_elapsed_ms
        input_mib_per_second = $protectStats.input_mib_per_second
        input_mb_per_second = $protectStats.input_mb_per_second
        observed_peak_working_set_bytes = $protectStats.observed_peak_working_set_bytes
        observed_peak_working_set_mib = $protectStats.observed_peak_working_set_mib
        verified = $true
        verification_method = "byte-for-byte"
    })

    # 5.2 Unprotect
    $unprotectRuns = @()
    for ($w = 1; $w -le $warmupCount; $w++) {
        $plain = Join-Path $runDir "m5-plain-warmup-$size-$w.bin"
        Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'unprotect',
            '--input', $lastEncFile,
            '--secret-key', $secKey,
            '--output', $plain,
            '--report-json'
        ) | Out-Null
        Verify-RoundtripOutput $payloadFile $plain
        Remove-Safe $plain
    }

    for ($m = 1; $m -le $measuredCount; $m++) {
        $plain = Join-Path $runDir "m5-plain-run-$size-$m.bin"
        $runRes = Invoke-BenchProcess -Program $m2cBin -Arguments @(
            'unprotect',
            '--input', $lastEncFile,
            '--secret-key', $secKey,
            '--output', $plain,
            '--report-json'
        )

        # Verify EACH recovered plaintext outside the timed region
        Verify-RoundtripOutput $payloadFile $plain
        Remove-Safe $plain

        $unprotectRuns += [pscustomobject]@{
            iteration = $m
            wall_clock_elapsed_ms = $runRes.wall_clock_elapsed_ms
            internal_elapsed_ms = if ($runRes.report) { $runRes.report.elapsed_ms } else { $null }
            observed_peak_working_set_bytes = $runRes.observed_peak_working_set_bytes
            exit_code = $runRes.exit_code
            verified = $true
            verification_method = "byte-for-byte"
        }
    }

    Remove-Safe $lastEncFile
    Remove-Safe $payloadFile

    $unprotectStats = Compute-ScenarioStats $unprotectRuns 0 $size
    Write-Host "  -> Unprotect: Median: $($unprotectStats.median_wall_clock_elapsed_ms) ms | Throughput: $($unprotectStats.input_mib_per_second) MiB/s | Peak WS: $($unprotectStats.observed_peak_working_set_mib) MiB" -ForegroundColor Green

    $benchmarkResults.Add([pscustomobject]@{
        benchmark = "m5-unprotect"
        command = "unprotect"
        payload_bytes = $size
        payload_sha256 = $payloadSha256
        warmup_runs = $warmupCount
        measured_runs = $measuredCount
        runs = $unprotectRuns
        median_wall_clock_elapsed_ms = $unprotectStats.median_wall_clock_elapsed_ms
        min_wall_clock_elapsed_ms = $unprotectStats.min_wall_clock_elapsed_ms
        max_wall_clock_elapsed_ms = $unprotectStats.max_wall_clock_elapsed_ms
        mean_wall_clock_elapsed_ms = $unprotectStats.mean_wall_clock_elapsed_ms
        median_internal_elapsed_ms = $unprotectStats.median_internal_elapsed_ms
        input_mib_per_second = $unprotectStats.input_mib_per_second
        input_mb_per_second = $unprotectStats.input_mb_per_second
        observed_peak_working_set_bytes = $unprotectStats.observed_peak_working_set_bytes
        observed_peak_working_set_mib = $unprotectStats.observed_peak_working_set_mib
        verified = $true
        verification_method = "byte-for-byte"
    })
}

Remove-Safe $keysDir

# =========================================================================
# 6. Microbenchmarks (benches/m6.rs)
# =========================================================================
Write-Host "`n--- Running Microbenchmarks (benches/m6.rs) ---" -ForegroundColor Yellow

$microProfileArg = if ($Profile -eq 'Smoke') { 'smoke' } else { 'full' }
$benchOutputLines = & cargo bench --bench m6 -- --profile $microProfileArg
if ($LASTEXITCODE -ne 0) {
    throw "Microbench execution failed with exit code $LASTEXITCODE"
}

$microSamples = [Collections.Generic.List[object]]::new()
foreach ($line in $benchOutputLines) {
    $trimmed = $line.Trim()
    if ($trimmed.StartsWith('{') -and $trimmed.EndsWith('}')) {
        try {
            $parsed = ConvertFrom-Json $trimmed
            if ($parsed.operation) {
                $microSamples.Add($parsed)
            }
        } catch { }
    }
}

# Aggregate microbench samples excluding warmups
$microSummary = [Collections.Generic.List[object]]::new()
$microGroups = $microSamples | Where-Object { -not $_.warmup } | Group-Object workload, operation

foreach ($grp in $microGroups) {
    $items = @($grp.Group)
    $nsList = @($items | ForEach-Object { [long]$_.ns_per_iteration } | Sort-Object)
    $n = $nsList.Count
    $medianNs = if ($n % 2 -eq 1) {
        $nsList[[int][Math]::Floor($n / 2)]
    } else {
        ($nsList[($n / 2) - 1] + $nsList[$n / 2]) / 2.0
    }

    $first = $items[0]
    $recordsPerSec = if ($first.records_per_iteration -and $medianNs -gt 0) {
        [Math]::Round(($first.records_per_iteration * 1000000000.0) / $medianNs, 2)
    } else { $null }

    # Distinction: MiB/s (1024^2) and MB/s (10^6)
    $bytesPerSec = if ($first.input_bytes_per_iteration -and $medianNs -gt 0) {
        ($first.input_bytes_per_iteration * 1000000000.0) / $medianNs
    } else { $null }

    $inputMibPerSec = if ($bytesPerSec) { [Math]::Round($bytesPerSec / 1048576.0, 2) } else { $null }
    $inputMbPerSec = if ($bytesPerSec) { [Math]::Round($bytesPerSec / 1000000.0, 2) } else { $null }

    Write-Host "Micro: Workload=$($first.workload), Op=$($first.operation) -> Median: $medianNs ns/it $(if ($recordsPerSec) { "($recordsPerSec records/s, $inputMibPerSec MiB/s)" } else { '' })" -ForegroundColor Green

    $microSummary.Add([pscustomobject]@{
        workload = $first.workload
        operation = $first.operation
        samples = $n
        median_ns_per_iteration = [long]$medianNs
        min_ns_per_iteration = $nsList[0]
        max_ns_per_iteration = $nsList[-1]
        mean_ns_per_iteration = [Math]::Round(($nsList | Measure-Object -Average).Average, 2)
        records_per_iteration = $first.records_per_iteration
        records_per_second = $recordsPerSec
        input_bytes_per_iteration = $first.input_bytes_per_iteration
        input_mib_per_second = $inputMibPerSec
        input_mb_per_second = $inputMbPerSec
    })
}

# =========================================================================
# 7. Emit Machine-Readable JSON & Summary
# =========================================================================
$finalReport = [ordered]@{
    schema_version = 1
    generated_at = (Get-Date -Format 'o')
    profile = $Profile.ToLower()
    git = $envMetadata.git
    host = $envMetadata.host
    toolchain = $envMetadata.toolchain
    benchmarks = $benchmarkResults
    microbenchmarks = $microSummary
}

$jsonPath = if ($OutputJson) {
    [IO.Path]::GetFullPath($OutputJson)
} else {
    Join-Path $runDir 'benchmark-result.json'
}

$jsonContent = ConvertTo-Json -InputObject $finalReport -Depth 15
[IO.File]::WriteAllText($jsonPath, $jsonContent, [Text.UTF8Encoding]::new($false))
Write-Host "`nMachine-readable benchmark JSON saved to: $jsonPath" -ForegroundColor Cyan

Write-Host "`n============================================================" -ForegroundColor Cyan
Write-Host " Benchmark Suite Completed Successfully ($Profile profile)" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
