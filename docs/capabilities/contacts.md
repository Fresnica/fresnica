# Contacts / Destination Resolution Capability

Maturity: **Defined**

## Purpose

Contacts / Destination Resolution provides product-friendly destination aliases and optional destination metadata used by Flows such as Send.

It is currently **Defined** because address-book schema, supported identity types, synchronization and product UX are expected to vary across platforms.

## Agreed boundary

A destination resolver may map a user-facing alias to semantic destination data such as:

- validated account address;
- optional memo/destination metadata;
- contact/display label.

Explicit Flow input must take precedence over stored defaults when the two represent the same field. For example, an explicitly supplied memo should not be silently replaced by a contact's default memo.

## Security and identity

Destination resolution must not bypass Account/Payment validation. A contact entry is convenience metadata, not proof that an address is safe, exists, has a trustline or belongs to a particular person.

## Reference Semantics: Python and Rust terminal implementations

The Python and Rust terminal implementations preserve the same useful destination-resolution behavior:

- [`reference/python/fresnica/contacts.py`](../../reference/python/fresnica/contacts.py)
- [`reference/python/tests/test_contacts.py`](../../reference/python/tests/test_contacts.py)
- [`reference/python/tests/test_tui_contacts.py`](../../reference/python/tests/test_tui_contacts.py)
- [`clients/rust-client/src/contacts.rs`](../../clients/rust-client/src/contacts.rs)

### 1. Resolution accepts either an alias or a direct destination

A caller may provide a saved contact name or an address directly. Resolution produces product-facing destination metadata without changing the underlying Payment validation rules.

Conceptually:

```text
user destination input
       |
       +--> saved alias -> stored destination + optional defaults
       |
       +--> direct address -> direct destination
       |
       v
ResolvedDestination
  address
  optional memo/default metadata
  optional contact/display identity
```

### 2. Explicit Flow input overrides stored defaults

Both reference implementations treat a saved contact memo as a convenience default. If the Send Flow supplies an explicit memo, the explicit value wins.

This precedence rule is a strong candidate for promotion because it prevents local address-book metadata from silently overriding current user intent.

### 3. Contact identity may survive into review/presentation

The Python TUI regression tests preserve the resolved contact name alongside the exact destination address in transaction review. This lets the user see both friendly identity and chain destination rather than replacing one with the other.

That separation is a useful candidate semantic:

```text
contact/display identity != chain destination identity
```

### 4. Stored destination data is validated, but not trusted as chain truth

The terminal references currently validate saved Classic `G...` addresses on contact creation/load. The Payment capability still performs the authoritative payment/account checks when the contact is used.

Future platforms may support richer address/identity types only when the consuming Capability defines their semantics.

## Candidate semantics for promotion

1. Resolve aliases and direct destinations into one semantic destination result.
2. Preserve both friendly contact identity and exact chain destination where useful for review.
3. Treat stored contact fields as defaults; explicit current Flow input has precedence.
4. Re-run consuming Capability validation after resolution rather than trusting address-book data as chain truth.
5. Keep contact storage/sync independent from transaction signing and cryptographic authority.

## Implementation-specific choices today

The following are not currently shared requirements:

- local `contacts.json` storage;
- one optional text memo field per contact;
- Classic `G...` as the only supported destination type;
- case-insensitive contact lookup semantics;
- exact Unicode normalization rules;
- sort order;
- avatars, tags, grouping or synchronization provider.

The name-normalization detail is a useful example of why the capability remains Defined: the Python reference uses Unicode `casefold`, while the Rust reference currently uses lowercase normalization. The shared contract should not freeze either mechanism without a real cross-platform need.

## Promotion criteria

Promote Contacts / Destination Resolution when multiple products converge on a stable destination-result model and precedence rules. Storage schema, sync mechanism and UI organization may remain platform-specific even after promotion.
