#!/usr/bin/env python3
"""Build a conservative private remc2 source and state-access atlas.

The extractor is intentionally intraprocedural and evidence-preserving. It
uses Clang USRs where available, records every diagnostic, marks unresolved
calls and pointer escapes, and stores sparse edges in SQLite so the largest
translation units can be processed sequentially without retaining one enormous
AST-derived graph in memory.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import shlex
import sqlite3
import subprocess
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Iterator, Sequence

from clang import cindex


SCHEMA_VERSION = "remc2-private-atlas/0.1"
ASSIGNMENT_OPERATORS = {
    "=",
    "+=",
    "-=",
    "*=",
    "/=",
    "%=",
    "<<=",
    ">>=",
    "&=",
    "|=",
    "^=",
}
COMPOUND_ASSIGNMENTS = ASSIGNMENT_OPERATORS - {"="}
FUNCTION_KINDS = {
    cindex.CursorKind.FUNCTION_DECL,
    cindex.CursorKind.CXX_METHOD,
    cindex.CursorKind.CONSTRUCTOR,
    cindex.CursorKind.DESTRUCTOR,
    cindex.CursorKind.FUNCTION_TEMPLATE,
    cindex.CursorKind.CONVERSION_FUNCTION,
}
RECORD_KINDS = {
    cindex.CursorKind.STRUCT_DECL,
    cindex.CursorKind.CLASS_DECL,
    cindex.CursorKind.UNION_DECL,
}
BULK_READ_WRITE = {"memcpy", "memmove", "qmemcpy", "memccpy"}
BULK_WRITE = {"memset", "bzero", "ZeroMemory"}


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    repo_root = script.parents[3]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source-root",
        type=Path,
        default=repo_root / "upstream" / "magic-carpet-2-hd",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=script.parent / "generated",
    )
    parser.add_argument("--compile-commands", type=Path)
    parser.add_argument(
        "--build-config",
        choices=("Debug", "Release"),
        default="Debug",
    )
    parser.add_argument(
        "--pilot",
        type=int,
        default=0,
        help="Parse only the first N compiled translation units.",
    )
    parser.add_argument(
        "--only",
        action="append",
        default=[],
        help="Parse only source paths containing this substring; repeatable.",
    )
    return parser.parse_args()


def run_text(command: Sequence[str], cwd: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return completed.stdout.strip()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def relative_or_absolute(path: Path, source_root: Path) -> str:
    try:
        return path.resolve().relative_to(source_root.resolve()).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def in_source_root(cursor: cindex.Cursor, source_root: Path) -> bool:
    location = cursor.location
    if not location or not location.file:
        return False
    try:
        Path(location.file.name).resolve().relative_to(source_root.resolve())
        return True
    except ValueError:
        return False


def location_tuple(
    cursor: cindex.Cursor, source_root: Path
) -> tuple[str, int, int]:
    location = cursor.location
    if not location or not location.file:
        return ("", 0, 0)
    return (
        relative_or_absolute(Path(location.file.name), source_root),
        location.line,
        location.column,
    )


def cursor_key(cursor: cindex.Cursor, source_root: Path, prefix: str) -> str:
    usr = cursor.get_usr()
    if usr:
        return usr
    file_name, line, column = location_tuple(cursor, source_root)
    return f"{prefix}:{file_name}:{line}:{column}:{cursor.spelling}"


def qualified_name(cursor: cindex.Cursor) -> str:
    parts: list[str] = []
    current = cursor
    while current and current.kind != cindex.CursorKind.TRANSLATION_UNIT:
        if current.spelling:
            parts.append(current.spelling)
        current = current.semantic_parent
    return "::".join(reversed(parts))


def extent_contains(outer: cindex.Cursor, inner: cindex.Cursor) -> bool:
    outer_start = outer.extent.start
    outer_end = outer.extent.end
    inner_start = inner.extent.start
    if not outer_start.file or not inner_start.file:
        return False
    if outer_start.file.name != inner_start.file.name:
        return False
    before_or_equal = (
        outer_start.line,
        outer_start.column,
    ) <= (
        inner_start.line,
        inner_start.column,
    )
    after_or_equal = (
        inner_start.line,
        inner_start.column,
    ) <= (
        outer_end.line,
        outer_end.column,
    )
    return before_or_equal and after_or_equal


def token_spellings(cursor: cindex.Cursor, limit: int = 200) -> list[str]:
    result: list[str] = []
    try:
        for token in cursor.get_tokens():
            result.append(token.spelling)
            if len(result) >= limit:
                break
    except cindex.LibclangError:
        pass
    return result


def compact_tokens(cursor: cindex.Cursor, limit: int = 240) -> str:
    text = " ".join(token_spellings(cursor, 120))
    return text[:limit]


def direct_children(cursor: cindex.Cursor) -> list[cindex.Cursor]:
    return list(cursor.get_children())


def classify_access(
    node: cindex.Cursor, ancestors: Sequence[cindex.Cursor]
) -> tuple[str, str]:
    """Return conservative access mode and confidence."""
    for ancestor in reversed(ancestors):
        if ancestor.kind == cindex.CursorKind.UNARY_OPERATOR:
            tokens = token_spellings(ancestor, 20)
            if "++" in tokens or "--" in tokens:
                return ("readwrite", "exact")
            if "&" in tokens:
                return ("address_escape", "possible")
        if ancestor.kind in {
            cindex.CursorKind.BINARY_OPERATOR,
            cindex.CursorKind.COMPOUND_ASSIGNMENT_OPERATOR,
        }:
            children = direct_children(ancestor)
            if not children:
                continue
            tokens = token_spellings(ancestor, 80)
            operator = next(
                (token for token in tokens if token in ASSIGNMENT_OPERATORS),
                "",
            )
            if not operator:
                continue
            if extent_contains(children[0], node):
                if operator in COMPOUND_ASSIGNMENTS:
                    return ("readwrite", "exact")
                return ("write", "exact")
            return ("read", "exact")
        if ancestor.kind == cindex.CursorKind.CALL_EXPR:
            return ("argument_or_escape", "possible")
        if ancestor.kind in FUNCTION_KINDS:
            break
    return ("read", "exact")


def is_global_variable(cursor: cindex.Cursor) -> bool:
    if cursor.kind != cindex.CursorKind.VAR_DECL:
        return False
    parent = cursor.semantic_parent
    return bool(
        parent
        and parent.kind
        in {
            cindex.CursorKind.TRANSLATION_UNIT,
            cindex.CursorKind.NAMESPACE,
        }
    )


def diagnostic_severity(value: int) -> str:
    names = {
        cindex.Diagnostic.Ignored: "ignored",
        cindex.Diagnostic.Note: "note",
        cindex.Diagnostic.Warning: "warning",
        cindex.Diagnostic.Error: "error",
        cindex.Diagnostic.Fatal: "fatal",
    }
    return names.get(value, f"unknown-{value}")


def cmake_sources(source_root: Path) -> list[Path]:
    remc2_root = source_root / "remc2"
    cmake_file = remc2_root / "CMakeLists.txt"
    text = cmake_file.read_text(encoding="utf-8", errors="replace")
    source_pattern = re.compile(r'"([^"]+\.(?:cpp|c))"')
    result: list[Path] = []
    for match in source_pattern.finditer(text):
        candidate = (remc2_root / match.group(1)).resolve()
        if candidate.exists():
            result.append(candidate)
    return sorted(set(result))


def all_project_units(source_root: Path) -> list[Path]:
    remc2_root = source_root / "remc2"
    return sorted(
        path.resolve()
        for path in remc2_root.rglob("*")
        if path.suffix in {".cpp", ".c"}
        and "/out/" not in path.as_posix()
        and "/Release/" not in path.as_posix()
    )


def libclang_toolchain_arguments(source_root: Path, source: Path) -> list[str]:
    resource_dir = run_text(
        ["clang", "-print-resource-dir"], cwd=source_root
    )
    arguments = [f"-resource-dir={resource_dir}"]
    if source.suffix != ".c":
        cxx_roots = sorted(
            Path("/usr/include/c++").glob("*"), reverse=True
        )
        if cxx_roots:
            cxx_root = cxx_roots[0]
            arguments.extend(
                [
                    f"-I{cxx_root}",
                    f"-I{cxx_root / 'x86_64-pc-linux-gnu'}",
                    f"-I{cxx_root / 'backward'}",
                ]
            )
    return arguments


def fallback_arguments(
    source_root: Path, build_config: str, source: Path
) -> list[str]:
    remc2_root = source_root / "remc2"
    arguments = [
        "-std=c11" if source.suffix == ".c" else "-std=c++17",
        "-DCOMPILE_FOR_64BIT",
        "-Wno-narrowing",
        "-Wno-unknown-pragmas",
        f"-I{remc2_root}",
        f"-I{remc2_root / 'engine'}",
        f"-I{remc2_root / 'portability'}",
        f"-I{remc2_root / 'utilities'}",
        f"-I{source_root}",
        f"-I{source_root / 'findfirst'}",
        f"-I{source_root / 'spdlog' / 'include'}",
        f"-I{source_root / 'nlohmann-json'}",
        f"-I{source_root / 'libADLMIDI-master' / 'include'}",
        "-I/usr/include/SDL2",
        *libclang_toolchain_arguments(source_root, source),
    ]
    if build_config == "Debug":
        arguments.append("-D_DEBUG")
    return arguments


def load_compile_commands(path: Path) -> dict[Path, list[str]]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    result: dict[Path, list[str]] = {}
    for entry in raw:
        file_path = Path(entry["file"])
        if not file_path.is_absolute():
            file_path = Path(entry["directory"]) / file_path
        if "arguments" in entry:
            arguments = list(entry["arguments"])
        else:
            arguments = shlex.split(entry["command"])
        if arguments:
            arguments = arguments[1:]
        cleaned: list[str] = []
        skip_next = False
        for argument in arguments:
            if skip_next:
                skip_next = False
                continue
            if argument in {"-c", "-o"}:
                skip_next = argument == "-o"
                continue
            if argument == str(file_path) or argument.endswith(file_path.name):
                continue
            cleaned.append(argument)
        result[file_path.resolve()] = cleaned
    return result


def initialize_database(path: Path) -> sqlite3.Connection:
    if path.exists():
        path.unlink()
    connection = sqlite3.connect(path)
    connection.executescript(
        """
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=FULL;
        CREATE TABLE metadata (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        CREATE TABLE translation_units (
          path TEXT PRIMARY KEY,
          build_membership TEXT NOT NULL,
          parse_status TEXT NOT NULL,
          line_count INTEGER NOT NULL,
          sha256 TEXT NOT NULL,
          argument_source TEXT NOT NULL,
          error_count INTEGER NOT NULL DEFAULT 0,
          warning_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE diagnostics (
          tu_path TEXT NOT NULL,
          severity TEXT NOT NULL,
          message TEXT NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL
        );
        CREATE TABLE functions (
          usr TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          qualified_name TEXT NOT NULL,
          kind TEXT NOT NULL,
          return_type TEXT NOT NULL,
          storage_class TEXT NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL,
          is_definition INTEGER NOT NULL,
          is_inline INTEGER NOT NULL
        );
        CREATE TABLE parameters (
          function_usr TEXT NOT NULL,
          ordinal INTEGER NOT NULL,
          name TEXT NOT NULL,
          type TEXT NOT NULL,
          PRIMARY KEY (function_usr, ordinal)
        );
        CREATE TABLE records (
          usr TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          qualified_name TEXT NOT NULL,
          kind TEXT NOT NULL,
          size_bytes INTEGER NOT NULL,
          align_bytes INTEGER NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL
        );
        CREATE TABLE fields (
          usr TEXT PRIMARY KEY,
          record_usr TEXT NOT NULL,
          name TEXT NOT NULL,
          qualified_name TEXT NOT NULL,
          type TEXT NOT NULL,
          offset_bits INTEGER NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL
        );
        CREATE TABLE globals (
          usr TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          qualified_name TEXT NOT NULL,
          type TEXT NOT NULL,
          storage_class TEXT NOT NULL,
          initializer TEXT NOT NULL,
          initializer_token_count INTEGER NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL
        );
        CREATE TABLE calls (
          caller_usr TEXT NOT NULL,
          callee_usr TEXT NOT NULL,
          callee_name TEXT NOT NULL,
          edge_kind TEXT NOT NULL,
          confidence TEXT NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL,
          UNIQUE (
            caller_usr, callee_usr, callee_name, edge_kind, file, line, column
          )
        );
        CREATE TABLE state_accesses (
          function_usr TEXT NOT NULL,
          target_usr TEXT NOT NULL,
          target_kind TEXT NOT NULL,
          target_name TEXT NOT NULL,
          access_kind TEXT NOT NULL,
          confidence TEXT NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL,
          UNIQUE (
            function_usr, target_usr, target_kind, access_kind,
            file, line, column
          )
        );
        CREATE TABLE literals (
          function_usr TEXT NOT NULL,
          literal_kind TEXT NOT NULL,
          value TEXT NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL,
          UNIQUE (function_usr, literal_kind, value, file, line, column)
        );
        CREATE TABLE bulk_operations (
          function_usr TEXT NOT NULL,
          operation TEXT NOT NULL,
          arguments TEXT NOT NULL,
          confidence TEXT NOT NULL,
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          column INTEGER NOT NULL,
          UNIQUE (function_usr, operation, file, line, column)
        );
        CREATE TABLE includes (
          source TEXT NOT NULL,
          included TEXT NOT NULL,
          depth INTEGER NOT NULL,
          UNIQUE (source, included, depth)
        );
        CREATE TABLE macros (
          file TEXT NOT NULL,
          line INTEGER NOT NULL,
          name TEXT NOT NULL,
          value TEXT NOT NULL,
          function_like INTEGER NOT NULL,
          build_membership TEXT NOT NULL,
          UNIQUE (file, line, name)
        );
        CREATE INDEX calls_caller ON calls(caller_usr);
        CREATE INDEX calls_callee ON calls(callee_usr);
        CREATE INDEX accesses_function ON state_accesses(function_usr);
        CREATE INDEX accesses_target ON state_accesses(target_usr);
        """
    )
    return connection


def insert_function(
    connection: sqlite3.Connection,
    cursor: cindex.Cursor,
    source_root: Path,
) -> str:
    usr = cursor_key(cursor, source_root, "function")
    file_name, line, column = location_tuple(cursor, source_root)
    try:
        inline = int(cursor.is_function_inlined())
    except cindex.LibclangError:
        inline = 0
    connection.execute(
        """
        INSERT OR REPLACE INTO functions VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            usr,
            cursor.spelling or cursor.displayname,
            qualified_name(cursor),
            cursor.kind.name,
            cursor.result_type.spelling
            if cursor.kind != cindex.CursorKind.CONSTRUCTOR
            else "",
            cursor.storage_class.name,
            file_name,
            line,
            column,
            int(cursor.is_definition()),
            inline,
        ),
    )
    for ordinal, argument in enumerate(cursor.get_arguments() or []):
        connection.execute(
            "INSERT OR REPLACE INTO parameters VALUES (?, ?, ?, ?)",
            (usr, ordinal, argument.spelling, argument.type.spelling),
        )
    return usr


