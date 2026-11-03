param([Parameter(Mandatory = $true)][string] $ProbeDirectory)
$ErrorActionPreference = 'Stop'
$reportPath = Join-Path $ProbeDirectory 'rust-probe-result.json'
$results = [Collections.Generic.List[object]]::new()
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$failure = $null
try {
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Rust tests must run as a standard user.'
    }
    $launch = Get-Content -Raw (Join-Path $ProbeDirectory 'launch.json') | ConvertFrom-Json
    $allTests = [Collections.Generic.List[string]]::new()
    foreach ($target in $launch.targets) {
        $executable = Join-Path $ProbeDirectory $target.executable
        if ((Get-FileHash -Algorithm SHA256 $executable).Hash.ToLowerInvariant() -ne $target.sha256) {
            throw "Test executable hash changed: $($target.name)"
        }
        $listing = @(& $executable --list --format terse 2>&1)
        if ($LASTEXITCODE -ne 0) { throw "Could not enumerate $($target.name): $listing" }
        $tests = @($listing | Where-Object { $_ -match ': test$' })
        if ($tests.Count -eq 0) { throw "No tests in $($target.name)." }
        foreach ($test in $tests) { $allTests.Add([string]$test) }
        $log = Join-Path $ProbeDirectory ($target.name + '.log')
        # No name filter: execute the complete libtest target, including ignored tests.
        & $executable --include-ignored --test-threads=1 2>&1 | Tee-Object -FilePath $log
        $code = $LASTEXITCODE
        $results.Add([pscustomobject]@{
            name = $target.name; executableSha256 = $target.sha256
            listedTests = $tests; exitCode = $code
        })
        if ($code -ne 0) { throw "Production Rust tests failed: $($target.name), exit $code" }
    }
    foreach ($required in @(
        'durability_tests::readonly_object_can_be_fetched_but_cannot_claim_fresh_custody: test',
        'durability_tests::every_object_write_sync_and_promotion_failure_refuses_without_losing_candidates: test',
        'windows::tests::filesystems_without_the_directory_flush_contract_refuse: test'
    )) {
        if ($required -notin $allTests) { throw "Required Windows/durability test missing: $required" }
    }
    if ($results.Count -ne $launch.targets.Count) { throw 'Some compiled test targets were not run.' }
} catch {
    $failure = $_.Exception.ToString()
} finally {
    [ordered]@{
        schema = 'eprfs-windows-rust-tests/v1'
        utc = [DateTime]::UtcNow.ToString('o')
        user = $identity.Name
        userSid = $identity.User.Value
        elevatedAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
        processArchitecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
        targetTriple = $launch.targetTriple
        rustToolchain = $launch.rustToolchain
        sourceReceiptSha256 = (Get-FileHash -Algorithm SHA256 (Join-Path $ProbeDirectory 'candidate.json')).Hash.ToLowerInvariant()
        productionRustTestsRun = $results.Count -gt 0
        allProductionRustTestsPassed = $null -eq $failure
        physicalPowerLossTested = $false
        error = $failure
        results = @($results.ToArray())
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $reportPath -Encoding utf8
}
if ($null -ne $failure) { exit 1 }
