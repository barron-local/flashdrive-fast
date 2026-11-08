$sourceFile = "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src\sample_200mb.dat"
$targetDir = "F:\_BENCHMARK_TEST_TMP"
$benchExe = "c:\Users\Woradech\Desktop\flashdrive-fast\target\release\bench.exe"

Write-Host "=========================================================="
Write-Host "Benchmark: PC -> Flash Drive F: (200 MB)"
Write-Host "=========================================================="

# 1. Standard Windows Copy
$stdTarget = Join-Path $targetDir "standard_copy.dat"
if (Test-Path $stdTarget) { Remove-Item $stdTarget -Force }

$sw1 = [System.Diagnostics.Stopwatch]::StartNew()
[System.IO.File]::Copy($sourceFile, $stdTarget, $true)
$stream = [System.IO.File]::Open($stdTarget, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
$stream.Flush($true)
$stream.Close()
$sw1.Stop()

$stdSec = $sw1.Elapsed.TotalSeconds
$stdSpeed = 200.0 / $stdSec
Write-Host "[1] Windows Standard Copy (PC -> F:):"
Write-Host ("    Time: {0:N3} sec" -f $stdSec)
Write-Host ("    Speed: {0:N2} MB/s" -f $stdSpeed)
Write-Host ""

# 2. Rust Fast Engine
$fastTarget = Join-Path $targetDir "fast_copy.dat"
if (Test-Path $fastTarget) { Remove-Item $fastTarget -Force }

Write-Host "[2] Flashdrive-Fast Engine (PC -> F:):"
$rustOutput = & $benchExe $sourceFile $targetDir
Write-Host ("    " + $rustOutput)

Write-Host ""
Write-Host "=========================================================="
Write-Host "Benchmark: Flash Drive F: -> PC (Readback 200 MB)"
Write-Host "=========================================================="

# 3. Readback Standard
$readBackStd = "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src\readback_std.dat"
if (Test-Path $readBackStd) { Remove-Item $readBackStd -Force }

$sw3 = [System.Diagnostics.Stopwatch]::StartNew()
[System.IO.File]::Copy($stdTarget, $readBackStd, $true)
$sw3.Stop()
$readStdSec = $sw3.Elapsed.TotalSeconds
$readStdSpeed = 200.0 / $readStdSec
Write-Host "[3] Windows Standard Copy (F: -> PC):"
Write-Host ("    Time: {0:N3} sec" -f $readStdSec)
Write-Host ("    Speed: {0:N2} MB/s" -f $readStdSpeed)
Write-Host ""

# 4. Readback Rust Engine
$readBackFast = "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src\readback_fast.dat"
if (Test-Path $readBackFast) { Remove-Item $readBackFast -Force }

Write-Host "[4] Flashdrive-Fast Engine (F: -> PC):"
$rustReadOutput = & $benchExe $stdTarget "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src"
Write-Host ("    " + $rustReadOutput)

# Clean up temporary test files
Write-Host ""
Write-Host "Cleaning up temporary test files..."
if (Test-Path $targetDir) { Remove-Item -Recurse -Force $targetDir }
if (Test-Path "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src") { Remove-Item -Recurse -Force "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src" }
Write-Host "Done! No original files on Drive F: were modified."