def insert_record(
    connection: sqlite3.Connection,
    cursor: cindex.Cursor,
    source_root: Path,
) -> str:
    usr = cursor_key(cursor, source_root, "record")
    file_name, line, column = location_tuple(cursor, source_root)
    connection.execute(
        "INSERT OR REPLACE INTO records VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            usr,
            cursor.spelling,
            qualified_name(cursor),
            cursor.kind.name,
            cursor.type.get_size(),
            cursor.type.get_align(),
            file_name,
            line,
            column,
        ),
    )
    return usr


def insert_field(
    connection: sqlite3.Connection,
    cursor: cindex.Cursor,
    source_root: Path,
) -> None:
    usr = cursor_key(cursor, source_root, "field")
    parent = cursor.semantic_parent
    record_usr = (
        cursor_key(parent, source_root, "record") if parent else ""
    )
    file_name, line, column = location_tuple(cursor, source_root)
    try:
        offset_bits = cursor.get_field_offsetof()
    except cindex.LibclangError:
        offset_bits = -1
    connection.execute(
        "INSERT OR REPLACE INTO fields VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            usr,
            record_usr,
            cursor.spelling,
            qualified_name(cursor),
            cursor.type.spelling,
            offset_bits,
            file_name,
            line,
            column,
        ),
    )


