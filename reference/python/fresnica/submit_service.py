"""Transaction submission service.

Keeps network submission separate from transaction building and signing.
"""


class SubmitService:
    def __init__(self, adapter):
        self.adapter = adapter

    def submit(self, signed_transaction):
        return self.adapter.submit_transaction(signed_transaction)
