"""Send command.

The command delegates to Fresnica services.
"""


def execute_send(context, request):
    transfer_service = context.transfer_service
    return transfer_service.prepare(request)