def insert_global(
    connection: sqlite3.Connection,
    cursor: cindex.Cursor,
    source_root: Path,
) -> None:
    usr = cursor_key(cursor, source_root, "global")
    file_name, line, column = location_tuple(cursor, source_root)
    tokens = token_spellings(cursor, 1000)
    initializer = " ".join(tokens[:120])[:500]
    connection.execute(
        "INSERT OR REPLACE INTO globals VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            usr,
            cursor.spelling,
            qualified_name(cursor),
            cursor.type.spelling,
            cursor.storage_class.name,
            initializer,
            len(tokens),
            file_name,
            line,
            column,
        ),
    )


def visit_function_body(
    connection: sqlite3.Connection,
    function: cindex.Cursor,
    function_usr: str,
    source_root: Path,
) -> None:
    def visit(
        node: cindex.Cursor, ancestors: tuple[cindex.Cursor, ...]
    ) -> None:
        if node != function and node.kind in FUNCTION_KINDS:
            return
        if in_source_root(node, source_root):
            file_name, line, column = location_tuple(node, source_root)
            if node.kind == cindex.CursorKind.CALL_EXPR:
                referenced = node.referenced
                if referenced and referenced.kind in FUNCTION_KINDS:
                    callee_usr = cursor_key(
                        referenced, source_root, "function"
                    )
                    callee_name = qualified_name(referenced) or node.spelling
                    edge_kind = "calls_direct"
                    confidence = "exact"
                else:
                    callee_usr = ""
                    callee_name = (
                        node.spelling
                        or node.displayname
                        or compact_tokens(node, 160)
                    )
                    edge_kind = "calls_indirect_or_unresolved"
                    confidence = "unknown"
                connection.execute(
                    "INSERT OR IGNORE INTO calls VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    (
                        function_usr,
                        callee_usr,
                        callee_name,
                        edge_kind,
                        confidence,
                        file_name,
                        line,
                        column,
                    ),
                )
                simple_name = (referenced.spelling if referenced else node.spelling)
                if simple_name in BULK_READ_WRITE | BULK_WRITE:
                    connection.execute(
                        "INSERT OR IGNORE INTO bulk_operations VALUES (?, ?, ?, ?, ?, ?, ?)",
                        (
                            function_usr,
                            simple_name,
                            compact_tokens(node, 500),
                            "contract",
                            file_name,
                            line,
                            column,
                        ),
                    )
            elif node.kind in {
                cindex.CursorKind.DECL_REF_EXPR,
                cindex.CursorKind.MEMBER_REF_EXPR,
            }:
                referenced = node.referenced
                if referenced:
                    target_kind = ""
                    if (
                        node.kind == cindex.CursorKind.DECL_REF_EXPR
                        and is_global_variable(referenced)
                    ):
                        target_kind = "global"
                    elif (
                        node.kind == cindex.CursorKind.MEMBER_REF_EXPR
                        and referenced.kind == cindex.CursorKind.FIELD_DECL
                    ):
                        target_kind = "field"
                    if target_kind:
                        access_kind, confidence = classify_access(
                            node, ancestors
                        )
                        target_usr = cursor_key(
                            referenced, source_root, target_kind
                        )
                        connection.execute(
                            """
                            INSERT OR IGNORE INTO state_accesses
                            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                            """,
                            (
                                function_usr,
                                target_usr,
                                target_kind,
                                qualified_name(referenced)
                                or referenced.spelling,
                                access_kind,
                                confidence,
                                file_name,
                                line,
                                column,
                            ),
                        )
                    elif referenced.kind in FUNCTION_KINDS:
                        direct_callee_reference = any(
                            ancestor.kind == cindex.CursorKind.CALL_EXPR
                            and ancestor.referenced
                            and cursor_key(
                                ancestor.referenced,
                                source_root,
                                "function",
                            )
                            == cursor_key(
                                referenced,
                                source_root,
                                "function",
                            )
                            for ancestor in ancestors
                        )
                        if not direct_callee_reference:
                            connection.execute(
                                """
                                INSERT OR IGNORE INTO calls
                                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                                """,
                                (
                                    function_usr,
                                    cursor_key(
                                        referenced,
                                        source_root,
                                        "function",
                                    ),
                                    qualified_name(referenced),
                                    "takes_address_or_callback_reference",
                                    "possible",
                                    file_name,
                                    line,
                                    column,
                                ),
                            )
            elif node.kind in {
                cindex.CursorKind.INTEGER_LITERAL,
                cindex.CursorKind.FLOATING_LITERAL,
                cindex.CursorKind.CHARACTER_LITERAL,
            }:
                connection.execute(
                    "INSERT OR IGNORE INTO literals VALUES (?, ?, ?, ?, ?, ?)",
                    (
                        function_usr,
                        node.kind.name,
                        compact_tokens(node, 100),
                        file_name,
                        line,
                        column,
                    ),
                )
        for child in node.get_children():
            visit(child, ancestors + (node,))

    for child in function.get_children():
        visit(child, (function,))


