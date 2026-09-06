[CmdletBinding()]
param(
    [ValidateSet('Verify','Fuzz','Bench','Demo')][string]$Mode = 'Verify',
    [Alias('Profile')][ValidateSet('Smoke','Full','Stress')][string]$RunProfile = 'Smoke',
    [string]$OutputRoot = '',
    [string]$Replay = '',
    [ValidateSet('m4','m5')][string]$ReplayKind = 'm4'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $repo
if ($RunProfile -eq 'Stress' -and $Mode -ne 'Bench') { throw 'Stress is only a Bench profile' }
if (-not $OutputRoot) { $OutputRoot = Join-Path $repo 'target/m6' }
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
function Assert-SafeAncestors([string]$Path) {
    $cursor = $Path
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse point in evidence path' }
            foreach ($marker in @('.m4.lock','manifest.json','.manifest.json.tmp','complete.json')) {
                if (Test-Path -LiteralPath (Join-Path $cursor $marker)) { throw 'Evidence must be outside M4 namespace' }
            }
        }
        $parent = [IO.Directory]::GetParent($cursor)
        $cursor = if ($parent) { $parent.FullName } else { '' }
    }
}
Assert-SafeAncestors $OutputRoot
[IO.Directory]::CreateDirectory($OutputRoot) | Out-Null
$run = Join-Path $OutputRoot ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run | Out-Null
$script:commandIndex = 0
$script:commands = [Collections.Generic.List[object]]::new()
$script:samples = [Collections.Generic.List[object]]::new()
$script:gateFailures = [Collections.Generic.List[string]]::new()
function Save-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (ConvertTo-Json -InputObject $Value -Depth 15), [Text.UTF8Encoding]::new($false))
}
function Invoke-Recorded {
    param([string]$Program,[string[]]$Arguments,[string]$Label='command',[int]$TimeoutSeconds=1800,[switch]$AllowFailure)
    $script:commandIndex++
    $stem = '{0:D4}-{1}' -f $script:commandIndex,$Label
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$Program; $info.WorkingDirectory=$repo; $info.UseShellExecute=$false
    $info.CreateNoWindow=$true; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process=[Diagnostics.Process]::new(); $process.StartInfo=$info
    $watch=[Diagnostics.Stopwatch]::StartNew(); $peak=$null; $timedOut=$false
    if (-not $process.Start()) { throw "Could not start $Program" }
    $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
    while (-not $process.WaitForExit(20)) {
        try { $process.Refresh(); $observed=$process.PeakWorkingSet64; if ($observed -gt 0 -and ($null -eq $peak -or $observed -gt $peak)) { $peak=$observed } } catch { }
        if ($watch.Elapsed.TotalSeconds -gt $TimeoutSeconds) { $timedOut=$true; $process.Kill($true); break }
    }
    $process.WaitForExit(); $watch.Stop()
    $text=$stdout.GetAwaiter().GetResult(); $errors=$stderr.GetAwaiter().GetResult(); $code=$process.ExitCode
    [IO.File]::WriteAllText((Join-Path $run "$stem.stdout.log"),$text)
    [IO.File]::WriteAllText((Join-Path $run "$stem.stderr.log"),$errors)
    $result=[pscustomobject]@{ label=$Label; program=$Program; arguments=$Arguments; exit_code=$code;
        campaign_seed=$env:M6_TEST_SEED; campaign_cases=$env:M6_TEST_CASES; replay_case=$env:M6_TEST_REPLAY;
        elapsed_ms=$watch.Elapsed.TotalMilliseconds; observed_peak_working_set_bytes=$peak; timed_out=$timedOut;
        stdout="$stem.stdout.log"; stderr="$stem.stderr.log"; skipped_lines=@(($text + "`n" + $errors) -split "`n" | Where-Object {$_ -match '(?i)skipped|skipping'}) }
    $script:commands.Add($result); Save-Json (Join-Path $run 'commands.json') $script:commands
    $process.Dispose()
    if ($timedOut -or ($code -ne 0 -and -not $AllowFailure)) { throw "$Label failed (exit=$code, timeout=$timedOut), see $run" }
    if ($Label -match '^(fuzz-|concrete-replay$|replay$|crash-recovery-demo$|legacy-cli-stderr$|report-json-errors$|campaign-self-test$)' -and $text -notmatch 'test result: ok\. [1-9][0-9]* passed') {
        throw "$Label did not execute a test"
    }
    if ($Label -in @('fuzz-m4','fuzz-m5') -and $text -notmatch 'M6_CAMPAIGN .*result=true') { throw 'Missing completed campaign marker' }
    if ($Label -in @('concrete-replay','replay') -and $text -notmatch 'M6_REPLAY_PASS') { throw 'Missing replay marker' }
    return $result
}
function Cargo([string[]]$Arguments,[string]$Label) { Invoke-Recorded -Program 'cargo' -Arguments $Arguments -Label $Label }
function Remove-Owned([string]$Path) {
    $resolved=[IO.Path]::GetFullPath($Path)
    if (-not $resolved.StartsWith($run + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Cleanup outside run' }
    if (Test-Path -LiteralPath $resolved) {
        $all=@(Get-Item -LiteralPath $resolved -Force) + @(Get-ChildItem -LiteralPath $resolved -Recurse -Force)
        if (@($all | Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}).Count) { throw 'Cleanup refuses reparse points' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
function Make-Dataset([string]$Path,[long]$Records) {
    $recordBytes=[IO.File]::ReadAllBytes((Join-Path $repo 'tests/fixtures/sample_fixed.bin'))
    $block=[byte[]]::new(105*1024)
    for ($i=0;$i -lt 1024;$i++) { [Array]::Copy($recordBytes,0,$block,$i*105,105) }
    $stream=[IO.File]::Open($Path,[IO.FileMode]::CreateNew)
    try { $remaining=$Records*35; while ($remaining -gt 0) { $n=[int][Math]::Min($remaining,$block.Length); $stream.Write($block,0,$n); $remaining-=$n } } finally {$stream.Dispose()}
}
function Verify-Output([string]$Kind,[string]$InputFile,[string]$OutputFile,[long]$Records,[long]$Batch) {
    $arguments=@('--kind',$Kind,'--input',$InputFile,'--output',$OutputFile)
    if ($Kind -ne 'roundtrip') { $arguments+=@('--records',"$Records",'--batch-records',"$Batch") }
    Invoke-Recorded -Program $script:verifier -Arguments $arguments -Label 'verify-output' | Out-Null
}
function Run-Conversion([string]$Kind,[string]$InputFile,[string]$OutputFile,[long]$Batch,[switch]$Resume) {
    $flag=if ($Kind -eq 'convert') {'--output'} else {'--output-dir'}
    $arguments=@($Kind,'--copybook',(Join-Path $repo 'tests/fixtures/sample_fixed.cpy'),'--input',$InputFile,$flag,$OutputFile,'--batch-records',"$Batch",'--report-json')
    if ($Resume) {$arguments+='--resume'}
    Invoke-Recorded -Program $script:binary -Arguments $arguments -Label $Kind
}
function Record-Sample($Result,[string]$Operation,[long]$Records,[long]$Bytes,[long]$Batch,[int]$Sample) {
    $row=[pscustomobject]@{operation=$Operation;records=$Records;input_bytes=$Bytes;batch_records=$Batch;sample=$Sample;warmup=($Sample -eq 0);
        elapsed_ms=$Result.elapsed_ms;observed_peak_working_set_bytes=$Result.observed_peak_working_set_bytes;
        records_per_second=if ($Records -gt 0 -and $Result.elapsed_ms -gt 0) {$Records*1000.0/$Result.elapsed_ms} else {$null};
        bytes_per_second=if ($Result.elapsed_ms -gt 0) {$Bytes*1000.0/$Result.elapsed_ms} else {$null};verified=$true}
    $script:samples.Add($row)
    [IO.File]::AppendAllText((Join-Path $run 'samples.jsonl'),(ConvertTo-Json -InputObject $row -Compress)+"`n")
}
$outcome='FAIL'
try {
    $rust=Cargo @('--version') 'cargo-version'
    Invoke-Recorded -Program 'rustc' -Arguments @('-Vv') -Label 'rustc-version' | Out-Null
    $commit=(& git rev-parse HEAD); $dirty=@(& git status --short)
    $environment=[ordered]@{mode=$Mode;profile=$RunProfile;commit=$commit;dirty=$dirty;os=[Environment]::OSVersion.ToString();
        powershell=$PSVersionTable.PSVersion.ToString();cpu=$env:PROCESSOR_IDENTIFIER;logical_processors=[Environment]::ProcessorCount;
        total_memory_available_bytes=[GC]::GetGCMemoryInfo().TotalAvailableMemoryBytes;lock_sha256=(Get-FileHash Cargo.lock).Hash;
        fixture_sha256=(Get-FileHash tests/fixtures/sample_fixed.bin).Hash;copybook_sha256=(Get-FileHash tests/fixtures/sample_fixed.cpy).Hash;
        corpus_sha256=(Get-FileHash tests/fixtures/m6/mutations.json).Hash;cache='warm; no purge';measurements='sequential; no concurrent experiment launched by runner'}
    $sourceHashes=[ordered]@{}
    foreach ($source in @(& git ls-files -co --exclude-standard | Sort-Object -Unique)) {
        if ($source -match '^(src/|tests/|examples/|benches/|scripts/|Cargo\.)') { $sourceHashes[$source]=(Get-FileHash -LiteralPath $source).Hash }
    }
    $environment.source_sha256=$sourceHashes
    if ($IsWindows) { $drive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($run)); $environment.volume_format=$drive.DriveFormat; $environment.volume_type="$($drive.DriveType)"; $environment.free_bytes=$drive.AvailableFreeSpace }
    Save-Json (Join-Path $run 'environment.json') $environment
    if ($Mode -eq 'Verify') {
        $checks=@(
            @{label='fmt'; argv=@('fmt','--all','--','--check')},
            @{label='clippy'; argv=@('clippy','--all-targets','--all-features','--','-D','warnings')},
            @{label='tests-default'; argv=@('test','--all-targets')},
            @{label='tests-pqc'; argv=@('test','--all-targets','--all-features')},
            @{label='doc-default'; argv=@('test','--doc')},
            @{label='doc-pqc'; argv=@('test','--doc','--all-features')},
            @{label='protection-release'; argv=@('test','--release','--test','protection','--all-features')},
            @{label='reparse-evidence'; argv=@('test','--all-features','reparse_point_in_write_path_fails_closed','--','--nocapture')},
            @{label='legacy-cli-stderr'; argv=@('test','--test','cli_report','--all-features','legacy_','--','--nocapture')},
            @{label='report-json-errors'; argv=@('test','--test','cli_report','--all-features','report_flag_errors_and_disabled_mode','--','--exact','--nocapture')},
            @{label='campaign-self-test'; argv=@('test','--lib','m6_campaign::tests::artificial_generated_failure_persists_and_replays_without_rng','--','--exact','--nocapture')}
        )
        foreach ($check in $checks) { try { Cargo $check.argv $check.label | Out-Null } catch { $script:gateFailures.Add($_.ToString()) } }
        if ($script:gateFailures.Count) { throw ($script:gateFailures -join "`n") }
    } elseif ($Mode -eq 'Fuzz') {
        $old=@{}; foreach ($key in @('M6_TEST_SEED','M6_TEST_CASES','M6_TEST_OUTPUT','M6_TEST_REPLAY','M6_TEST_COMMIT','M6_HARNESS_SELF_TEST','M6_HARNESS_GENERATED_ONLY')) { $old[$key]=[Environment]::GetEnvironmentVariable($key) }
        try {
            $env:M6_TEST_OUTPUT=Join-Path $run 'corpus'
            $env:M6_TEST_COMMIT=$commit
            [Environment]::SetEnvironmentVariable('M6_HARNESS_SELF_TEST',$null)
            [Environment]::SetEnvironmentVariable('M6_HARNESS_GENERATED_ONLY',$null)
            if ($Replay) {
                $env:M6_TEST_REPLAY=[IO.Path]::GetFullPath($Replay)
                $filter=if ($ReplayKind -eq 'm4') {'m6_combined_resume_mutations'} else {'m6_structured_protection_mutations'}
                Cargo @('test','--release','--all-features','--lib',$filter,'--','--nocapture') 'replay' | Out-Null
            } else {
                [Environment]::SetEnvironmentVariable('M6_TEST_REPLAY',$null)
                $seeds=if ($RunProfile -eq 'Full') {@(0x4D3643,0x4D3644,0x4D3645,0x4D3646)} else {@(0x4D3643)}
                foreach ($seed in $seeds) {
                    $env:M6_TEST_SEED="$seed"; $env:M6_TEST_CASES=if ($RunProfile -eq 'Full') {'10000'} else {'128'}
                    Cargo @('test','--release','--lib','deterministic_arbitrary_invalid_sources_never_panic','--','--nocapture') 'fuzz-parser' | Out-Null
                    Cargo @('test','--release','--test','decode_properties','--','--nocapture') 'fuzz-decoder' | Out-Null
                    $env:M6_TEST_CASES=if ($RunProfile -eq 'Full') {'256'} else {'8'}
                    Cargo @('test','--release','--lib','m6_combined_resume_mutations','--','--nocapture') 'fuzz-m4' | Out-Null
                    Cargo @('test','--release','--all-features','--lib','m6_structured_protection_mutations','--','--nocapture') 'fuzz-m5' | Out-Null
                }
                foreach ($family in @('m4','m5')) {
                    $example=Get-ChildItem -LiteralPath $env:M6_TEST_OUTPUT -Directory | Where-Object {$_.Name.StartsWith($family+'-')} | Select-Object -First 1
                    if (-not $example) {throw "Missing concrete replay for $family"}
                    $env:M6_TEST_REPLAY=Join-Path $example.FullName 'replay.json'
                    $filter=if ($family -eq 'm4') {'m6_combined_resume_mutations'} else {'m6_structured_protection_mutations'}
                    Cargo @('test','--release','--all-features','--lib',$filter,'--','--nocapture') 'concrete-replay' | Out-Null
                }
            }
        } finally { foreach ($key in $old.Keys) { [Environment]::SetEnvironmentVariable($key,$old[$key]) } }
    } else {
        if (-not $IsWindows -or $drive.DriveFormat -ne 'NTFS' -or $drive.DriveType -ne 'Fixed') { throw 'Demo/Bench requires Windows local fixed NTFS' }
        Cargo @('build','--release','--all-features','--bin','m2c-pipeline','--example','m6_verify','--locked') 'build-release' | Out-Null
        # Subsequent Cargo invocations can replace the shared release binary with a
        # default-feature build. Snapshot the selected executables for this run.
        $bin=Join-Path $run 'bin'; New-Item -ItemType Directory -Path $bin | Out-Null
        $script:binary=Join-Path $bin 'm2c-pipeline.exe'; $script:verifier=Join-Path $bin 'm6_verify.exe'
        Copy-Item -LiteralPath (Join-Path $repo 'target/release/m2c-pipeline.exe') -Destination $script:binary
        Copy-Item -LiteralPath (Join-Path $repo 'target/release/examples/m6_verify.exe') -Destination $script:verifier
        $environment.binary_sha256=(Get-FileHash $script:binary).Hash; $environment.verifier_sha256=(Get-FileHash $script:verifier).Hash
        Save-Json (Join-Path $run 'environment.json') $environment
        if ($Mode -eq 'Demo') {
            $data=Join-Path $run 'input.bin'; Make-Dataset $data 3
            $single=Join-Path $run 'single.parquet'; Run-Conversion 'convert' $data $single 2 | Out-Null
            Verify-Output 'm3' $data $single 3 2
            $parts=Join-Path $run 'job'; Run-Conversion 'convert-parts' $data $parts 2 | Out-Null
            Verify-Output 'm4' $data $parts 3 2
            $before=@(Get-ChildItem -LiteralPath $parts -Recurse -File | Get-FileHash | ForEach-Object { $_.Hash })
            Run-Conversion 'convert-parts' $data $parts 2 -Resume | Out-Null
            $after=@(Get-ChildItem -LiteralPath $parts -Recurse -File | Get-FileHash | ForEach-Object { $_.Hash })
            if (Compare-Object $before $after) {throw 'Resume rewrote committed artifacts'}
            Cargo @('test','--lib','recovery::tests::process_interruption_matrix_converges_and_preserves_every_committed_part','--','--exact','--nocapture') 'crash-recovery-demo' | Out-Null
            $keys=Join-Path $run 'keys'
            Invoke-Recorded -Program $script:binary -Arguments @('keygen','--output-dir',$keys,'--report-json') -Label 'keygen' | Out-Null
            $envelope=Join-Path $run 'protected.m5'; $recovered=Join-Path $run 'recovered.parquet'
            Invoke-Recorded -Program $script:binary -Arguments @('protect','--input',$single,'--public-key',(Join-Path $keys 'public.key'),'--output',$envelope,'--report-json') -Label 'protect' | Out-Null
            Invoke-Recorded -Program $script:binary -Arguments @('unprotect','--input',$envelope,'--secret-key',(Join-Path $keys 'secret.key'),'--output',$recovered,'--report-json') -Label 'unprotect' | Out-Null
            Verify-Output 'roundtrip' $single $recovered 0 0
            $envelopeHash=(Get-FileHash -LiteralPath $envelope).Hash
            $recoveredHash=(Get-FileHash -LiteralPath $recovered).Hash
            $tampered=Join-Path $run 'tampered.m5'; $bad=[IO.File]::ReadAllBytes($envelope); $bad[$bad.Length-1]=$bad[$bad.Length-1] -bxor 1; [IO.File]::WriteAllBytes($tampered,$bad)
            $rejected=Join-Path $run 'must-not-exist'
            $failure=Invoke-Recorded -Program $script:binary -Arguments @('unprotect','--input',$tampered,'--secret-key',(Join-Path $keys 'secret.key'),'--output',$rejected,'--report-json') -Label 'tamper' -AllowFailure
            if ($failure.exit_code -eq 0 -or (Test-Path -LiteralPath $rejected)) {throw 'Tamper was accepted'}
            $collision=Invoke-Recorded -Program $script:binary -Arguments @('unprotect','--input',$envelope,'--secret-key',(Join-Path $keys 'secret.key'),'--output',$recovered,'--report-json') -Label 'no-clobber' -AllowFailure
            if ($collision.exit_code -eq 0 -or (Get-FileHash -LiteralPath $recovered).Hash -ne $recoveredHash -or (Get-FileHash -LiteralPath $envelope).Hash -ne $envelopeHash) {throw 'Existing artifacts changed'}
            Remove-Owned $keys
        } else {
            $microProfile=if ($RunProfile -eq 'Smoke') {'smoke'} else {'full'}
            if ($RunProfile -ne 'Stress') { Cargo @('bench','--bench','m6','--','--profile',$microProfile) 'microbench' | Out-Null }
            $scenarios=if ($RunProfile -eq 'Smoke') { @(@{rows=3000;batch=256;kind='convert'},@{rows=3000;batch=256;kind='convert-parts'}) }
                elseif ($RunProfile -eq 'Stress') { @(@{rows=3000000;batch=256;kind='convert-parts'}) }
                else { @(
                    foreach ($rows in @(300000,3000000)) { foreach ($batch in @(256,4096,65536)) { @{rows=$rows;batch=$batch;kind='convert'} } }
                    foreach ($batch in @(256,4096,65536)) { @{rows=300000;batch=$batch;kind='convert-parts'} }
                    @{rows=3000000;batch=65536;kind='convert-parts'}
                ) }
            $count=if ($RunProfile -eq 'Smoke') {3} else {7}
            foreach ($scenario in $scenarios) {
                $data=Join-Path $run ('input-'+[guid]::NewGuid().ToString('N')); Make-Dataset $data $scenario.rows
                Save-Json ($data+'.json') @{sha256=(Get-FileHash $data).Hash;records=$scenario.rows;batch=$scenario.batch;estimated_parts=[Math]::Ceiling($scenario.rows/$scenario.batch);free_bytes=$drive.AvailableFreeSpace}
                for ($sample=0;$sample -le $count;$sample++) {
                    $dest=Join-Path $run ('output-'+[guid]::NewGuid().ToString('N'))
                    $result=Run-Conversion $scenario.kind $data $dest $scenario.batch
                    $kind=if ($scenario.kind -eq 'convert') {'m3'} else {'m4'}
                    Verify-Output $kind $data $dest $scenario.rows $scenario.batch
                    Record-Sample $result $scenario.kind $scenario.rows ($scenario.rows*35) $scenario.batch $sample
                    if ($kind -eq 'm4') {
                        $resumeResult=Run-Conversion $scenario.kind $data $dest $scenario.batch -Resume
                        Verify-Output $kind $data $dest $scenario.rows $scenario.batch
                        Record-Sample $resumeResult 'resume-validation' $scenario.rows ($scenario.rows*35) $scenario.batch $sample
                    }
                    Remove-Owned $dest
                }
                Remove-Owned $data
            }
            if ($RunProfile -ne 'Stress') {
                $keys=Join-Path $run 'keys'; Invoke-Recorded -Program $script:binary -Arguments @('keygen','--output-dir',$keys) -Label 'keygen-setup' | Out-Null
                $sizes=if ($RunProfile -eq 'Smoke') {@(1MB)} else {@(1MB,64MB)}
                foreach ($size in $sizes) {
                    $data=Join-Path $run "payload-$size"; $file=[IO.File]::Open($data,[IO.FileMode]::CreateNew); $buffer=[byte[]]::new(1MB); [Array]::Fill[byte]($buffer,0x42)
                    try { for ($written=0;$written -lt $size;$written+=$buffer.Length) {$file.Write($buffer,0,$buffer.Length)} } finally {$file.Dispose()}
                    Save-Json ($data+'.json') @{bytes=$size;sha256=(Get-FileHash $data).Hash}
                    for ($sample=0;$sample -le $count;$sample++) {
                        $enc=Join-Path $run "enc-$size-$sample"; $plain=Join-Path $run "plain-$size-$sample"
                        $protected=Invoke-Recorded -Program $script:binary -Arguments @('protect','--input',$data,'--public-key',(Join-Path $keys 'public.key'),'--output',$enc,'--report-json') -Label 'protect-bench'
                        $unprotected=Invoke-Recorded -Program $script:binary -Arguments @('unprotect','--input',$enc,'--secret-key',(Join-Path $keys 'secret.key'),'--output',$plain,'--report-json') -Label 'unprotect-bench'
                        Verify-Output 'roundtrip' $data $plain 0 0
                        Record-Sample $protected 'protect' 0 $size 0 $sample; Record-Sample $unprotected 'unprotect' 0 $size 0 $sample
                        Remove-Owned $enc; Remove-Owned $plain
                    }
                    Remove-Owned $data
                }
                Remove-Owned $keys
            }
            $summary=@($script:samples | Where-Object {-not $_.warmup} | Group-Object operation,records,input_bytes,batch_records | ForEach-Object {
                $times=@($_.Group.elapsed_ms | Sort-Object); [pscustomobject]@{scenario=$_.Name;samples=$times.Count;min_ms=$times[0];median_ms=$times[[int][Math]::Floor($times.Count/2)];max_ms=$times[-1]}
            })
            Save-Json (Join-Path $run 'summary.json') $summary
        }
    }
    $outcome='PASS'
} catch {
    [IO.File]::WriteAllText((Join-Path $run 'failure.txt'),($_ | Out-String)); Write-Warning $_
} finally {
    Save-Json (Join-Path $run 'result.json') @{status=$outcome;mode=$Mode;profile=$RunProfile;commands=$script:commands.Count;gate_failures=$script:gateFailures;skips=@($script:commands | Where-Object {$_.skipped_lines.Count -gt 0})}
    Write-Output "M6 $Mode/$RunProfile $outcome evidence: $run"
}
if ($outcome -ne 'PASS') {exit 1}
