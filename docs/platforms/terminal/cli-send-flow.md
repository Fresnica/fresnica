# Fresnica CLI Send Flow

Example:

```
fresnica send 100 XLM to GABC...
```

Flow:

```
Command
  |
  v
WalletManager
  |
  v
Wallet state check
  |
  +-- watch-only -> reject
  +-- locked -> request unlock
  |
  v
Availability check
  |
  v
Build transaction
  |
  v
Human review
  |
  v
Sign
  |
  v
Submit
  |
  v
Show transaction hash
```

Example review:

```
You (main wallet, GABC...DBCA)
will transfer:

100 XLM

to:
Alice
GXYZ...ABCD

Confirm? Y/N
```

The CLI should present blockchain operations as human understandable actions.
