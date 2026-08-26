# Contacts / Destination Resolution Capability

Maturity: **Defined**

## Purpose

Contacts / Destination Resolution provides product-friendly destination aliases and optional destination metadata used by Flows such as Send.

It is currently **Defined** because address-book schema, supported identity types and product UX are expected to vary across platforms.

## Agreed boundary

A destination resolver may map a user-facing alias to semantic destination data such as:

- validated account address;
- optional memo/destination metadata;
- contact/display label.

Explicit Flow input must take precedence over stored defaults when the two represent the same field. For example, an explicitly supplied memo should not be silently replaced by a contact's default memo.

## Security and identity

Destination resolution must not bypass Account/Payment validation. A contact entry is convenience metadata, not proof that an address is safe, exists, has a trustline or belongs to a particular person.

Current terminal contacts accept Classic `G...` destinations. Other platforms may support richer identity/address types only when the consuming Capability defines their semantics.

## Implementation freedom

Storage engine, contact synchronization, naming rules, avatar metadata and UI are platform-owned.

The current Rust `contacts.json` file and case-insensitive name lookup are reference implementation details, not normative cross-platform behavior.
