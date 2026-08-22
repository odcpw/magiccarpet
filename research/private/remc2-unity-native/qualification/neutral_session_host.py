#!/usr/bin/env python3
"""Strict NDJSON process transport for the neutral native GameSession.

This host is deliberately thin: it dynamically loads the explicitly supplied
candidate plug-in and forwards requests to ``mc_game_session_*_v1``. It owns no
gameplay state or fallback model. Stdout is data-only NDJSON; diagnostics go to
stderr and process startup fails closed.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import NoReturn

from verify_neutral_game_session import (
    MC_STATUS_BOUNDS,
    MC_STATUS_GENERATION_MISMATCH,
    MC_STATUS_INVALID_ARGUMENT,
    MC_STATUS_OK,
    NeutralClient,
    NeutralNativeError,
)


PROCESS_CONTRACT = "carpet-flight-process-json/v1"
MAX_REQUEST_LINE_BYTES = 16 * 1024
U64_PATTERN = re.compile(r"(?:0|[1-9][0-9]*)\Z")


def status_code(status: int) -> str:
    return {
        MC_STATUS_INVALID_ARGUMENT: "INVALID_ARGUMENT",
        MC_STATUS_BOUNDS: "BOUNDS",
        MC_STATUS_GENERATION_MISMATCH: "GENERATION_MISMATCH",
    }.get(status, f"NATIVE_STATUS_{status}")


def emit(value: dict) -> None:
    print(json.dumps(value, sort_keys=True, separators=(",", ":")), flush=True)


def reject(request_id: object, code: str, message: str, status: int | None = None) -> None:
    error = {"code": code, "message": message, "state_changed": False}
    if status is not None:
        error["native_status"] = status
    emit({"id": request_id, "ok": False, "error": error})


def require_fields(request: dict, required: set[str]) -> bool:
    actual = set(request)
    return actual == required


def parse_u64(value: object, field: str) -> int:
    if not isinstance(value, str) or U64_PATTERN.fullmatch(value) is None:
        raise ValueError(f"{field} must be a canonical decimal u64 string")
    parsed = int(value)
    if parsed > (1 << 64) - 1:
        raise ValueError(f"{field} exceeds u64")
    return parsed


def parse_u32(value: object, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= (1 << 32) - 1:
        raise ValueError(f"{field} must be a u32 JSON number")
    return value


def native_result(request_id: object, status: int, result: object = None) -> bool:
    if status != MC_STATUS_OK:
        reject(
            request_id,
            status_code(status),
            f"neutral native operation failed with status {status}",
            status,
        )
        return False
    emit({"id": request_id, "ok": True, "result": result})
    return True


def fatal(message: str) -> NoReturn:
    print(message, file=sys.stderr)
    raise SystemExit(2)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin", required=True, type=Path)
    parser.add_argument("--workspace", type=Path)
    args = parser.parse_args()
    try:
        client = NeutralClient(
            args.plugin,
            open_session=False,
            workspace=args.workspace,
        )
    except Exception as error:
        fatal(f"cannot open neutral native session: {error}")
    try:
        negotiated = False
        for line in sys.stdin:
            request_id: object = None
            try:
                if len(line.encode("utf-8")) > MAX_REQUEST_LINE_BYTES:
                    reject(None, "BOUNDS", "process request exceeds maximum byte length")
                    continue
                request = json.loads(line)
                if not isinstance(request, dict):
                    raise ValueError("request must be a JSON object")
                request_id = request.get("id")
                if request.get("contract") != PROCESS_CONTRACT:
                    reject(request_id, "UNSUPPORTED_CONTRACT", "unsupported process contract")
                    continue
                operation = request.get("operation")
                common = {"contract", "id", "operation"}
                if operation == "contract" and require_fields(request, common):
                    result = client.contract()
                    result["process_contract"] = PROCESS_CONTRACT
                    result["maximum_process_request_bytes"] = MAX_REQUEST_LINE_BYTES
                    emit({"id": request_id, "ok": True, "result": result})
                elif operation == "negotiate" and require_fields(
                    request,
                    common | {"engine_contract", "transport_contract", "capabilities"},
                ):
                    if negotiated:
                        reject(request_id, "INVALID_REQUEST", "session is already negotiated")
                        continue
                    contract = client.contract()
                    capabilities = request["capabilities"]
                    if not isinstance(capabilities, list) or not all(
                        isinstance(value, str) for value in capabilities
                    ):
                        raise ValueError("capabilities must be a string array")
                    if (
                        request["engine_contract"] != contract["engine_contract"]
                        or request["transport_contract"] != contract["transport_contract"]
                        or not set(capabilities).issubset(contract["capabilities"])
                    ):
                        reject(
                            request_id,
                            "UNSUPPORTED_CONTRACT",
                            "requested engine, transport, or capability is unsupported",
                        )
                        continue
                    client.open()
                    negotiated = True
                    emit(
                        {
                            "id": request_id,
                            "ok": True,
                            "result": {
                                "engine_contract": contract["engine_contract"],
                                "transport_contract": contract["transport_contract"],
                                "capabilities": capabilities,
                            },
                        }
                    )
                elif operation == "close" and require_fields(request, common):
                    emit({"id": request_id, "ok": True, "result": None})
                    break
                elif not negotiated:
                    reject(request_id, "NOT_NEGOTIATED", "negotiate before session operations")
                elif operation == "snapshot" and require_fields(request, common):
                    emit({"id": request_id, "ok": True, "result": client.snapshot()})
                elif operation == "step" and require_fields(
                    request, common | {"generation", "expected_tick", "command"}
                ):
                    native_result(
                        request_id,
                        client.step(
                            parse_u64(request["generation"], "generation"),
                            parse_u64(request["expected_tick"], "expected_tick"),
                            request["command"],
                        ),
                    )
                elif operation == "events" and require_fields(
                    request,
                    common | {"generation", "expected_tick", "cursor", "limit"},
                ):
                    result = client.events(
                        parse_u64(request["generation"], "generation"),
                        parse_u64(request["expected_tick"], "expected_tick"),
                        parse_u64(request["cursor"], "cursor"),
                        parse_u32(request["limit"], "limit"),
                    )
                    emit({"id": request_id, "ok": True, "result": result})
                elif operation == "audio" and require_fields(
                    request,
                    common | {"generation", "expected_tick", "event_sequence"},
                ):
                    result = client.try_audio(
                        parse_u64(request["generation"], "generation"),
                        parse_u64(request["expected_tick"], "expected_tick"),
                        parse_u64(request["event_sequence"], "event_sequence"),
                    )
                    emit({"id": request_id, "ok": True, "result": result})
                elif operation == "presentation_asset" and require_fields(
                    request,
                    common | {"generation", "expected_tick", "semantic_id"},
                ):
                    semantic_id = request["semantic_id"]
                    if not isinstance(semantic_id, str):
                        raise ValueError("semantic_id must be a string")
                    result = client.presentation_asset(
                        parse_u64(request["generation"], "generation"),
                        parse_u64(request["expected_tick"], "expected_tick"),
                        semantic_id,
                    )
                    emit({"id": request_id, "ok": True, "result": result})
                elif operation == "reset" and require_fields(
                    request, common | {"generation", "expected_tick"}
                ):
                    native_result(
                        request_id,
                        client.reset(
                            parse_u64(request["generation"], "generation"),
                            parse_u64(request["expected_tick"], "expected_tick"),
                        ),
                    )
                else:
                    reject(
                        request_id,
                        "INVALID_REQUEST",
                        "unknown operation or unexpected/missing request field",
                    )
            except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
                reject(request_id, "INVALID_REQUEST", str(error))
            except NeutralNativeError as error:
                reject(
                    request_id,
                    status_code(error.status),
                    str(error),
                    error.status,
                )
    finally:
        client.close()


if __name__ == "__main__":
    main()