def scan_translation_unit(
    connection: sqlite3.Connection,
    index: cindex.Index,
    source: Path,
    arguments: Sequence[str],
    source_root: Path,
    build_membership: str,
    argument_source: str,
) -> tuple[str, int, int]:
    source_rel = relative_or_absolute(source, source_root)
    line_count = sum(
        1 for _ in source.open("r", encoding="utf-8", errors="replace")
    )
    status = "parsed"
    errors = 0
    warnings = 0
    try:
        translation_unit = index.parse(
            str(source),
            args=list(arguments),
            options=cindex.TranslationUnit.PARSE_INCOMPLETE,
        )
    except cindex.TranslationUnitLoadError as error:
        connection.execute(
            "INSERT INTO diagnostics VALUES (?, ?, ?, ?, ?, ?)",
            (
                source_rel,
                "fatal",
                f"TranslationUnitLoadError: {error}",
                source_rel,
                0,
                0,
            ),
        )
        status = "load_failed"
        errors = 1
        translation_unit = None

    if translation_unit is not None:
        for diagnostic in translation_unit.diagnostics:
            severity = diagnostic_severity(diagnostic.severity)
            if severity in {"error", "fatal"}:
                errors += 1
            elif severity == "warning":
                warnings += 1
            location = diagnostic.location
            file_name = (
                relative_or_absolute(Path(location.file.name), source_root)
                if location and location.file
                else source_rel
            )
            connection.execute(
                "INSERT INTO diagnostics VALUES (?, ?, ?, ?, ?, ?)",
                (
                    source_rel,
                    severity,
                    diagnostic.spelling,
                    file_name,
                    location.line if location else 0,
                    location.column if location else 0,
                ),
            )
        if errors:
            status = "parsed_with_errors"

        for include in translation_unit.get_includes():
            if not include.source or not include.include:
                continue
            connection.execute(
                "INSERT OR IGNORE INTO includes VALUES (?, ?, ?)",
                (
                    relative_or_absolute(
                        Path(include.source.name), source_root
                    ),
                    relative_or_absolute(
                        Path(include.include.name), source_root
                    ),
                    include.depth,
                ),
            )

        def top_level(cursor: cindex.Cursor) -> None:
            if in_source_root(cursor, source_root):
                if cursor.kind in FUNCTION_KINDS and cursor.is_definition():
                    function_usr = insert_function(
                        connection, cursor, source_root
                    )
                    visit_function_body(
                        connection, cursor, function_usr, source_root
                    )
                    return
                if cursor.kind in RECORD_KINDS and cursor.is_definition():
                    insert_record(connection, cursor, source_root)
                elif cursor.kind == cindex.CursorKind.FIELD_DECL:
                    insert_field(connection, cursor, source_root)
                elif is_global_variable(cursor):
                    insert_global(connection, cursor, source_root)
            for child in cursor.get_children():
                top_level(child)

        top_level(translation_unit.cursor)
        del translation_unit

    connection.execute(
        """
        INSERT OR REPLACE INTO translation_units
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            source_rel,
            build_membership,
            status,
            line_count,
            sha256_file(source),
            argument_source,
            errors,
            warnings,
        ),
    )
    connection.commit()
    return status, errors, warnings


def scan_macros(
    connection: sqlite3.Connection,
    source_root: Path,
    compiled: set[Path],
) -> None:
    remc2_root = source_root / "remc2"
    candidates = sorted(
        path
        for path in remc2_root.rglob("*")
        if path.suffix in {".h", ".hpp", ".cpp", ".c"}
        and "/out/" not in path.as_posix()
        and "/Release/" not in path.as_posix()
    )
    macro_pattern = re.compile(
        r"^\s*#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)(\s*\([^)]*\))?\s*(.*)$"
    )
    for path in candidates:
        membership = (
            "compiled_translation_unit"
            if path.resolve() in compiled
            else "header_or_present_source"
        )
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(),
            1,
        ):
            match = macro_pattern.match(line)
            if not match:
                continue
            connection.execute(
                "INSERT OR IGNORE INTO macros VALUES (?, ?, ?, ?, ?, ?)",
                (
                    relative_or_absolute(path, source_root),
                    line_number,
                    match.group(1),
                    match.group(3).strip()[:1000],
                    int(bool(match.group(2))),
                    membership,
                ),
            )
    connection.commit()


def table_count(connection: sqlite3.Connection, table: str) -> int:
    return connection.execute(
        f"SELECT COUNT(*) FROM {table}"  # table names are internal constants
    ).fetchone()[0]


def write_json(path: Path, value: object) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    path.chmod(0o600)


def write_subsystem_seeds(
    connection: sqlite3.Connection, path: Path
) -> None:
    patterns = [
        ("lifecycle", r"sub_main|Basic|LevelInit"),
        ("input_player_camera", r"Player|ViewPort|GameUI"),
        ("entity_event_ai", r"Events|Animation"),
        ("terrain_world", r"Terrain|Map|Level"),
        ("spells_projectiles", r"Spell"),
        ("audio_music", r"Sound|AIL|Xmi"),
        ("render_presentation", r"Render|Graphics|Bitmap|Texture|Projection"),
        ("network", r"Network|port_net"),
        ("data_files_config", r"Read|Data|Config|filesystem"),
        ("regression_tooling", r"Compare|StateMonitor|Recorder|test"),
    ]
    rows = connection.execute(
        "SELECT usr, qualified_name, file, line FROM functions ORDER BY file, line"
    ).fetchall()
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            ["function_usr", "qualified_name", "file", "line", "seed_cluster"]
        )
        for usr, name, file_name, line in rows:
            haystack = f"{name} {file_name}"
            clusters = [
                label
                for label, pattern in patterns
                if re.search(pattern, haystack, re.IGNORECASE)
            ]
            writer.writerow(
                [usr, name, file_name, line, "|".join(clusters) or "unclassified"]
            )
    path.chmod(0o600)


def main() -> int:
    os.umask(0o077)
    args = parse_args()
    source_root = args.source_root.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    output_dir.chmod(0o700)

    target_sources = cmake_sources(source_root)
    compiled_sources = list(target_sources)
    if args.only:
        compiled_sources = [
            path
            for path in compiled_sources
            if any(value in path.as_posix() for value in args.only)
        ]
    if args.pilot:
        compiled_sources = compiled_sources[: args.pilot]

    all_units = all_project_units(source_root)
    unbuilt_units = sorted(set(all_units) - set(target_sources))
    unparsed_target_sources = sorted(
        set(target_sources) - set(compiled_sources)
    )
    compile_commands: dict[Path, list[str]] = {}
    argument_source = "fallback"
    if args.compile_commands:
        compile_commands = load_compile_commands(
            args.compile_commands.resolve()
        )
        argument_source = "compile_commands"

    database_path = output_dir / "atlas.sqlite"
    connection = initialize_database(database_path)
    connection.execute(
        "INSERT INTO metadata VALUES (?, ?)",
        ("schema_version", SCHEMA_VERSION),
    )
    connection.commit()

    index = cindex.Index.create()
    status_counts: Counter[str] = Counter()
    for ordinal, source in enumerate(compiled_sources, 1):
        arguments = compile_commands.get(source)
        source_argument_kind = argument_source
        if arguments is None:
            arguments = fallback_arguments(
                source_root, args.build_config, source
            )
            source_argument_kind = "fallback"
        else:
            arguments = [
                *arguments,
                *libclang_toolchain_arguments(source_root, source),
            ]
        print(
            f"[{ordinal}/{len(compiled_sources)}] {relative_or_absolute(source, source_root)}",
            flush=True,
        )
        status, errors, warnings = scan_translation_unit(
            connection,
            index,
            source,
            arguments,
            source_root,
            "compiled_target",
            source_argument_kind,
        )
        status_counts[status] += 1
        if errors:
            print(
                f"  coverage gap: {errors} error diagnostics, {warnings} warnings",
                flush=True,
            )

    for source in unbuilt_units:
        source_rel = relative_or_absolute(source, source_root)
        line_count = sum(
            1
            for _ in source.open(
                "r", encoding="utf-8", errors="replace"
            )
        )
        connection.execute(
            """
            INSERT OR REPLACE INTO translation_units
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                source_rel,
                "present_unbuilt",
                "not_parsed",
                line_count,
                sha256_file(source),
                "none",
                0,
                0,
            ),
        )
    for source in unparsed_target_sources:
        source_rel = relative_or_absolute(source, source_root)
        line_count = sum(
            1
            for _ in source.open(
                "r", encoding="utf-8", errors="replace"
            )
        )
        connection.execute(
            """
            INSERT OR REPLACE INTO translation_units
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                source_rel,
                "compiled_target",
                "not_parsed_by_filter",
                line_count,
                sha256_file(source),
                "none",
                0,
                0,
            ),
        )
    scan_macros(connection, source_root, set(target_sources))

    nested_status = run_text(
        ["git", "status", "--short"], cwd=source_root
    )
    nested_commit = run_text(
        ["git", "rev-parse", "HEAD"], cwd=source_root
    )
    diff_bytes = subprocess.run(
        ["git", "diff", "--binary"],
        cwd=source_root,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    ).stdout
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "classification": "PRIVATE_ANALYSIS",
        "source_root": str(source_root),
        "nested_git_commit": nested_commit,
        "nested_git_dirty": bool(nested_status),
        "nested_git_status": nested_status.splitlines(),
        "nested_git_diff_sha256": sha256_bytes(diff_bytes),
        "build_config": args.build_config,
        "compile_commands": (
            str(args.compile_commands.resolve())
            if args.compile_commands
            else None
        ),
        "argument_mode": argument_source,
        "clang_version": cindex.conf.lib.clang_getClangVersion()
        if False
        else run_text(["clang", "--version"], cwd=source_root).splitlines()[0],
        "python_version": sys.version.split()[0],
        "compiled_source_count": len(target_sources),
        "selected_source_count": len(compiled_sources),
        "present_unbuilt_source_count": len(unbuilt_units),
        "pilot": args.pilot,
        "only_filters": args.only,
    }
    counts = {
        table: table_count(connection, table)
        for table in (
            "translation_units",
            "diagnostics",
            "functions",
            "parameters",
            "records",
            "fields",
            "globals",
            "calls",
            "state_accesses",
            "literals",
            "bulk_operations",
            "includes",
            "macros",
        )
    }
    parsed_units = connection.execute(
        """
        SELECT COUNT(*) FROM translation_units
        WHERE build_membership='compiled_target'
          AND parse_status='parsed'
        """
    ).fetchone()[0]
    units_with_errors = connection.execute(
        """
        SELECT COUNT(*) FROM translation_units
        WHERE build_membership='compiled_target'
          AND error_count > 0
        """
    ).fetchone()[0]
    unresolved_calls = connection.execute(
        """
        SELECT COUNT(*) FROM calls
        WHERE edge_kind='calls_indirect_or_unresolved'
        """
    ).fetchone()[0]
    possible_accesses = connection.execute(
        """
        SELECT COUNT(*) FROM state_accesses
        WHERE confidence='possible'
        """
    ).fetchone()[0]
    if argument_source != "compile_commands":
        claim_state = "PROVISIONAL"
    elif len(compiled_sources) != len(target_sources):
        claim_state = "EXACT_BUILD_PARTIAL"
    elif units_with_errors:
        claim_state = "EXACT_BUILD_WITH_PARSE_GAPS"
    else:
        claim_state = "STATIC_BASELINE_COMPLETE"
    coverage = {
        "claim_state": claim_state,
        "compiled_translation_units_denominator": len(target_sources),
        "compiled_translation_units_selected": len(compiled_sources),
        "compiled_translation_units_parsed_without_errors": parsed_units,
        "compiled_translation_units_with_error_gaps": units_with_errors,
        "present_unbuilt_translation_units": len(unbuilt_units),
        "unresolved_direct_or_indirect_call_edges": unresolved_calls,
        "possible_pointer_or_argument_access_edges": possible_accesses,
        "status_counts": dict(status_counts),
        "known_static_limitations": [
            "pointer aliasing and type punning",
            "packed structures and union views",
            "bulk memory region effects",
            "function pointers and data-driven dispatch",
            "loaded table overwrite and patch lineage",
            "platform and preprocessor variants outside this manifest",
            "runtime reachability and causal semantics",
        ],
    }
    write_json(output_dir / "manifest.json", manifest)
    write_json(output_dir / "coverage.json", coverage)
    write_json(output_dir / "summary.json", {"counts": counts})
    write_subsystem_seeds(
        connection, output_dir / "subsystem-seeds.csv"
    )
    connection.execute("PRAGMA wal_checkpoint(FULL)")
    connection.close()
    database_path.chmod(0o600)
    wal = database_path.with_name(database_path.name + "-wal")
    shm = database_path.with_name(database_path.name + "-shm")
    if wal.exists():
        wal.chmod(0o600)
    if shm.exists():
        shm.chmod(0o600)
    print(json.dumps({"coverage": coverage, "counts": counts}, indent=2))
    return 0 if units_with_errors == 0 else 2


if __name__ == "__main__":
    raise SystemExit(main())
