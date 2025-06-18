# Vesting Parameters (V2)
| Investment Bracket    | Vesting Structure     | TGE Unlock | Cliff Period     | Notes                                                               |
|-----------------------|-----------------------|------------|------------------|---------------------------------------------------------------------|
|0 - 500,000            | 4-Month Linear (X2)   | 5%         | No Cliff         | Full unlock on listing to encourage retail participation & adoption |
|500,000 - 1,000,000    | 6-Month Linear (X3)   | 5%         | No Cliff         | Gradual release over 6 months to reduce dump risk                   |
|1,000,000+             | 12-Month Linear (X6)  | 5%         | No Cliff         | Long-term investor commitment with reduced early unlocks            |

# Deploying Guide
- Change working directory to project repo and pull repo.
  ```
  ls
  cd <REPO_DIR_NAME>
  git pull origin main
  ```
- Revoke current programId (Optional - This deploys new contract)
  - `rm ./target/deploy/private_vesting-keypair.json`
- Sync program address
    - show programId
      - `anchor keys list`
    - sync programId
      - `anchor keys sync`
- Build smart contract
  - `anchor build`
- Change id.json file(Should keep in secret)
  - `nano ~/.config/solana/id.json`
- Set RPC url to helius (Optional)
  - `solana config set -u <HELIUS_RPC_URL>` (either devnet or mainnet)
- Deploy
  - `solana program deploy ./target/deploy/private_vesting.so`

## ***Workaround for failed deploy***
- In devnet
  - Change temporarily rpc url: `solana config set --url https://rpc.ankr.com/solana_devnet`
  - Close buffers: `solana program close --buffers`
  - Change RPC to helius again: `solana config set --url <HELIUS_RPC_URL>`
  - Deploy again: `solana program deploy ./target/deploy/private_vesting.so`
- In mainnet
  > In mainnet, there is no need to change rpc url temporarily.
  - Close buffers: `solana program close --buffers`
  - Deploy again: `solana program deploy ./target/deploy/private_vesting.so`
# FAQ
- Why closing buffers?
  - Because every deployment would reduce SOL balance, we don't have to waste SOL even if deployment has been failed.
  - If you close buffer, you can get SOL locked in buffer account into your wallet.
- Why deployment fails?
  - It's not due to smart contract code itself.
  - Because of solana network congestion, deployment failes sometimes in the peak timezone or when the transactions are actively sent to network in bulk.
# Migrate to V2: Changelog
- price changed from `$0.025` to `$0.01`
- TGE unlock: `5%`
- cliff: `0` (Removed all cliffs)
- Tier parameters updated. See [Vesting Parameters](#vesting-parameters-v2)
- MigrateV2 method added.
  - Set private sale allocation 30M
  - X1 duration set `2 months`, so as new vesting periods to be `4, 6, 12 months`. See [Vesting Parameters](#vesting-parameters-v2)
  - Return 170M to ADMIN_WALLET
# Migration Guide to V2
## Available command to upgrade authority.
```
solana program set-upgrade-authority <PROGRAM_ADDRESS> --skip-new-upgrade-authority-signer-check --upgrade-authority <UPGRADE_AUTHORITY_SIGNER> --new-upgrade-authority <NEW_UPGRADE_AUTHORITY>
```
## Upgrade authority
> Caution: Setting new upgrade authority requires careful manipulation. Follow guide strictly!
- OLD_ADMIN_WALLET should have a little SOL.
- Create NEW_ADMIN_WALLET and it should have SOL for tx fee.
- You should ensure having .json file of NEW_ADMIN_WALLET.
- Copy public address of NEW_ADMIN_WALLET into following command
  ```
  solana program set-upgrade-authority 2dn5aBpMLEXZXrhdq8CsNVsWt9Qe5B1uv1T7E6VYcydb --skip-new-upgrade-authority-signer-check --new-upgrade-authority <NEW_UPGRADE_AUTHORITY>
  ```
- Change id.json file with NEW_ADMIN_WALLET. (Should keep in secret)
  ```
  nano ~/.config/solana/id.json
  ```
## Deploy smart contract
- Pull changes from github repo.
- Sync keys
  ```
  anchor keys list
  anchor keys sync
  ```
- Build
  - `anchor build`
- Deploy
  - `solana program deploy ./target/deploy/private_vesting.so`