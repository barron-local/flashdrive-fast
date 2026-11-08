$tempDir = "C:\Users\Woradech\AppData\Local\Temp\flash_bench_src"
if (!(Test-Path $tempDir)) { New-Item -ItemType Directory -Path $tempDir -Force | Out-Null }
$filePath = Join-Path $tempDir "sample_200mb.dat"

if (!(Test-Path $filePath) -or ((Get-Item $filePath).Length -ne 209715200)) {
    $fs = [System.IO.File]::Create($filePath)
    $fs.SetLength(209715200) # 200MB
    $fs.Close()
}
Write-Output "File ready at $filePath"

$destDir = "F:\_BENCHMARK_TEST_TMP"
if (!(Test-Path $destDir)) { New-Item -ItemType Directory -Path $destDir -Force | Out-Null }
Write-Output "Target test folder ready at $destDir"
