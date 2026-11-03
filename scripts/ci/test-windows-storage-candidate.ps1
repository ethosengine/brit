param([Parameter(Mandatory = $true)][string] $Target)
$ErrorActionPreference = 'Stop'
$env:CARGO_TARGET_DIR = Join-Path $env:RUNNER_TEMP 'eprfs-storage-candidate-target'
$fixture = (Resolve-Path 'scripts/ci/fixtures/windows-storage-candidate').Path
$evidence = Join-Path $env:GITHUB_WORKSPACE 'rust-probe-evidence'
New-Item -ItemType Directory $evidence | Out-Null
$receiptPath = Join-Path $fixture 'candidate.json'
$receipt = Get-Content -Raw $receiptPath | ConvertFrom-Json
Copy-Item $receiptPath $evidence
$actual = @(Get-ChildItem -Recurse -File -Force $fixture | Where-Object { $_.FullName -ne $receiptPath })
if ($actual.Count -ne $receipt.files.Count) { throw 'Candidate file inventory differs from receipt.' }
foreach ($file in $receipt.files) {
    $path = Join-Path $fixture $file.path
    if ((Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant() -ne $file.sha256) {
        throw "Candidate bytes changed: $($file.path)"
    }
}
if ($receipt.storageVersion -ne '0.3.4' -or $receipt.rustToolchain -ne '1.96.1') {
    throw 'Unexpected candidate or toolchain.'
}
$toolchain = '1.96.1-' + $Target
& rustup toolchain install $toolchain --profile minimal --no-self-update
if ($LASTEXITCODE -ne 0) { throw 'Pinned Rust installation failed.' }
& rustc "+$toolchain" -vV | Set-Content (Join-Path $evidence 'rustc.txt')
if ($LASTEXITCODE -ne 0) { throw 'Pinned Rust inspection failed.' }
$messages = Join-Path $evidence 'cargo-build.jsonl'
Push-Location (Join-Path $fixture 'elohim/eprfs')
try {
    & cargo "+$toolchain" test --locked -p eprfs-storage --all-targets --target $Target --no-run --message-format=json 2> (Join-Path $evidence 'cargo-build.stderr') | Tee-Object -FilePath $messages
    if ($LASTEXITCODE -ne 0) { throw 'Production test compilation failed.' }
} finally { Pop-Location }
$artifacts = @(Get-Content $messages | ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { $_.reason -eq 'compiler-artifact' -and $_.profile.test -and $_.executable })
$expected = @('eprfs_storage', 'boundary', 'tier_chain', 'walk_directory_storage')
if (@(Compare-Object $expected @($artifacts.target.name)).Count -ne 0) {
    throw 'Compiled target inventory differs from the complete storage test suite.'
}
$probeUser = 'BritRust' + [Guid]::NewGuid().ToString('N').Substring(0, 8)
$password = [Guid]::NewGuid().ToString('N') + 'aA1!'
Write-Output "::add-mask::$password"
$secure = ConvertTo-SecureString $password -AsPlainText -Force
$probeDir = Join-Path $env:PUBLIC ('brit-rust-tests-' + [Guid]::NewGuid().ToString('N'))
$created = $false
try {
    New-LocalUser -Name $probeUser -Password $secure -AccountNeverExpires | Out-Null
    $created = $true
    Add-LocalGroupMember -Group (Get-LocalGroup -SID 'S-1-5-32-545') -Member $probeUser
    New-Item -ItemType Directory (Join-Path $probeDir 'bin') | Out-Null
    $targets = @()
    foreach ($artifact in $artifacts) {
        $filename = [IO.Path]::GetFileName($artifact.executable)
        Copy-Item -LiteralPath $artifact.executable -Destination (Join-Path $probeDir "bin/$filename")
        $targets += [pscustomobject]@{
            name = $artifact.target.name; executable = "bin/$filename"
            sha256 = (Get-FileHash -Algorithm SHA256 $artifact.executable).Hash.ToLowerInvariant()
        }
    }
    @{ targets = $targets; targetTriple = $Target; rustToolchain = $toolchain } |
        ConvertTo-Json -Depth 6 | Set-Content (Join-Path $probeDir 'launch.json') -Encoding utf8
    Copy-Item $receiptPath $probeDir
    Copy-Item scripts/ci/run-windows-storage-tests.ps1 $probeDir
    $grant = '{0}\{1}:(OI)(CI)F' -f $env:COMPUTERNAME, $probeUser
    & icacls.exe $probeDir /grant $grant | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Fixture ACL grant failed.' }
    $credential = [PSCredential]::new("$env:COMPUTERNAME\$probeUser", $secure)
    $script = Join-Path $probeDir 'run-windows-storage-tests.ps1'
    $saved = @{}
    foreach ($key in @('TEMP', 'TMP', 'CARGO_REGISTRIES_ELOHIM_TOKEN')) {
        $saved[$key] = [Environment]::GetEnvironmentVariable($key)
    }
    try {
        $env:TEMP = $probeDir
        $env:TMP = $probeDir
        Remove-Item Env:CARGO_REGISTRIES_ELOHIM_TOKEN -ErrorAction SilentlyContinue
        $process = Start-Process -FilePath (Get-Command pwsh).Source -Credential $credential -LoadUserProfile -WorkingDirectory $probeDir -Wait -PassThru -ArgumentList @('-NoProfile', '-File', "`"$script`"", '-ProbeDirectory', "`"$probeDir`"")
    } finally {
        foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key]) }
    }
    $result = Get-Content -Raw (Join-Path $probeDir 'rust-probe-result.json') | ConvertFrom-Json
    if ($result.userSid -ne (Get-LocalUser -Name $probeUser).SID.Value -or $result.elevatedAdministrator) {
        throw 'Test result does not identify the prepared standard user.'
    }
    if ($process.ExitCode -ne 0 -or -not $result.allProductionRustTestsPassed) {
        throw 'Production Windows test run failed; inspect rust-probe-evidence.'
    }
} finally {
    if (Test-Path $probeDir) {
        Get-ChildItem -File $probeDir | Copy-Item -Destination $evidence
    }
    if ($created) { Remove-LocalUser -Name $probeUser }
}
