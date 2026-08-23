from fresnica.offer_result import (
    _offer_outcome_from_result_json,
    parse_offer_submission_outcome,
)


def test_offer_outcome_finds_manage_offer_after_change_trust():
    payload = {
        "result": {
            "txsuccess": [
                {"opinner": {"change_trust": "success"}},
                {
                    "opinner": {
                        "manage_sell_offer": {
                            "success": {
                                "offers_claimed": [{"claim": 1}, {"claim": 2}],
                                "offer": {"created": {"offer_id": "123"}},
                            }
                        }
                    }
                },
            ]
        }
    }

    outcome = _offer_outcome_from_result_json(payload)

    assert outcome is not None
    assert outcome.effect == "created"
    assert outcome.claimed_offer_count == 2
    assert outcome.offer_id == "123"


def test_offer_outcome_handles_manage_buy_full_fill_without_resting_offer():
    payload = {
        "result": {
            "txsuccess": [
                {
                    "opinner": {
                        "manage_buy_offer": {
                            "success": {
                                "offers_claimed": [{"claim": 1}],
                                "offer": "deleted",
                            }
                        }
                    }
                }
            ]
        }
    }

    outcome = _offer_outcome_from_result_json(payload)

    assert outcome is not None
    assert outcome.effect == "deleted"
    assert outcome.claimed_offer_count == 1
    assert outcome.offer_id is None


def test_offer_outcome_handles_update():
    payload = {
        "result": {
            "txsuccess": [
                {
                    "opinner": {
                        "manage_sell_offer": {
                            "success": {
                                "offers_claimed": [],
                                "offer": {"updated": {"offer_id": "456"}},
                            }
                        }
                    }
                }
            ]
        }
    }

    outcome = _offer_outcome_from_result_json(payload)

    assert outcome is not None
    assert outcome.effect == "updated"
    assert outcome.claimed_offer_count == 0
    assert outcome.offer_id == "456"


def test_offer_outcome_ignores_non_success_and_invalid_xdr():
    assert _offer_outcome_from_result_json({"result": {"txfailed": []}}) is None
    assert parse_offer_submission_outcome("not-xdr") is None
