"""Wallet-level anchor transfer workflow independent from presentation layers."""

from dataclasses import dataclass, field
from typing import Literal, TypeAlias

from .anchor_service import AnchorCapabilities, AnchorError, AnchorService
from .models import Asset


AnchorTransferKind = Literal["deposit", "withdraw"]
AnchorProtocol = Literal["sep24", "sep6"]


@dataclass(frozen=True)
class AnchorTransferPlan:
    kind: AnchorTransferKind
    protocol: AnchorProtocol
    requires_signing: bool
    transfer_type: str | None = None
    fields: dict = field(default_factory=dict)

    @property
    def user_fields(self) -> dict:
        return {
            name: spec
            for name, spec in self.fields.items()
            if name not in {"asset_code", "account"}
        }

    @property
    def requires_fields(self) -> bool:
        return bool(self.user_fields)


@dataclass(frozen=True)
class AnchorNeedFields:
    plan: AnchorTransferPlan


@dataclass(frozen=True)
class AnchorOpenUrl:
    kind: AnchorTransferKind
    url: str
    transaction_id: str | None = None


@dataclass(frozen=True)
class AnchorKycRequired:
    kind: AnchorTransferKind
    payload: dict


@dataclass(frozen=True)
class AnchorDepositInstructions:
    payload: dict


@dataclass(frozen=True)
class AnchorWithdrawalPayment:
    asset: Asset
    amount: str
    destination: str
    memo: str | None = None
    memo_type: str | None = None
    anchor_domain: str | None = None
    extra_info: str | None = None
    payload: dict = field(default_factory=dict)


AnchorTransferOutcome: TypeAlias = (
    AnchorNeedFields
    | AnchorOpenUrl
    | AnchorKycRequired
    | AnchorDepositInstructions
    | AnchorWithdrawalPayment
)


class AnchorTransferService:
    """Translate discovered anchor capabilities into wallet-level next actions."""

    def __init__(self, protocol_service: AnchorService | None = None):
        self.protocol_service = protocol_service or AnchorService()

    def discover(self, asset: Asset, home_domain: str) -> AnchorCapabilities:
        return self.protocol_service.discover(asset, home_domain)

    def plan(
        self,
        capabilities: AnchorCapabilities,
        kind: AnchorTransferKind,
    ) -> AnchorTransferPlan:
        if kind not in {"deposit", "withdraw"}:
            raise ValueError(f"Unsupported anchor transfer kind: {kind}")

        sep24_enabled = (
            capabilities.sep24_deposit if kind == "deposit" else capabilities.sep24_withdraw
        )
        if (
            sep24_enabled
            and capabilities.sep24_url
            and capabilities.web_auth_url
            and capabilities.signing_key
        ):
            return AnchorTransferPlan(kind=kind, protocol="sep24", requires_signing=True)

        sep6_enabled = (
            capabilities.sep6_deposit if kind == "deposit" else capabilities.sep6_withdraw
        )
        if sep6_enabled and capabilities.sep6_url:
            info = (
                capabilities.sep6_deposit_info
                if kind == "deposit"
                else capabilities.sep6_withdraw_info
            )
            transfer_type, fields = _sep6_schema(info)
            requires_signing = kind == "withdraw" or bool(
                info.get("authentication_required", False)
                if isinstance(info, dict)
                else False
            )
            return AnchorTransferPlan(
                kind=kind,
                protocol="sep6",
                requires_signing=requires_signing,
                transfer_type=transfer_type,
                fields=fields,
            )

        raise AnchorError(
            f"No usable SEP-24/SEP-6 {kind} flow is advertised for this asset"
        )

    def start(
        self,
        wallet,
        asset: Asset,
        capabilities: AnchorCapabilities,
        kind: AnchorTransferKind,
        network_passphrase: str,
        *,
        fields: dict | None = None,
        plan: AnchorTransferPlan | None = None,
    ) -> AnchorTransferOutcome:
        plan = plan or self.plan(capabilities, kind)
        if plan.kind != kind:
            raise AnchorError("Anchor transfer plan does not match requested action")
        if plan.requires_signing and not wallet.can_sign():
            raise AnchorError("This anchor transfer requires a signing wallet")

        if plan.protocol == "sep24":
            transfer = self.protocol_service.start_sep24(
                wallet,
                asset,
                capabilities,
                kind,
                network_passphrase,
            )
            return AnchorOpenUrl(
                kind=kind,
                url=transfer.url,
                transaction_id=transfer.transaction_id,
            )

        if plan.requires_fields and fields is None:
            return AnchorNeedFields(plan)

        request_fields = dict(fields or {})
        _validate_fields(plan, request_fields)
        if plan.transfer_type and "type" not in request_fields:
            request_fields["type"] = plan.transfer_type
        transfer = self.protocol_service.start_sep6(
            wallet,
            asset,
            capabilities,
            kind,
            network_passphrase,
            request_fields,
        )
        return _interpret_sep6(asset, capabilities, transfer)


def _interpret_sep6(asset, capabilities, transfer) -> AnchorTransferOutcome:
    payload = dict(transfer.payload)
    response_type = str(payload.get("type") or "")
    if response_type in {
        "non_interactive_customer_info_needed",
        "customer_info_status",
    }:
        return AnchorKycRequired(kind=transfer.kind, payload=payload)

    if transfer.kind == "deposit":
        return AnchorDepositInstructions(payload=payload)

    destination = _optional_text(payload.get("account_id"))
    amount = _optional_text(transfer.request.get("amount"))
    if not destination or not amount:
        raise AnchorError(
            "SEP-6 withdraw response is missing account_id or requested amount"
        )
    extra = payload.get("extra_info")
    if isinstance(extra, dict):
        extra = extra.get("message")
    return AnchorWithdrawalPayment(
        asset=asset,
        amount=amount,
        destination=destination,
        memo=_optional_text(payload.get("memo")),
        memo_type=_optional_text(payload.get("memo_type")),
        anchor_domain=capabilities.domain,
        extra_info=_optional_text(extra),
        payload=payload,
    )


def _validate_fields(plan: AnchorTransferPlan, values: dict) -> None:
    missing = []
    for name, raw_spec in plan.user_fields.items():
        spec = raw_spec if isinstance(raw_spec, dict) else {}
        if spec.get("optional", False):
            continue
        if not str(values.get(name) or "").strip():
            missing.append(name)
    if missing:
        raise AnchorError(f"Anchor transfer requires field: {missing[0]}")


def _sep6_schema(info: dict) -> tuple[str | None, dict]:
    if not isinstance(info, dict):
        return None, {}
    types = info.get("types")
    if isinstance(types, dict) and types:
        if len(types) != 1:
            raise AnchorError("Anchor advertises multiple SEP-6 transfer methods")
        transfer_type = next(iter(types))
        spec = types.get(transfer_type)
        fields = spec.get("fields", {}) if isinstance(spec, dict) else {}
        return transfer_type, dict(fields) if isinstance(fields, dict) else {}
    fields = info.get("fields", {})
    return None, dict(fields) if isinstance(fields, dict) else {}


def _optional_text(value) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None
