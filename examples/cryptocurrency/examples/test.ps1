#!/usr/bin/env pwsh

#
# PowerShell script for testing the cryptocurrency demo.
#

# Base URL for demo service endpoints
$BASE_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency/v1';
$TRANSACTION_URL = 'http://127.0.0.1:8000/api/explorer/v1/transactions';
# Directory with the current script
$wd = $myinvocation.mycommand.path | Split-Path;

# Creates a wallet using a transaction stored in the specified file.
function Create-Wallet ($jsonFilename) {
  $body = cat $jsonFilename;
  $resp = Invoke-WebRequest "$TRANSACTION_URL" `
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
  $resp = Invoke-WebRequest "$TRANSACTION_URL" `
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
  $resp = Invoke-WebRequest "http://127.0.0.1:8000/api/explorer/v1/transactions?hash=$($tx.hash)";
  $error = $False;
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
      hash = '75a9d95694f22823ae01a6feafb3d4e27b55b83bd6897aa581456ea5da382dde';
    },
    @{
      name = 'Bob';
      json = "$wd/create-wallet-2.json";
      hash = '7a09053aa590704332b7a18f552150caa8b6e4f777afa4005d169038f481b7f7';
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
  $transferHash = 'ae3afbe35f1bfd102daea2f3f72884f04784a10aabe9d726749b1188a6b9fe9b';
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
  $pubkey = '114e49a764813f2e92609d103d90f23dc5b7e94e74b3e08134c1272441614bd9';
  $resp = (Invoke-WebRequest "$BASE_URL/wallet?pub_key=$pubkey").Content | ConvertFrom-Json;
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
