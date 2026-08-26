# Application Capability References

This directory contains the detailed contracts and supporting references for Fresnica Application Capabilities.

The canonical vocabulary, maturity rules and governance model are defined in [`../application-capabilities.md`](../application-capabilities.md). A document here may describe either:

- a **Normative** cross-platform semantic contract;
- a **Defined** capability whose boundary is agreed but detailed APIs remain platform-specific; or
- an experimental/provider reference that must not be mistaken for a normative capability.

## Shared domain vocabulary

- [Domain primitives](domain-primitives.md) defines network/account/signer/asset/amount/price semantics reused across multiple capabilities.

## Capability index

| ID | Capability | Maturity | Reference |
| --- | --- | --- | --- |
| `account` | Account | Normative | [account.md](account.md) |
| `signer` | Signer | Normative | [signer.md](signer.md) |
| `wallet` | Wallet | Defined | [wallet.md](wallet.md) |
| `balance` | Balance / Availability | Normative | [balance.md](balance.md) |
| `payment` | Payment | Normative | [payment.md](payment.md) |
| `transaction` | Transaction | Normative | [transaction.md](transaction.md) |
| `trustline` | Trustline | Normative | [trustline.md](trustline.md) |
| `history` | History / Activity | Defined | [history.md](history.md) |
| `contacts` | Contacts / Destination Resolution | Defined | [contacts.md](contacts.md) |
| `sdex` | SDEX | Normative | [sdex.md](sdex.md) |
| `anchor` | Anchor | Normative | [anchor.md](anchor.md) |
| `signing` | Signing Coordination | Normative | [signing-coordination.md](signing-coordination.md) |
| `security` | Application Security | Defined | [application-security.md](application-security.md) |
| `dapp` | Dapp Interaction | Defined | [dapp.md](dapp.md) |
| `external-signer` | Hardware / External Signer Interaction | Defined | [external-signer.md](external-signer.md) |
| `network` | Network / Gateway | Defined | [network.md](network.md) |

## Experimental/provider references

- [Passkey / smart-account reference](passkey-smart-account.md) records the current Testnet/provider work. It is not a new universal software-signer model and is not itself a normative capability contract.

## Reading rule

Capability references define **wallet meaning**, not UI structure and not one required source implementation.

A platform may implement the same capability in Rust, TypeScript/JavaScript, Swift, Kotlin or another suitable language. Normative semantics and the [`Core Security Boundary`](../core-security-boundary.md) remain the compatibility boundary.
