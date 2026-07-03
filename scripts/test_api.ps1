$ErrorActionPreference = "Stop"
$Base = "http://localhost:3000"
$GroupId = "4d2c1954-c678-4967-a3b4-02154d567760"
$MpesaSecret = "test-mpesa-secret"

function Test-Endpoint {
    param(
        [string]$Name,
        [string]$Method,
        [string]$Path,
        [object]$Body = $null,
        [hashtable]$Headers = @{},
        [int]$ExpectedStatus = 200
    )

    $uri = "$Base$Path"
    $params = @{
        Uri = $uri
        Method = $Method
        Headers = $Headers
    }
    if ($Body) {
        $params.Body = ($Body | ConvertTo-Json -Depth 10 -Compress)
        $params.ContentType = "application/json"
    }

    try {
        $response = Invoke-WebRequest @params -UseBasicParsing
        $status = $response.StatusCode
        $content = $response.Content
    } catch {
        $status = [int]$_.Exception.Response.StatusCode
        $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
        $content = $reader.ReadToEnd()
    }

    $ok = $status -eq $ExpectedStatus
    $icon = if ($ok) { "PASS" } else { "FAIL" }
    Write-Host "[$icon] $Name -> $status (expected $ExpectedStatus)"
    if (-not $ok) { Write-Host "  Response: $content" }
    return @{ Ok = $ok; Status = $status; Content = $content }
}

function Get-Json($result) {
    return $result.Content | ConvertFrom-Json
}

Write-Host "`n=== Community Savings API Tests ===`n"

# Health
$r = Test-Endpoint -Name "Ping" -Method GET -Path "/ping"
if ($r.Content -ne "pong") { Write-Host "  FAIL: expected pong, got $($r.Content)" }

Test-Endpoint -Name "Health" -Method GET -Path "/health" | Out-Null

$Suffix = (Get-Date -Format "HHmmss")

# Members
$m1Body = @{
    group_id = $GroupId
    full_name = "Alice Member"
    phone_number = "2547123$Suffix"
}
$m1 = Get-Json (Test-Endpoint -Name "Create member 1" -Method POST -Path "/api/members" -Body $m1Body -ExpectedStatus 200)
$Member1Id = $m1.id

$m2Body = @{
    group_id = $GroupId
    full_name = "Bob Guarantor"
    phone_number = "2547987$Suffix"
}
$m2 = Get-Json (Test-Endpoint -Name "Create member 2" -Method POST -Path "/api/members" -Body $m2Body -ExpectedStatus 200)
$Member2Id = $m2.id

Test-Endpoint -Name "List members" -Method GET -Path "/api/members" | Out-Null
Test-Endpoint -Name "Get member" -Method GET -Path "/api/members/$Member1Id" | Out-Null

Test-Endpoint -Name "Update member" -Method PATCH -Path "/api/members/$Member1Id" -Body @{ full_name = "Alice Updated" } | Out-Null

# Attendance
$attPresent = @{
    group_id = $GroupId
    meeting_date = "2026-08-01"
    status = "present"
}
Test-Endpoint -Name "Attendance present" -Method POST -Path "/api/members/$Member1Id/attendance" -Body $attPresent -ExpectedStatus 200 | Out-Null

$attAbsent = @{
    group_id = $GroupId
    meeting_date = "2026-08-02"
    status = "absent"
}
Test-Endpoint -Name "Attendance absent (auto-fine)" -Method POST -Path "/api/members/$Member1Id/attendance" -Body $attAbsent -ExpectedStatus 200 | Out-Null

Test-Endpoint -Name "List attendance" -Method GET -Path "/api/members/$Member1Id/attendance?group_id=$GroupId" | Out-Null

# Transactions
$deposit = @{
    group_id = $GroupId
    member_id = $Member1Id
    amount = 50000
    tx_type = "deposit"
    reference = "test deposit"
}
$tx = Get-Json (Test-Endpoint -Name "Append deposit" -Method POST -Path "/api/transactions" -Body $deposit -ExpectedStatus 200)
$TxId = $tx.id

Test-Endpoint -Name "List transactions" -Method GET -Path "/api/transactions?group_id=$GroupId" | Out-Null
Test-Endpoint -Name "Get transaction" -Method GET -Path "/api/transactions/$TxId" | Out-Null

# Loans
$loanReq = @{
    group_id = $GroupId
    member_id = $Member1Id
    principal = 100000
    term_months = 12
}
$loan = Get-Json (Test-Endpoint -Name "Request loan" -Method POST -Path "/api/loans" -Body $loanReq -ExpectedStatus 200)
$LoanId = $loan.id

Test-Endpoint -Name "Add guarantor" -Method POST -Path "/api/loans/$LoanId/guarantors" -Body @{ member_id = $Member2Id } -ExpectedStatus 200 | Out-Null
Test-Endpoint -Name "List guarantors" -Method GET -Path "/api/loans/$LoanId/guarantors" | Out-Null
Test-Endpoint -Name "Approve loan" -Method POST -Path "/api/loans/$LoanId/approve" -ExpectedStatus 200 | Out-Null
Test-Endpoint -Name "Loan schedule" -Method GET -Path "/api/loans/$LoanId/schedule" | Out-Null
$disbursed = Get-Json (Test-Endpoint -Name "Disburse loan" -Method POST -Path "/api/loans/$LoanId/disburse" -ExpectedStatus 200)
if ($disbursed.status -ne "disbursed") { Write-Host "  FAIL: loan not disbursed" }

# Penalties
Test-Endpoint -Name "List penalties" -Method GET -Path "/api/penalties" | Out-Null
$penCalc = @{
    loan_id = $LoanId
    overdue_days = 7
}
Test-Endpoint -Name "Calculate penalty" -Method POST -Path "/api/penalties/calculate" -Body $penCalc -ExpectedStatus 200 | Out-Null

# M-Pesa callback
$mpesaPayload = @{
    transaction_id = "TEST-TX-001"
    phone_number = "254712345678"
    member_id = $Member1Id
    group_id = $GroupId
    amount = 25000
    result_code = 0
    result_desc = "Success"
}
# Must match serde_json field order from MpesaCallbackRequest struct
$mpesaJson = '{"transaction_id":"TEST-TX-001","phone_number":"254712345678","member_id":"' + $Member1Id + '","group_id":"' + $GroupId + '","amount":25000,"result_code":0,"result_desc":"Success"}'
$hmac = New-Object System.Security.Cryptography.HMACSHA256
$hmac.Key = [Text.Encoding]::UTF8.GetBytes($MpesaSecret)
$sigBytes = $hmac.ComputeHash([Text.Encoding]::UTF8.GetBytes($mpesaJson))
$signature = -join ($sigBytes | ForEach-Object { $_.ToString("x2") })

try {
    $mpesaResp = Invoke-WebRequest -Uri "$Base/api/mpesa/callback" -Method POST `
        -Body $mpesaJson -ContentType "application/json" `
        -Headers @{ "x-mpesa-signature" = $signature } -UseBasicParsing
    Write-Host "[PASS] M-Pesa callback -> $($mpesaResp.StatusCode)"
} catch {
    $status = [int]$_.Exception.Response.StatusCode
    $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
    $content = $reader.ReadToEnd()
    Write-Host "[FAIL] M-Pesa callback -> $status"
    Write-Host "  Response: $content"
}

Write-Host "`n=== Tests complete ===`n"
