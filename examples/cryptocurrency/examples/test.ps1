#!/usr/bin/env pwsh

#
# PowerShell script for testing the cryptocurrency demo.
#

# Base URL for demo service endpoints
$BASE_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency/v1';
# Directory with the current script
$wd = $myinvocation.mycommand.path | Split-Path;

# Creates a wallet using a transaction stored in the specified file.
function Create-Wallet ($jsonFilename) {
  $body = cat $jsonFilename;
  $resp = Invoke-WebRequest "$BASE_URL/wallets" `
    -Method POST `
    -ContentType 'application/json' `
    -Body $body;

  if ($resp.StatusCode -eq 200) {
    return ($resp.Content | ConvertFrom-Json).tx_hash;
  } else {
    return '';
  }
}

# Performs a transfer using a transaction stored in the specified file.
function Transfer ($jsonFilename) {
  $body = cat $jsonFilename;
  $resp = Invoke-WebRequest "$BASE_URL/wallets/transfer" `
    -Method POST `
    -ContentType 'application/json' `
    -Body $body;

  if ($resp.StatusCode -eq 200) {
    return ($resp.Content | ConvertFrom-Json).tx_hash;
  } else {
    return '';
  }
}

# Checks that a `CreateWallet` transaction is committed to the blockchain.
function Check-CreateTx ($tx) {
  $resp = Invoke-WebRequest "http://127.0.0.1:8000/api/explorer/v1/transactions/$($tx.hash)";
  $error = false;
  if ($resp.StatusCode -eq 200) {
    $respJson = $resp.Content | ConvertFrom-Json;
    if (($respJson.type -ne 'Committed') -or ($respJson.content.body.name -ne $tx.name)) {
      $error = true;
    }
  } else {
    $error = true;
  }

  if ($error) {
    throw "Unexpected response: $($resp.Content)";
  } else {
    echo "OK, got expected TxCreateWallet for user $($tx.name)";
  }
}

# Checks the state of a wallet object.
function Check-Wallet ($wallet, $name, $balance) {
  if (($wallet.name -eq $name) -and ($wallet.balance -eq $balance)) {
    echo "OK, got expected transaction balance $balance for user $name";
  } else {
    throw "Unexpected wallet state: $wallet";
  }
}

function Compile-Server () {
  cargo build --example demo;
}

function Start-Server () {
  return (Start-Process cargo -PassThru -WorkingDirectory (pwd) -ArgumentList run,--example,demo);
}

function Main () {
  # Expected transaction hashes
  $txs = @(
    @{
      name = 'Alice';
      json = "$wd/create-wallet-1.json";
      hash = 'b45f18c71ae62479e90ee0fb1201bface4c4009f6aa759fe672fc367e1dd3a94';
    },
    @{
      name = 'Bob';
      json = "$wd/create-wallet-2.json";
      hash = '118d0b9377ab491544adbcbdd92bb186f072063a99e9c29e5790b51c070d2bc6';
    }
  );

  foreach ($tx in $txs) {
    echo "Creating wallet for $($tx.name)";
    $hash = Create-Wallet $tx.json;
    if ($hash -eq $tx.hash) {
      echo "OK, got expected transaction hash $($tx.hash)";
    } else {
      throw "Unexpected transaction hash: $hash";
    }
  }

  echo 'Waiting a bit until transactions are committed...';
  sleep 5;

  foreach ($tx in $txs) {
    echo "Checking that $($tx.name)'s transaction is committed";
    Check-CreateTx $tx;
  }

  echo 'Transferring tokens between Alice and Bob...';
  $transferHash = '45b8363b8c61a2aaebf6df2a52c1246a7eeac0e604f95b58d7c14177da581ae0';
  $hash = Transfer("$wd/transfer-funds.json");
  if ($hash -ne $transferHash) {
    throw "Unexpected transaction hash: $hash";
  }

  echo 'Waiting a bit until transaction is committed...';
  sleep 5;

  echo 'Retrieving info on all wallets...';
  $resp = (Invoke-WebRequest "$BASE_URL/wallets").Content | ConvertFrom-Json;
  # Wallet records in the response are deterministically ordered by increasing
  # public key. As Alice's pubkey is lexicographically lesser than Bob's, it is possible to
  # determine his wallet as .[0] and hers as .[1].
  Check-Wallet $resp[0] 'Alice' '85';
  Check-Wallet $resp[1] 'Bob' '115';

  echo "Retrieving info on Alice's wallet...";
  $pubkey = '3fc6dad512a26ddaefb24f1f4187dccb21c182a217cf7fdc356e02a008aba30c';
  $resp = (Invoke-WebRequest "$BASE_URL/wallet/$pubkey").Content | ConvertFrom-Json;
  Check-Wallet $resp 'Alice' '85';
}

Compile-Server;
$server = Start-Server;

# Wait until the service is started. As we have compiled the server previously,
# starting it shouldn't take long.
sleep 12;

try {
  Main;
} finally {
  kill $server.id;
}
