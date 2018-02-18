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
  $resp = Invoke-WebRequest "http://127.0.0.1:8000/api/system/v1/transactions/$($tx.hash)";
  $error = False;
  if ($resp.StatusCode -eq 200) {
    $respJson = $resp.Content | ConvertFrom-Json;
    if (($respJson.type -ne 'Committed') -or ($respJson.content.body.name -ne $tx.name)) {
      $error = True;
    }
  } else {
    $error = True;
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
      name = 'Johnny Doe';
      json = "$wd/create-wallet-1.json";
      hash = '44c6c2c58eaab71f8d627d75ca72f244289bc84586a7fb42186a676b2ec4626b';
    },
    @{
      name = 'Janie Roe';
      json = "$wd/create-wallet-2.json";
      hash = '8714e90607afc05f43b82c475c883a484eecf2193df97b243b0d8630812863fd';
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

  echo 'Transferring tokens between Johnny and Janie...';
  $transferHash = 'e63b28caa07adffb6e2453390a59509a1469e66698c75b4cfb2f0ae7a6887fdc';
  $hash = Transfer("$wd/transfer-funds.json");
  if ($hash -ne $transferHash) {
    throw "Unexpected transaction hash: $hash";
  }

  echo 'Waiting a bit until transaction is committed...';
  sleep 5;

  echo 'Retrieving info on all wallets...';
  $resp = (Invoke-WebRequest "$BASE_URL/wallets").Content | ConvertFrom-Json;
  # Wallet records in the response are deterministically ordered by increasing
  # public key. As Johnny's pubkey is lexicographically lesser than Janie's, it is possible to
  # determine his wallet as .[0] and hers as .[1].
  Check-Wallet $resp[0] 'Johnny Doe' '90';
  Check-Wallet $resp[1] 'Janie Roe' '110';

  echo "Retrieving info on Johnny's wallet...";
  $pubkey = '03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
  $resp = (Invoke-WebRequest "$BASE_URL/wallet/$pubkey").Content | ConvertFrom-Json;
  Check-Wallet $resp 'Johnny Doe' '90';
}

Compile-Server;
$server = Start-Server;

# Wait until the service is started. As we have compiled the server previously,
# starting it shouldn't take long.
sleep 5;

try {
  Main;
} finally {
  kill $server.id;
}
