#!/usr/bin/env python3
"""Bounded shell discovery and command-context checks for repository gates.

This is deliberately not a general shell interpreter.  It recognizes the
static syntax named in tests/shell-scanner-coverage.sh and refuses malformed
input rather than recovering from it.  Heredoc bodies and quoted arguments are
data.  Commands inside $(...) and backticks are scanned as commands.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import re
import stat
import subprocess
import sys
from pathlib import Path


SHELL_SHEBANG = re.compile(
    rb"^#![ \t]*(?:/bin/(?:ba)?sh|/usr/bin/(?:ba)?sh|/usr/bin/env[ \t]+(?:-S[ \t]+)?(?:ba)?sh)(?:[ \t].*)?$"
)
EXCLUDED_PARTS = {"target", ".git"}
BUILDER = "scripts/require-built-commands.sh"
TARGET_USE = re.compile(r"(?:^|/)target/debug/[A-Za-z]")
ASSIGNMENT = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$", re.S)
VARIABLE_WORD = re.compile(r"^\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*))$")
MAX_SUBSTITUTION_NESTING = 64
MAX_FUNCTION_EXPANSIONS = 1024
FUNCTION_EXPANSION_FACTOR = 64
MAX_FUNCTION_DEPTH = 128
MIN_PROCESSED_COMMANDS = 4096
PROCESSED_COMMAND_FACTOR = 128


class ScanError(Exception):
    pass


@dataclasses.dataclass(frozen=True)
class Token:
    kind: str
    text: str
    raw: str
    line: int
    index: int
    quoted_command_word: bool = False


@dataclasses.dataclass
class Command:
    words: list[Token]
    assignments: list[Token]
    line: int
    index: int
    function: str | None = None
    uncertain: bool = False

    @property
    def name(self) -> str:
        return self.words[0].text if self.words else ""


@dataclasses.dataclass
class Pipeline:
    commands: list[Command]
    line: int


@dataclasses.dataclass
class Analysis:
    path: str
    tokens: list[Token]
    commands: list[Command]
    pipelines: list[Pipeline]
    negations: list[Token]
    unsafe_negations: list[Token]
    unsupported: list[tuple[int, str]]


@dataclasses.dataclass(frozen=True)
class FunctionSummary:
    direct_builder: bool
    direct_target: bool
    invoked: frozenset[str]
    modified: frozenset[str]
    builder_values: frozenset[str]
    target_values: frozenset[str]
    refusal: str | None


def tracked_shell_files(root: Path) -> list[Path]:
    try:
        raw = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout
    except subprocess.CalledProcessError as exc:
        raise ScanError(f"cannot enumerate tracked files: {exc.stderr.decode(errors='replace').strip()}") from exc
    result: list[Path] = []
    for encoded in raw.split(b"\0"):
        if not encoded:
            continue
        relative = Path(os.fsdecode(encoded))
        if any(part in EXCLUDED_PARTS for part in relative.parts):
            continue
        path = root / relative
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            # The index can still name a tracked deletion while a change is
            # under review. There are no bytes in the working tree to scan.
            continue
        except OSError as exc:
            raise ScanError(f"cannot inspect {relative}: {exc}") from exc
        if not stat.S_ISREG(mode):
            continue
        if path.suffix == ".sh":
            result.append(relative)
            continue
        try:
            with path.open("rb") as handle:
                first = handle.readline().rstrip(b"\r\n")
        except OSError as exc:
            raise ScanError(f"cannot read {relative}: {exc}") from exc
        if SHELL_SHEBANG.fullmatch(first):
            result.append(relative)
    return sorted(result, key=lambda item: os.fsencode(str(item)))


class Lexer:
    OPERATORS = ("<<<", "<<-", "<<", ">>", "<&", ">&", "<>", ";;&", ";;", ";&", "&&", "||", "|&", "[[", "]]", "((", "))", "|", ";", "&", "(", ")", "{", "}", "<", ">", "!")

    def __init__(self, source: str, path: str, base_line: int = 1):
        self.source = source
        self.path = path
        self.i = 0
        self.line = base_line
        self.tokens: list[Token] = []
        self.substitutions: list[tuple[str, int, int]] = []
        self.pending_heredocs: list[tuple[str, bool, int, bool, int]] = []

    def error(self, message: str) -> ScanError:
        return ScanError(f"{self.path}:{self.line}: {message}")

    def emit(self, kind: str, text: str, raw: str, line: int, quoted_command_word: bool = False) -> None:
        self.tokens.append(Token(kind, text, raw, line, len(self.tokens), quoted_command_word))

    def quoted(self, quote: str) -> str:
        start_line = self.line
        self.i += 1
        out: list[str] = []
        while self.i < len(self.source):
            char = self.source[self.i]
            if char == quote:
                self.i += 1
                return "".join(out)
            if char == "\n":
                self.line += 1
            if quote == '"' and char == "\\" and self.i + 1 < len(self.source):
                nxt = self.source[self.i + 1]
                if nxt == "\n":
                    self.line += 1
                    self.i += 2
                    continue
                if nxt in '$`"\\':
                    out.append(nxt)
                    self.i += 2
                    continue
            if quote == '"' and self.source.startswith("$(", self.i):
                body, body_line, arithmetic = self.command_substitution()
                if arithmetic:
                    self.collect_embedded_expansions(body, body_line, len(self.tokens))
                else:
                    self.substitutions.append((body, body_line, len(self.tokens)))
                out.append("$()")
                continue
            if quote == '"' and self.source.startswith("${", self.i):
                body, body_line, raw = self.parameter_expansion()
                self.collect_embedded_expansions(body, body_line, len(self.tokens))
                out.append(raw)
                continue
            if quote == '"' and char == "`":
                body, body_line = self.backtick_substitution()
                self.substitutions.append((body, body_line, len(self.tokens)))
                out.append("$()")
                continue
            out.append(char)
            self.i += 1
        raise ScanError(f"{self.path}:{start_line}: unterminated {quote} quote")

    def ansi_c_quoted(self) -> str:
        start_line = self.line
        self.i += 2
        out: list[str] = []
        while self.i < len(self.source):
            char = self.source[self.i]
            if char == "'":
                self.i += 1
                return "".join(out)
            if char == "\\" and self.i + 1 < len(self.source):
                out.append(self.source[self.i + 1])
                self.i += 2
                continue
            if char == "\n":
                self.line += 1
            out.append(char)
            self.i += 1
        raise ScanError(f"{self.path}:{start_line}: unterminated ANSI-C quote")

    def comment_start(self, begin: int) -> bool:
        if self.source[self.i] != "#":
            return False
        if self.i >= 2 and self.source[self.i - 2 : self.i] == "${":
            return False
        return self.i == begin or self.source[self.i - 1] in " \t\r\n;|&({"

    def command_substitution(self) -> tuple[str, int, bool]:
        start_line = self.line
        arithmetic = self.source.startswith("$((", self.i)
        opener = 3 if arithmetic else 2
        self.i += opener
        begin = self.i
        depth = 1
        quote: str | None = None
        comment = False
        while self.i < len(self.source):
            char = self.source[self.i]
            if comment:
                if char == "\n":
                    comment = False
                    self.line += 1
                self.i += 1
                continue
            if quote:
                if char == "\\" and quote == '"' and self.i + 1 < len(self.source):
                    if self.source[self.i + 1] == "\n":
                        self.line += 1
                    self.i += 2
                    continue
                if char == quote:
                    quote = None
                if char == "\n":
                    self.line += 1
                self.i += 1
                continue
            if char in "'\"":
                quote = char
                self.i += 1
                continue
            if not arithmetic and self.comment_start(begin):
                comment = True
                self.i += 1
                continue
            if char == "\n":
                self.line += 1
            if arithmetic and char == "(":
                depth += 1
                self.i += 1
                continue
            if not arithmetic:
                if self.source.startswith("$((", self.i):
                    depth += 2
                    self.i += 3
                    continue
                if self.source.startswith("$(", self.i):
                    depth += 1
                    self.i += 2
                    continue
                if char == "(":
                    depth += 1
                    self.i += 1
                    continue
            if char == ")":
                depth -= 1
                if depth == 0:
                    body = self.source[begin:self.i]
                    self.i += 1
                    if arithmetic:
                        if self.i >= len(self.source) or self.source[self.i] != ")":
                            raise self.error("unterminated arithmetic region")
                        self.i += 1
                        return body, start_line, True
                    return body, start_line, False
            self.i += 1
        kind = "arithmetic region" if arithmetic else "command substitution"
        raise ScanError(f"{self.path}:{start_line}: unterminated {kind}")

    def parameter_expansion(self) -> tuple[str, int, str]:
        start_line = self.line
        start = self.i
        self.i += 2
        begin = self.i
        depth = 1
        quote: str | None = None
        while self.i < len(self.source):
            char = self.source[self.i]
            if quote:
                if char == "\\" and quote == '"' and self.i + 1 < len(self.source):
                    if self.source[self.i + 1] == "\n":
                        self.line += 1
                    self.i += 2
                    continue
                if char == quote:
                    quote = None
                if char == "\n":
                    self.line += 1
                self.i += 1
                continue
            if char in "'\"":
                quote = char
                self.i += 1
                continue
            if char == "\\" and self.i + 1 < len(self.source):
                if self.source[self.i + 1] == "\n":
                    self.line += 1
                self.i += 2
                continue
            if self.source.startswith("${", self.i):
                depth += 1
                self.i += 2
                continue
            if char == "}":
                depth -= 1
                if depth == 0:
                    body = self.source[begin:self.i]
                    self.i += 1
                    return body, start_line, self.source[start:self.i]
            if char == "\n":
                self.line += 1
            self.i += 1
        raise ScanError(f"{self.path}:{start_line}: unterminated parameter expansion")

    def collect_embedded_expansions(
        self,
        fragment: str,
        base_line: int,
        anchor: int,
        nesting: int = 0,
        honor_quotes: bool = True,
    ) -> None:
        if nesting > MAX_SUBSTITUTION_NESTING:
            raise ScanError(
                f"{self.path}:{base_line}: parameter expansion nesting exceeds "
                f"{MAX_SUBSTITUTION_NESTING}"
            )
        probe = Lexer(fragment, self.path, base_line)
        quote: str | None = None
        while probe.i < len(probe.source):
            char = probe.source[probe.i]
            if honor_quotes and quote == "'":
                if char == "'":
                    quote = None
                if char == "\n":
                    probe.line += 1
                probe.i += 1
                continue
            if honor_quotes and quote == '"' and char == '"':
                quote = None
                probe.i += 1
                continue
            if honor_quotes and quote is None and char in "'\"":
                quote = char
                probe.i += 1
                continue
            if char == "\\" and probe.i + 1 < len(probe.source):
                if probe.source[probe.i + 1] == "\n":
                    probe.line += 1
                probe.i += 2
                continue
            if probe.source.startswith("$(", probe.i):
                body, line, arithmetic = probe.command_substitution()
                if arithmetic:
                    self.collect_embedded_expansions(
                        body, line, anchor, nesting + 1, honor_quotes
                    )
                else:
                    self.substitutions.append((body, line, anchor))
                continue
            if probe.source.startswith("${", probe.i):
                body, line, _ = probe.parameter_expansion()
                self.collect_embedded_expansions(
                    body, line, anchor, nesting + 1, honor_quotes
                )
                continue
            if char == "`":
                body, line = probe.backtick_substitution()
                self.substitutions.append((body, line, anchor))
                continue
            if char == "\n":
                probe.line += 1
            probe.i += 1

    def process_substitution(self) -> tuple[str, int]:
        start_line = self.line
        self.i += 2
        begin = self.i
        depth = 1
        quote: str | None = None
        comment = False
        while self.i < len(self.source):
            char = self.source[self.i]
            if comment:
                if char == "\n":
                    comment = False
                    self.line += 1
                self.i += 1
                continue
            if quote:
                if char == "\\" and quote == '"' and self.i + 1 < len(self.source):
                    if self.source[self.i + 1] == "\n":
                        self.line += 1
                    self.i += 2
                    continue
                if char == quote:
                    quote = None
                if char == "\n":
                    self.line += 1
                self.i += 1
                continue
            if char in "'\"":
                quote = char
                self.i += 1
                continue
            if self.comment_start(begin):
                comment = True
                self.i += 1
                continue
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth == 0:
                    body = self.source[begin:self.i]
                    self.i += 1
                    return body, start_line
            if char == "\n":
                self.line += 1
            self.i += 1
        raise ScanError(f"{self.path}:{start_line}: unterminated process substitution")

    def backtick_substitution(self) -> tuple[str, int]:
        start_line = self.line
        self.i += 1
        begin = self.i
        while self.i < len(self.source):
            char = self.source[self.i]
            if char == "\\" and self.i + 1 < len(self.source):
                if self.source[self.i + 1] == "\n":
                    self.line += 1
                self.i += 2
                continue
            if char == "`":
                body = self.source[begin:self.i]
                self.i += 1
                return body, start_line
            if char == "\n":
                self.line += 1
            self.i += 1
        raise ScanError(f"{self.path}:{start_line}: unterminated backtick substitution")

    def word(self) -> tuple[str, int, bool]:
        line = self.line
        out: list[str] = []
        quoted_command_word = False
        while self.i < len(self.source):
            char = self.source[self.i]
            if char in " \t\r\n":
                break
            if char == "#" and not out:
                break
            matching_operator = next((op for op in self.OPERATORS if self.source.startswith(op, self.i)), None)
            if matching_operator and not (matching_operator == "!" and (out or (self.i + 1 < len(self.source) and self.source[self.i + 1] not in " \t\r\n("))):
                break
            if self.source.startswith("$'", self.i):
                if not out:
                    quoted_command_word = True
                out.append(self.ansi_c_quoted())
                continue
            if char in "'\"":
                if not out:
                    quoted_command_word = True
                out.append(self.quoted(char))
                continue
            if self.source.startswith("$(", self.i):
                body, body_line, arithmetic = self.command_substitution()
                if arithmetic:
                    self.collect_embedded_expansions(body, body_line, len(self.tokens))
                elif body:
                    self.substitutions.append((body, body_line, len(self.tokens)))
                out.append("$()")
                continue
            if self.source.startswith("${", self.i):
                body, body_line, raw = self.parameter_expansion()
                self.collect_embedded_expansions(body, body_line, len(self.tokens))
                out.append(raw)
                continue
            if char == "`":
                body, body_line = self.backtick_substitution()
                self.substitutions.append((body, body_line, len(self.tokens)))
                out.append("$()")
                continue
            if char == "\\" and self.i + 1 < len(self.source):
                nxt = self.source[self.i + 1]
                if nxt == "\n":
                    self.line += 1
                    self.i += 2
                    continue
                out.append(nxt)
                self.i += 2
                continue
            out.append(char)
            self.i += 1
        return "".join(out), line, quoted_command_word

    def skip_heredocs(self) -> None:
        pending = self.pending_heredocs
        self.pending_heredocs = []
        for delimiter, tabs, declared, quoted, anchor in pending:
            found = False
            body_start = self.i
            body_line_number = self.line
            while self.i < len(self.source):
                line_start = self.i
                end = self.source.find("\n", self.i)
                if end < 0:
                    end = len(self.source)
                body_line = self.source[self.i:end]
                comparison = body_line.lstrip("\t") if tabs else body_line
                self.i = end
                if comparison == delimiter:
                    found = True
                    if not quoted:
                        self.collect_embedded_expansions(
                            self.source[body_start:line_start],
                            body_line_number,
                            anchor,
                            honor_quotes=False,
                        )
                    if self.i < len(self.source):
                        self.i += 1
                        self.line += 1
                    break
                if self.i < len(self.source):
                    self.i += 1
                    self.line += 1
            if not found:
                raise ScanError(f"{self.path}:{declared}: unterminated heredoc {delimiter!r}")

    def run(self) -> tuple[list[Token], list[tuple[str, int, int]]]:
        expect_delimiter: tuple[bool, int] | None = None
        while self.i < len(self.source):
            char = self.source[self.i]
            if char in " \t\r":
                self.i += 1
                continue
            if char == "\\" and self.i + 1 < len(self.source) and self.source[self.i + 1] == "\n":
                self.i += 2
                self.line += 1
                continue
            if char == "\n":
                self.emit("newline", "\n", "\n", self.line)
                self.i += 1
                self.line += 1
                if self.pending_heredocs:
                    self.skip_heredocs()
                continue
            if char == "#":
                end = self.source.find("\n", self.i)
                self.i = len(self.source) if end < 0 else end
                continue
            if self.source.startswith("<(", self.i) or self.source.startswith(">(", self.i):
                start = self.i
                body, body_line = self.process_substitution()
                self.substitutions.append((body, body_line, len(self.tokens)))
                self.emit("word", "$()", self.source[start:self.i], body_line)
                continue
            operator = next((op for op in self.OPERATORS if self.source.startswith(op, self.i)), None)
            if operator == "!" and self.i + 1 < len(self.source) and self.source[self.i + 1] not in " \t\r\n(":
                operator = None
            if operator:
                self.emit("op", operator, operator, self.line)
                self.i += len(operator)
                if operator in {"<<", "<<-"}:
                    expect_delimiter = (operator == "<<-", self.line)
                continue
            word_start = self.i
            value, line, quoted_command_word = self.word()
            if not value and self.i < len(self.source) and self.source[self.i] == "#":
                continue
            if not value and self.i == word_start:
                raise self.error(f"unsupported token starting with {self.source[self.i:self.i+8]!r}")
            raw = self.source[word_start:self.i]
            self.emit("word", value, raw, line, quoted_command_word)
            if expect_delimiter:
                tabs, declared = expect_delimiter
                quoted = raw != value
                self.pending_heredocs.append((value, tabs, declared, quoted, self.tokens[-1].index))
                expect_delimiter = None
        if expect_delimiter:
            raise self.error("heredoc operator has no delimiter")
        if self.pending_heredocs:
            self.skip_heredocs()
        return self.tokens, self.substitutions


REDIRECTIONS = {"<", ">", ">>", "<&", ">&", "<>", "<<<", "<<", "<<-"}
SEPARATORS = {";", "\n", "&", "&&", "||", "|", "|&", "(", ")", "{", "}", ";;", ";&", ";;&"}
RESERVED = {"if", "then", "elif", "else", "fi", "while", "until", "do", "done", "for", "select", "in", "case", "esac", "function", "time", "coproc"}


def function_ranges(tokens: list[Token]) -> tuple[list[tuple[int, int, str]], list[tuple[int, str]], set[int]]:
    ranges: list[tuple[int, int, str]] = []
    unsupported: list[tuple[int, str]] = []
    declarations: set[int] = set()
    brace_stack: list[int] = []
    brace_closers: dict[int, int] = {}
    for index, token in enumerate(tokens):
        if token.kind == "op" and token.text == "{":
            brace_stack.append(index)
        elif token.kind == "op" and token.text == "}" and brace_stack:
            brace_closers[brace_stack.pop()] = index
    i = 0
    while i < len(tokens):
        name: str | None = None
        opener: int | None = None
        if i + 3 < len(tokens) and (
            tokens[i].kind == "word"
            and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[i].text)
            and tokens[i + 1].kind == "op"
            and tokens[i + 1].text == "("
            and tokens[i + 2].kind == "op"
            and tokens[i + 2].text == ")"
            and tokens[i + 3].kind == "op"
            and tokens[i + 3].text == "{"
        ):
            name = tokens[i].text
            opener = i + 3
        elif (
            i + 2 < len(tokens)
            and tokens[i].kind == "word"
            and not tokens[i].quoted_command_word
            and tokens[i].text == "function"
            and tokens[i + 1].kind == "word"
            and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[i + 1].text)
        ):
            name = tokens[i + 1].text
            if tokens[i + 2].kind == "op" and tokens[i + 2].text == "{":
                opener = i + 2
            elif (
                i + 4 < len(tokens)
                and tokens[i + 2].kind == "op"
                and tokens[i + 2].text == "("
                and tokens[i + 3].kind == "op"
                and tokens[i + 3].text == ")"
                and tokens[i + 4].kind == "op"
                and tokens[i + 4].text == "{"
            ):
                opener = i + 4
        if name is not None and opener is not None:
            closer = brace_closers.get(opener)
            if closer is None:
                unsupported.append((tokens[i].line, f"unterminated function {name}"))
            else:
                ranges.append((opener + 1, closer, name))
                declarations.update(range(i, opener + 1))
                # Keep scanning inside the body. Nested declarations belong to
                # their own function and cannot lend evidence to the caller.
        i += 1
    ranges.sort(key=lambda item: (item[0], -(item[1] - item[0])))
    return ranges, unsupported, declarations


def uncertain_control_tokens(tokens: list[Token], syntax_tokens: set[int]) -> set[int]:
    uncertain: set[int] = set()
    stack: list[str] = []
    opener_to_closer = {"if": "fi", "while": "done", "until": "done", "for": "done", "select": "done", "case": "esac"}
    for token in tokens:
        if stack:
            uncertain.add(token.index)
        if token.index not in syntax_tokens:
            continue
        if token.text in opener_to_closer:
            stack.append(opener_to_closer[token.text])
            uncertain.add(token.index)
        elif stack and token.text == stack[-1]:
            stack.pop()
    return uncertain


def parse_commands(tokens: list[Token]) -> tuple[list[Command], list[Pipeline], list[tuple[int, str]], set[int], set[int]]:
    ranges, unsupported, declaration_tokens = function_ranges(tokens)
    range_starts: dict[int, list[tuple[int, str]]] = {}
    for start, end, name in ranges:
        range_starts.setdefault(start, []).append((end, name))
    active_ranges: list[tuple[int, str]] = []
    function_owners: list[str | None] = [None] * len(tokens)
    for index in range(len(tokens)):
        while active_ranges and active_ranges[-1][0] <= index:
            active_ranges.pop()
        for item in sorted(range_starts.get(index, []), reverse=True):
            active_ranges.append(item)
        if active_ranges:
            function_owners[index] = active_ranges[-1][1]
    syntax_tokens: set[int] = set()
    negation_tokens: set[int] = set()
    commands: list[Command] = []
    pipelines: list[Pipeline] = []
    pipeline_commands: list[Command] = []
    words: list[Token] = []
    assignments: list[Token] = []
    start_index = 0
    skip_target = False
    command_started = False
    bracket = False
    awaiting_pipeline_command = False
    rhs_uncertain = False
    pipeline_groups: list[str] = []
    def function_at(index: int) -> str | None:
        return function_owners[index] if index < len(function_owners) else None

    def finish_command() -> None:
        nonlocal words, assignments, command_started, start_index, skip_target, awaiting_pipeline_command, rhs_uncertain
        if words or assignments:
            token = words[0] if words else assignments[0]
            command = Command(
                words[:],
                assignments[:],
                token.line,
                start_index,
                function_at(start_index),
                rhs_uncertain,
            )
            commands.append(command)
            pipeline_commands.append(command)
            awaiting_pipeline_command = False
        words = []
        assignments = []
        command_started = False
        skip_target = False
        rhs_uncertain = False

    def finish_pipeline() -> None:
        nonlocal pipeline_commands
        if pipeline_commands:
            pipelines.append(Pipeline(pipeline_commands[:], pipeline_commands[0].line))
        pipeline_commands = []

    i = 0
    while i < len(tokens):
        token = tokens[i]
        text = token.text
        if token.index in declaration_tokens:
            i += 1
            continue
        if bracket:
            if text == "]]":
                bracket = False
            else:
                words.append(token)
            i += 1
            continue
        if skip_target:
            if token.kind == "word":
                skip_target = False
                i += 1
                continue
            unsupported.append((token.line, f"redirection {tokens[i-1].text} has no static target"))
            skip_target = False
        if token.kind == "op" and text in REDIRECTIONS:
            skip_target = True
            i += 1
            continue
        if token.kind == "op" and text == "!" and not command_started:
            negation_tokens.add(token.index)
            i += 1
            continue
        if token.kind == "op" and text in {"|", "|&"}:
            finish_command()
            awaiting_pipeline_command = True
            i += 1
            continue
        if text == "\n" and awaiting_pipeline_command:
            i += 1
            continue
        if token.kind == "op" and pipeline_groups and text == pipeline_groups[-1]:
            finish_command()
            pipeline_groups.pop()
            i += 1
            continue
        if (token.kind in {"op", "newline"} and text in {";", "\n", "&", "&&", "||", ")", "}", ";;", ";&", ";;&"}):
            finish_command()
            if not pipeline_groups:
                finish_pipeline()
            if text in {"&&", "||"}:
                rhs_uncertain = True
            i += 1
            continue
        if token.kind == "op" and text in {"(", "{"}:
            # The name in `name() {` declares a function; it is not invoked.
            if text == "(" and i > 0 and i + 1 < len(tokens) and tokens[i + 1].kind == "op" and tokens[i + 1].text == ")" and tokens[i - 1].kind == "word":
                words = []
                assignments = []
                command_started = False
                i += 2
                continue
            if awaiting_pipeline_command or pipeline_groups:
                pipeline_groups.append(")" if text == "(" else "}")
                i += 1
                continue
            finish_command()
            finish_pipeline()
            i += 1
            continue
        if token.kind != "word":
            i += 1
            continue
        if not command_started and text in RESERVED and not token.quoted_command_word:
            syntax_tokens.add(token.index)
            i += 1
            continue
        if not command_started and ASSIGNMENT.fullmatch(text):
            assignments.append(token)
            if not assignments[:-1]:
                start_index = token.index
            i += 1
            continue
        if not command_started:
            start_index = token.index
            command_started = True
            awaiting_pipeline_command = False
            words.append(token)
            bracket = text == "[["
        else:
            words.append(token)
        i += 1
    finish_command()
    finish_pipeline()
    uncertain_tokens = uncertain_control_tokens(tokens, syntax_tokens)
    for command in commands:
        command.uncertain = command.uncertain or command.index in uncertain_tokens
    return commands, pipelines, unsupported, syntax_tokens, negation_tokens


def command_depths(tokens: list[Token]) -> list[tuple[int, int]]:
    depths: list[tuple[int, int]] = []
    paren = brace = 0
    for token in tokens:
        depths.append((paren, brace))
        if token.kind == "op" and token.text == "(":
            paren += 1
        elif token.kind == "op" and token.text == ")" and paren:
            paren -= 1
        elif token.kind == "op" and token.text == "{":
            brace += 1
        elif token.kind == "op" and token.text == "}" and brace:
            brace -= 1
    return depths


def next_list_separators(tokens: list[Token], depths: list[tuple[int, int]]) -> list[str | None]:
    result: list[str | None] = [None] * len(tokens)
    nearest: dict[tuple[int, int], str] = {}
    separators = {"||", "&&", ";", "\n", "&", ";;", ";&", ";;&"}
    for index in range(len(tokens) - 1, -1, -1):
        depth = depths[index]
        result[index] = nearest.get(depth)
        if tokens[index].kind in {"op", "newline"} and tokens[index].text in separators:
            nearest[depth] = tokens[index].text
    return result


def conditional_contexts(tokens: list[Token], syntax_tokens: set[int]) -> list[bool]:
    result: list[bool] = []
    if_stack: list[bool] = []
    loop_stack: list[bool] = []
    active = 0
    for token in tokens:
        result.append(active > 0)
        if token.index not in syntax_tokens:
            continue
        text = token.text
        if text == "if":
            if_stack.append(True)
            active += 1
        elif text == "elif" and if_stack:
            if not if_stack[-1]:
                if_stack[-1] = True
                active += 1
        elif text in {"then", "else"} and if_stack and if_stack[-1]:
            if_stack[-1] = False
            active -= 1
        elif text == "fi" and if_stack:
            if if_stack.pop():
                active -= 1
        elif text in {"while", "until"}:
            loop_stack.append(True)
            active += 1
        elif text == "do" and loop_stack and loop_stack[-1]:
            loop_stack[-1] = False
            active -= 1
        elif text == "done" and loop_stack:
            if loop_stack.pop():
                active -= 1
    return result


def analyze_text(source: str, path: str, base_line: int = 1, substitution_nesting: int = 0) -> Analysis:
    if substitution_nesting > MAX_SUBSTITUTION_NESTING:
        raise ScanError(f"{path}:{base_line}: command substitution nesting exceeds {MAX_SUBSTITUTION_NESTING}")
    lexer = Lexer(source, path, base_line)
    tokens, substitutions = lexer.run()
    commands, pipelines, unsupported, syntax_tokens, negation_tokens = parse_commands(tokens)
    ranges, _, _ = function_ranges(tokens)
    uncertain_tokens = uncertain_control_tokens(tokens, syntax_tokens)
    depths = command_depths(tokens)
    next_separators = next_list_separators(tokens, depths)
    in_condition = conditional_contexts(tokens, syntax_tokens)
    negations: list[Token] = []
    unsafe: list[Token] = []
    bracket_depth = 0
    for token in tokens:
        if token.kind == "op" and token.text == "[[":
            bracket_depth += 1
        elif token.kind == "op" and token.text == "]]" and bracket_depth:
            bracket_depth -= 1
        elif token.kind == "op" and token.text == "!" and bracket_depth == 0 and token.index in negation_tokens:
            negations.append(token)
            if not in_condition[token.index] and next_separators[token.index] not in {"||", "&&"}:
                unsafe.append(token)
    for body, line, anchor in substitutions:
        nested_analysis = analyze_text(body, path, line, substitution_nesting + 1)
        inherited_function = next(
            (name for start, end, name in ranges if start <= anchor < end), None
        )
        inherited_uncertain = anchor in uncertain_tokens
        for command in nested_analysis.commands:
            if command.function is None:
                command.function = inherited_function
            command.uncertain = command.uncertain or inherited_uncertain
        commands.extend(nested_analysis.commands)
        pipelines.extend(nested_analysis.pipelines)
        unsupported.extend(nested_analysis.unsupported)
        negations.extend(nested_analysis.negations)
        unsafe.extend(nested_analysis.unsafe_negations)
    commands.sort(key=lambda item: (item.line, item.index))
    pipelines.sort(key=lambda item: item.line)
    negations.sort(key=lambda item: (item.line, item.index))
    unsafe.sort(key=lambda item: (item.line, item.index))
    return Analysis(path, tokens, commands, pipelines, negations, unsafe, unsupported)


def analyze_file(path: Path, display: str) -> Analysis:
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise ScanError(f"cannot read {display}: {exc}") from exc
    try:
        return analyze_text(source, display)
    except RecursionError as exc:
        raise ScanError(f"{display}: parser recursion exceeded its bounded grammar") from exc


def command_has_target_use(command: Command) -> bool:
    if not command.words:
        return False
    return any(TARGET_USE.search(word.text) for word in command.words)


def variable_reference(token: Token, allow_quoted: bool = False) -> str | None:
    raw = token.raw
    if allow_quoted and len(raw) >= 2 and raw[0] == raw[-1] == '"':
        raw = raw[1:-1]
    match = VARIABLE_WORD.fullmatch(raw)
    return (match.group(1) or match.group(2)) if match else None


def builder_invocation(command: Command, bindings: dict[str, bool | None]) -> bool:
    if not command.words:
        return False
    name = command.words[0].text
    if name.endswith(BUILDER):
        return True
    if name in {"bash", "sh", "source", "."} and len(command.words) > 1:
        argument = command.words[1]
        if argument.text.endswith(BUILDER):
            return True
    variable = variable_reference(command.words[0], allow_quoted=True)
    if variable and bindings.get(variable) is True:
        return True
    return False


def check_builder(analysis: Analysis) -> dict[str, object]:
    functions: dict[str, list[Command]] = {}
    top_level: list[Command] = []
    for command in analysis.commands:
        if command.function:
            functions.setdefault(command.function, []).append(command)
        else:
            top_level.append(command)
    function_starts = {
        name: min((command.line, command.index) for command in body)
        for name, body in functions.items()
        if body
    }

    bindings: dict[str, bool | None] = {}
    target_bindings: dict[str, bool | None] = {}
    uses: list[tuple[int, int]] = []
    builds: list[tuple[int, int]] = []
    unsupported: list[dict[str, object]] = []
    sequence = 0
    function_expansions = 0
    function_expansion_limit = max(
        MAX_FUNCTION_EXPANSIONS, len(analysis.commands) * FUNCTION_EXPANSION_FACTOR
    )
    expansion_refused = False
    processed_commands = 0
    processed_command_limit = max(
        MIN_PROCESSED_COMMANDS, len(analysis.commands) * PROCESSED_COMMAND_FACTOR
    )
    processing_refused = False

    def assignment_value(token: Token) -> tuple[str, str]:
        match = ASSIGNMENT.fullmatch(token.text)
        assert match
        return match.group(1), match.group(2)

    def referenced_target_state(command: Command, current_targets: dict[str, bool | None]) -> bool | None:
        state: bool | None = False
        for word in command.words:
            variable = variable_reference(word, allow_quoted=True)
            if variable and variable in current_targets:
                if current_targets[variable] is True:
                    return True
                if current_targets[variable] is None:
                    state = None
        return state

    def has_target_use(command: Command, current_targets: dict[str, bool | None]) -> bool:
        if command_has_target_use(command):
            return True
        return referenced_target_state(command, current_targets) is True

    def direct_assignment_states(command: Command) -> tuple[bool, bool]:
        tokens = command.assignments[:]
        if command.name in {"local", "declare", "readonly", "export"}:
            tokens.extend(word for word in command.words[1:] if ASSIGNMENT.fullmatch(word.text))
        builder_assignment = False
        target_assignment = False
        for token in tokens:
            _, value = assignment_value(token)
            builder_assignment = builder_assignment or value.endswith(BUILDER)
            target_assignment = target_assignment or bool(TARGET_USE.search(value))
        return builder_assignment, target_assignment

    summary_cache: dict[str, FunctionSummary] = {}

    def function_summary(
        name: str, seen: frozenset[str] = frozenset(), depth: int = 0
    ) -> FunctionSummary:
        """Return direct build/use, invoked/modified variables, and uncertainty."""
        if name in summary_cache:
            return summary_cache[name]
        if depth >= MAX_FUNCTION_DEPTH:
            return FunctionSummary(
                False,
                False,
                frozenset(),
                frozenset(),
                frozenset(),
                frozenset(),
                f"function summary depth exceeds {MAX_FUNCTION_DEPTH} calls",
            )
        if name in seen:
            return FunctionSummary(
                False,
                False,
                frozenset(),
                frozenset(),
                frozenset(),
                frozenset(),
                "recursive function summary is unsupported",
            )
        has_builder = has_target = False
        refusal: str | None = None
        invoked: set[str] = set()
        modified: set[str] = set()
        builder_values: set[str] = set()
        target_values: set[str] = set()
        for command in functions.get(name, []):
            has_builder = has_builder or builder_invocation(command, {})
            has_target = has_target or command_has_target_use(command)
            if command.words:
                variable = variable_reference(command.words[0], allow_quoted=True)
                if variable:
                    invoked.add(variable)
            for word in command.words:
                variable = variable_reference(word, allow_quoted=True)
                if variable:
                    invoked.add(variable)
            assignment_tokens = command.assignments[:]
            if command.name in {"local", "declare", "readonly", "export"}:
                assignment_tokens.extend(
                    word for word in command.words[1:] if ASSIGNMENT.fullmatch(word.text)
                )
            for token in assignment_tokens:
                assigned_name, value = assignment_value(token)
                modified.add(assigned_name)
                if value.endswith(BUILDER):
                    builder_values.add(assigned_name)
                if TARGET_USE.search(value):
                    target_values.add(assigned_name)
            if command.name in {"unset", "read"}:
                modified.update(
                    word.text.lstrip("-")
                    for word in command.words[1:]
                    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", word.text.lstrip("-"))
                )
            if command.name in functions and function_available(command.name, command):
                nested = function_summary(command.name, seen | {name}, depth + 1)
                has_builder = has_builder or nested.direct_builder
                has_target = has_target or nested.direct_target
                invoked.update(nested.invoked)
                modified.update(nested.modified)
                builder_values.update(nested.builder_values)
                target_values.update(nested.target_values)
                refusal = refusal or nested.refusal
        result = FunctionSummary(
            has_builder,
            has_target,
            frozenset(invoked),
            frozenset(modified),
            frozenset(builder_values),
            frozenset(target_values),
            refusal,
        )
        summary_cache[name] = result
        return result

    def function_available(name: str, command: Command) -> bool:
        start = function_starts.get(name)
        return start is not None and (command.line, command.index) > start

    def execute(
        commands: list[Command],
        current_bindings: dict[str, bool | None],
        current_targets: dict[str, bool | None],
        active: tuple[str, ...] = (),
    ) -> None:
        nonlocal sequence, function_expansions, expansion_refused
        nonlocal processed_commands, processing_refused
        for command in commands:
            processed_commands += 1
            if processed_commands > processed_command_limit:
                if not processing_refused:
                    unsupported.append(
                        {
                            "line": command.line,
                            "reason": (
                                "processed command count exceeds "
                                f"{processed_command_limit}"
                            ),
                        }
                    )
                    processing_refused = True
                return
            if command.uncertain:
                sequence += 1
                uncertain_builder = builder_invocation(command, current_bindings)
                uncertain_use = has_target_use(command, current_targets)
                function_effect_reported = False
                assigned_builder, assigned_target = direct_assignment_states(command)
                if command.name in functions and function_available(command.name, command):
                    summary = function_summary(command.name)
                    affected_existing = {
                        variable
                        for variable in summary.modified
                        if current_bindings.get(variable, False) is not False
                        or current_targets.get(variable, False) is not False
                    }
                    relevant_invocation = any(
                        current_bindings.get(variable, False) is not False
                        or current_targets.get(variable, False) is not False
                        for variable in summary.invoked
                    )
                    relevant = bool(
                        summary.direct_builder
                        or summary.direct_target
                        or affected_existing
                        or relevant_invocation
                        or summary.builder_values
                        or summary.target_values
                        or summary.refusal
                    )
                    if relevant:
                        unsupported.append(
                            {
                                "line": command.line,
                                "reason": summary.refusal
                                or "relevant function effects occur in unproven control flow",
                            }
                        )
                        function_effect_reported = True
                    uncertain_builder = uncertain_builder or summary.direct_builder or any(
                        current_bindings.get(variable, False) is not False
                        for variable in summary.invoked
                    )
                    uncertain_use = uncertain_use or summary.direct_target or any(
                        current_targets.get(variable, False) is not False
                        for variable in summary.invoked
                    )
                    for variable in summary.modified:
                        if (
                            current_bindings.get(variable, False) is not False
                            or variable in summary.builder_values
                        ):
                            current_bindings[variable] = None
                        if (
                            current_targets.get(variable, False) is not False
                            or variable in summary.target_values
                        ):
                            current_targets[variable] = None
                if uncertain_builder and not builds and not function_effect_reported:
                    unsupported.append(
                        {
                            "line": command.line,
                            "reason": "builder invocation occurs in unproven control flow",
                        }
                    )
                if uncertain_use:
                    uses.append((sequence, command.line))
                    if not builds and not function_effect_reported:
                        unsupported.append(
                            {
                                "line": command.line,
                                "reason": "target use occurs in unproven control flow before a definite build",
                            }
                        )
                for assignment in command.assignments:
                    name, value = assignment_value(assignment)
                    possible_builder = value.endswith(BUILDER)
                    possible_target = bool(TARGET_USE.search(value))
                    prior_builder = current_bindings.get(name, False)
                    prior_target = current_targets.get(name, False)
                    current_bindings[name] = (
                        prior_builder if prior_builder == possible_builder else None
                    )
                    current_targets[name] = (
                        prior_target if prior_target == possible_target else None
                    )
                if command.name in {"local", "declare", "readonly", "export"}:
                    for word in command.words[1:]:
                        match = ASSIGNMENT.fullmatch(word.text)
                        if match:
                            name, value = match.group(1), match.group(2)
                            possible_builder = value.endswith(BUILDER)
                            possible_target = bool(TARGET_USE.search(value))
                            prior_builder = current_bindings.get(name, False)
                            prior_target = current_targets.get(name, False)
                            current_bindings[name] = (
                                prior_builder if prior_builder == possible_builder else None
                            )
                            current_targets[name] = (
                                prior_target if prior_target == possible_target else None
                            )
                if command.name in {"unset", "read"}:
                    for word in command.words[1:]:
                        name = word.text.lstrip("-")
                        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
                            continue
                        prior_builder = current_bindings.get(name, False)
                        prior_target = current_targets.get(name, False)
                        current_bindings[name] = False if prior_builder is False else None
                        current_targets[name] = False if prior_target is False else None
                continue
            sequence += 1
            if not command.words:
                for assignment in command.assignments:
                    name, value = assignment_value(assignment)
                    current_bindings[name] = value.endswith(BUILDER)
                    current_targets[name] = bool(TARGET_USE.search(value))
                continue

            if command.name in functions and function_available(command.name, command):
                if len(active) >= MAX_FUNCTION_DEPTH:
                    unsupported.append(
                        {
                            "line": command.line,
                            "reason": f"function call depth exceeds {MAX_FUNCTION_DEPTH} calls",
                        }
                    )
                elif command.name in active:
                    unsupported.append(
                        {"line": command.line, "reason": "recursive function flow is unsupported"}
                    )
                else:
                    function_expansions += 1
                    if function_expansions > function_expansion_limit:
                        if not expansion_refused:
                            unsupported.append(
                                {
                                    "line": command.line,
                                    "reason": f"function expansion exceeds {function_expansion_limit} calls",
                                }
                            )
                            expansion_refused = True
                    else:
                        execute(
                            functions[command.name],
                            current_bindings,
                            current_targets,
                            active + (command.name,),
                        )

            if builder_invocation(command, current_bindings):
                builds.append((sequence, command.line))
            builder_variable = variable_reference(command.words[0]) if command.words else None
            if builder_variable and current_bindings.get(builder_variable, False) is None:
                unsupported.append(
                    {
                        "line": command.line,
                        "reason": f"builder variable {builder_variable} has unproven control flow",
                    }
                )
            if has_target_use(command, current_targets):
                uses.append((sequence, command.line))
            elif referenced_target_state(command, current_targets) is None:
                uses.append((sequence, command.line))
                unsupported.append(
                    {
                        "line": command.line,
                        "reason": "target variable has unproven control flow",
                    }
                )

            if command.name in {"unset", "read"}:
                for word in command.words[1:]:
                    name = word.text.lstrip("-")
                    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
                        current_bindings[name] = False
                        current_targets[name] = False
            if command.name in {"local", "declare", "readonly", "export"}:
                for word in command.words[1:]:
                    match = ASSIGNMENT.fullmatch(word.text)
                    if match:
                        current_bindings[match.group(1)] = match.group(2).endswith(BUILDER)
                        current_targets[match.group(1)] = bool(TARGET_USE.search(match.group(2)))

    execute(top_level, bindings, target_bindings)
    first_use_position = min(uses, default=None)
    first_build = min(builds, default=None)
    ok = first_use_position is None or (first_build is not None and first_build[0] < first_use_position[0])
    return {
        "ok": ok,
        "uses": len(uses),
        "first_use": first_use_position[1] if first_use_position else None,
        "first_build": first_build[1] if first_build else None,
        "bindings": sum(1 for value in bindings.values() if value is True),
        "processed": processed_commands,
        "processed_limit": processed_command_limit,
        "unsupported": unsupported,
    }


def grep_options(words: list[str]) -> tuple[bool, bool]:
    quiet = False
    early = False
    i = 1
    while i < len(words):
        word = words[i]
        if word == "--":
            break
        if not word.startswith("-") or word == "-":
            break
        if word in {"--quiet", "--silent"}:
            quiet = True
        if word in {"--files-with-matches", "--files-without-match"} or word.startswith("--max-count="):
            early = True
        if word in {"-m", "--max-count"}:
            early = True
            i += 1
        if word in {"-e", "--regexp", "-f", "--file"}:
            i += 2
            continue
        if word.startswith("--regexp=") or word.startswith("--file="):
            i += 1
            continue
        if word.startswith("-") and not word.startswith("--"):
            letters = word[1:]
            offset = 0
            while offset < len(letters):
                option = letters[offset]
                if option == "q":
                    quiet = True
                elif option in {"l", "L"}:
                    early = True
                elif option == "m":
                    early = True
                    # The remainder, when present, is this option's argument.
                    break
                elif option in {"e", "f"}:
                    # The remainder, when present, is the pattern or file.
                    # Its letters are data, not more options.
                    if offset + 1 == len(letters):
                        i += 1
                    break
                offset += 1
        i += 1
    return quiet, early


def early_reader(command: Command) -> str | None:
    words = [word.text for word in command.words]
    if not words:
        return None
    name = Path(words[0]).name
    if name == "head":
        return "head"
    if name == "read" and len(words) == 1:
        return "bare read"
    if name == "grep":
        quiet, early = grep_options(words)
        if quiet:
            return "quiet grep"
        if early:
            return "early-closing grep"
    if name == "sed":
        scripts: list[str] = []
        index = 1
        while index < len(words):
            word = words[index]
            if word in {"-e", "--expression"}:
                if index + 1 < len(words):
                    scripts.append(words[index + 1])
                index += 2
                continue
            if word.startswith("-e") and len(word) > 2:
                scripts.append(word[2:])
                index += 1
                continue
            if word.startswith("--expression="):
                scripts.append(word.split("=", 1)[1])
                index += 1
                continue
            if word.startswith("-"):
                index += 1
                continue
            if not scripts:
                scripts.append(word)
            break
        if any(
            re.search(r"(?:^|[;{}\s])(?:[0-9$]+|/[^/]*/)?q(?:[;}]|$)", script)
            for script in scripts
        ):
            return "sed q"
    if name in {"awk", "gawk", "mawk", "nawk"}:
        program: str | None = None
        index = 1
        while index < len(words):
            word = words[index]
            if word == "--":
                program = words[index + 1] if index + 1 < len(words) else None
                break
            if word in {"-f", "--file"}:
                # A program read from another file is outside this shell scan.
                break
            if word in {"-F", "--field-separator", "-v", "--assign"}:
                index += 2
                continue
            if word.startswith("-"):
                index += 1
                continue
            program = word
            break
        if program:
            without_end = re.sub(r"END\s*\{[^{}]*\}", "", program)
            if re.search(r"(^|[;{}\s])exit(?:[;()\s]|$)", without_end):
                return "exit-on-match awk"
    return None


def pipeline_findings(analysis: Analysis) -> list[dict[str, object]]:
    findings: list[dict[str, object]] = []
    for pipeline in analysis.pipelines:
        if len(pipeline.commands) < 2:
            continue
        for stage, command in enumerate(pipeline.commands[1:], start=2):
            reader = early_reader(command)
            if reader:
                findings.append({"path": analysis.path, "line": command.line, "stage": stage, "reader": reader})
    return findings


def load_analyses(root: Path) -> list[Analysis]:
    analyses: list[Analysis] = []
    for relative in tracked_shell_files(root):
        analyses.append(analyze_file(root / relative, str(relative)))
    return analyses


def cmd_discover(args: argparse.Namespace) -> int:
    paths = tracked_shell_files(Path(args.root).resolve())
    if args.null:
        sys.stdout.buffer.write(b"".join(os.fsencode(str(path)) + b"\0" for path in paths))
    else:
        for path in paths:
            print(path)
    return 0


def cmd_negative(args: argparse.Namespace) -> int:
    root = Path(args.root).resolve()
    analyses = load_analyses(root)
    unsafe = [(analysis.path, token.line) for analysis in analyses for token in analysis.unsafe_negations]
    unsupported = [(analysis.path, line, why) for analysis in analyses for line, why in analysis.unsupported]
    for path, line, why in unsupported:
        print(f"{path}:{line}: unsupported shell syntax: {why}", file=sys.stderr)
    for path, line in unsafe:
        print(f"{path}:{line}: negated command has no explicit failure assertion", file=sys.stderr)
    print(json.dumps({"commands": sum(len(item.commands) for item in analyses), "files": len(analyses), "negations": sum(len(item.negations) for item in analyses), "unsafe": len(unsafe), "unsupported": len(unsupported)}, sort_keys=True))
    return 1 if unsafe or unsupported else 0


def cmd_builder(args: argparse.Namespace) -> int:
    root = Path(args.root).resolve()
    analyses = load_analyses(root)
    held = 0
    bad = 0
    unsupported_count = 0
    for analysis in analyses:
        if analysis.path in {"tests/built-executable-currency.sh", "tests/shell-scanner-coverage.sh"}:
            continue
        result = check_builder(analysis)
        for line, why in analysis.unsupported:
            unsupported_count += 1
            print(f"{analysis.path}:{line}: unsupported shell syntax: {why}", file=sys.stderr)
        for finding in result["unsupported"]:
            unsupported_count += 1
            print(
                f"{analysis.path}:{finding['line']}: unsupported builder flow: {finding['reason']}",
                file=sys.stderr,
            )
        if result["uses"]:
            held += 1
            if not result["ok"]:
                bad += 1
                print(f"{analysis.path}:{result['first_use']}: target/debug use precedes a recognized builder invocation", file=sys.stderr)
    print(json.dumps({"files": len(analyses), "held": held, "bad": bad, "unsupported": unsupported_count}, sort_keys=True))
    return 1 if bad or unsupported_count else 0


def cmd_pipelines(args: argparse.Namespace) -> int:
    root = Path(args.root).resolve()
    analyses = load_analyses(root)
    findings = [finding for analysis in analyses for finding in pipeline_findings(analysis)]
    unsupported = [(analysis.path, line, why) for analysis in analyses for line, why in analysis.unsupported]
    for path, line, why in unsupported:
        print(f"{path}:{line}: unsupported shell syntax: {why}", file=sys.stderr)
    for finding in findings:
        print(f"{finding['path']}:{finding['line']}: pipeline stage {finding['stage']} uses {finding['reader']}", file=sys.stderr)
    print(json.dumps({"files": len(analyses), "pipelines": sum(len(item.pipelines) for item in analyses), "findings": len(findings), "unsupported": len(unsupported)}, sort_keys=True))
    return 1 if findings or unsupported else 0


def cmd_file(args: argparse.Namespace) -> int:
    path = Path(args.path)
    analysis = analyze_file(path, args.display or str(path))
    payload: dict[str, object] = {
        "commands": [{"line": item.line, "name": item.name, "words": [word.text for word in item.words], "function": item.function} for item in analysis.commands],
        "negations": [item.line for item in analysis.negations],
        "unsafe_negations": [item.line for item in analysis.unsafe_negations],
        "pipelines": [[item.name for item in pipeline.commands] for pipeline in analysis.pipelines if len(pipeline.commands) > 1],
        "pipeline_findings": pipeline_findings(analysis),
        "builder": check_builder(analysis),
        "unsupported": analysis.unsupported,
    }
    print(json.dumps(payload, sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(required=True)
    discover = subparsers.add_parser("discover")
    discover.add_argument("root")
    discover.add_argument("--null", action="store_true")
    discover.set_defaults(function=cmd_discover)
    negative = subparsers.add_parser("negative")
    negative.add_argument("root")
    negative.set_defaults(function=cmd_negative)
    builder = subparsers.add_parser("builder")
    builder.add_argument("root")
    builder.set_defaults(function=cmd_builder)
    pipelines = subparsers.add_parser("pipelines")
    pipelines.add_argument("root")
    pipelines.set_defaults(function=cmd_pipelines)
    file_parser = subparsers.add_parser("file")
    file_parser.add_argument("path")
    file_parser.add_argument("--display")
    file_parser.set_defaults(function=cmd_file)
    args = parser.parse_args()
    try:
        return args.function(args)
    except ScanError as exc:
        print(f"shell scan: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
