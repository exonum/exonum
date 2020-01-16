#!/usr/bin/env pwsh

#
# PowerShell script for testing the cryptocurrency demo.
#

# Base URL for demo service endpoints
$BASE_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency/v1';
$TRANSACTION_URL = 'http://127.0.0.1:8000/api/explorer/v1/transactions';
# Directory with the current script
$wd = $myinvocation.mycommand.path | Split-Path;

# Sends a transaction stored in the specified file.
function Send-Tx ($jsonFilename) {
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
  $resp = Invoke-WebRequest "$($TRANSACTION_URL)?hash=$($tx.hash)";
  $error = $False;
  if ($resp.StatusCode -eq 200) {
    $respJson = $resp.Content | ConvertFrom-Json;
    if ($respJson.type -ne 'committed') {
      $error = $True;
    }
  } else {
    $error = $True;
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
      hash = 'abe9ac1eef23b4cda7fc408ce488b233c3446331ac0f8195b7d21a210908b447';
    },
    @{
      name = 'Bob';
      json = "$wd/create-wallet-2.json";
      hash = '59198ccaba93d0dcf2081f3820e54e5233d7eaf223f13c147df88ccfc351ac27';
    }
  );

  foreach ($tx in $txs) {
    echo "Creating wallet for $($tx.name)";
    $hash = Send-Tx $tx.json;
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
  $transferHash = 'b5d68015cb47f1b1f909e7667c219f1c63a0b7c978cdd6e8ffc279d05ba66fec';
  $hash = Send-Tx("$wd/transfer-funds.json");
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
  Check-Wallet $resp[0] 'Alice' '95';
  Check-Wallet $resp[1] 'Bob' '105';
  echo "Retrieving info on Alice's wallet...";
  $pubkey = '070122b6eb3f63a14b25aacd7a1922c418025e04b1be9d1febdfdbcf67615799';
  $resp = (Invoke-WebRequest "$BASE_URL/wallet?pub_key=$pubkey").Content | ConvertFrom-Json;
  Check-Wallet $resp 'Alice' '95';
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
