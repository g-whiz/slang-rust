"""Rust lexer shared by the checkers under scripts/.

Splits Rust source into comments, literals, words, lifetimes, and single
punctuation characters, which is enough to find every comment without
mistaking text inside a string, raw string, byte string, C string, or
character literal for one. It also groups line documentation comments
into the Markdown text that rustdoc reads, and finds fenced code blocks
in that text. Documentation written as a #[doc = ...] attribute or as a
block documentation comment is not read as Markdown.
"""

import os
import re
import subprocess
from bisect import bisect_left
from pathlib import Path

WORD = re.compile(r"\w+")
SPACE = re.compile(r"\s+")
STRING_STOP = re.compile(r'[\\"]')
BLOCK_STOP = re.compile(r"/\*|\*/")
FENCE_OPEN = re.compile(r"^\s*(`{3,}|~{3,})(.*)$")
RAW_PREFIXES = ("r", "br", "cr")
BOM = "\ufeff"


class SourceError(Exception):
    pass


class SetupError(Exception):
    pass


class Token:
    __slots__ = ("kind", "start", "end", "style", "doc")

    def __init__(self, kind, start, end, style=None, doc=None):
        self.kind = kind
        self.start = start
        self.end = end
        self.style = style
        self.doc = doc


class Source:
    def __init__(self, text):
        if text.startswith(BOM):
            text = " " + text[1:]
        self.text = text
        self.newlines = [m.start() for m in re.finditer("\n", text)]
        self.tokens = tokenize(text)

    def line(self, offset):
        return bisect_left(self.newlines, offset) + 1


def read_source(path):
    data = Path(path).read_bytes()
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as error:
        line = data.count(b"\n", 0, error.start) + 1
        raise SourceError(
            f"invalid UTF-8 byte 0x{data[error.start]:02X} at line {line}"
        ) from None


def display(path):
    return path.replace("\n", "\\n").replace("\r", "\\r")


def git(*args):
    try:
        result = subprocess.run(["git", *args], capture_output=True, check=False)
    except OSError as error:
        raise SetupError(f"cannot run git: {error.strerror}") from None
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", "replace").strip().splitlines()
        raise SetupError(f"git {args[0]} failed: {message[0] if message else 'no output'}")
    return result.stdout.decode("utf-8", "surrogateescape")


def repository_files(*patterns):
    root = git("rev-parse", "--show-toplevel").rstrip("\n")
    os.chdir(root)
    output = git("ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", *patterns)
    names = sorted(n for n in output.split("\0") if n)
    return [n for n in names if Path(n).is_file()]


def tokenize(text):
    tokens = []
    n = len(text)
    i = 0
    if text.startswith("#!") and not text[2:].split("\n", 1)[0].lstrip().startswith("["):
        end = text.find("\n")
        i = n if end < 0 else end
    while i < n:
        c = text[i]
        if c.isspace():
            i = SPACE.match(text, i).end()
            continue
        if c == "/" and text.startswith("//", i):
            end = text.find("\n", i)
            end = n if end < 0 else end
            stop = end - 1 if end > i and text[end - 1] == "\r" else end
            body = text[i:stop]
            doc = None
            if body.startswith("///") and not body.startswith("////"):
                doc = "outer"
            elif body.startswith("//!"):
                doc = "inner"
            tokens.append(Token("comment", i, stop, "line", doc))
            i = end
            continue
        if c == "/" and text.startswith("/*", i):
            depth = 1
            j = i + 2
            while depth:
                match = BLOCK_STOP.search(text, j)
                if match is None:
                    j = n
                    break
                depth += 1 if match.group(0) == "/*" else -1
                j = match.end()
            doc = None
            if text.startswith("/**", i) and not text.startswith(("/***", "/**/"), i):
                doc = "outer"
            elif text.startswith("/*!", i):
                doc = "inner"
            tokens.append(Token("comment", i, j, "block", doc))
            i = j
            continue
        if c == '"':
            end = scan_string(text, i)
            tokens.append(Token("literal", i, end))
            i = end
            continue
        if c == "'":
            kind, end = scan_quote(text, i, False)
            tokens.append(Token(kind, i, end))
            i = end
            continue
        match = WORD.match(text, i)
        if match is None:
            tokens.append(Token("punct", i, i + 1))
            i += 1
            continue
        word = match.group(0)
        j = match.end()
        if word in RAW_PREFIXES and j < n and text[j] in '"#':
            k = j
            while k < n and text[k] == "#":
                k += 1
            if k < n and text[k] == '"':
                close = '"' + "#" * (k - j)
                end = text.find(close, k + 1)
                end = n if end < 0 else end + len(close)
                tokens.append(Token("literal", i, end))
                i = end
                continue
            if word == "r" and k - j == 1:
                raw = WORD.match(text, k)
                if raw is not None:
                    tokens.append(Token("word", i, raw.end()))
                    i = raw.end()
                    continue
        elif word in ("b", "c") and j < n and text[j] == '"':
            end = scan_string(text, j)
            tokens.append(Token("literal", i, end))
            i = end
            continue
        elif word == "b" and j < n and text[j] == "'":
            _, end = scan_quote(text, j, True)
            tokens.append(Token("literal", i, end))
            i = end
            continue
        tokens.append(Token("word", i, j))
        i = j
    return tokens


def scan_string(text, start):
    i = start + 1
    while True:
        match = STRING_STOP.search(text, i)
        if match is None:
            return len(text)
        if match.group(0) == "\\":
            i = match.end() + 1
            continue
        return match.end()


def scan_quote(text, start, force_char):
    n = len(text)
    if start + 1 < n and text[start + 1] == "\\":
        j = start + 3
        while j < n and text[j] not in "'\n":
            j += 1
        return "literal", min(j + 1, n)
    if start + 2 < n and text[start + 2] == "'" and text[start + 1] != "\n":
        return "literal", start + 3
    if force_char:
        j = start + 1
        while j < n and text[j] not in "'\n":
            j += 1
        return "literal", min(j + 1, n)
    match = WORD.match(text, start + 1)
    if match is None:
        return "punct", start + 1
    return "lifetime", match.end()


def is_punct(source, token, char):
    return token.kind == "punct" and source.text[token.start] == char


def skip_attribute(source, index, inner):
    tokens = source.tokens
    k = index + 1
    if inner:
        if k >= len(tokens) or not is_punct(source, tokens[k], "!"):
            return None
        k += 1
    if k >= len(tokens) or not is_punct(source, tokens[k], "["):
        return None
    depth = 0
    while k < len(tokens):
        if is_punct(source, tokens[k], "["):
            depth += 1
        elif is_punct(source, tokens[k], "]"):
            depth -= 1
            if depth == 0:
                return k + 1
        k += 1
    return k


def doc_groups(source, doc, crate_level=False):
    groups = []
    current = None
    tokens = source.tokens
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if token.kind == "comment":
            if token.style == "line" and token.doc == doc:
                if current is None:
                    current = []
                    groups.append(current)
                body = source.text[token.start + 3:token.end]
                current.append((source.line(token.start), body))
            index += 1
            continue
        if is_punct(source, token, "#"):
            after = skip_attribute(source, index, doc == "inner")
            if after is not None:
                index = after
                continue
        if crate_level:
            break
        current = None
        index += 1
    return [unindent(group) for group in groups]


def crate_docs(source):
    groups = doc_groups(source, "inner", crate_level=True)
    return groups[0] if groups else []


def unindent(group):
    indents = [len(text) - len(text.lstrip(" \t")) for _, text in group if text.strip()]
    margin = min(indents, default=0)
    return [(number, text[margin:] if text.strip() else "") for number, text in group]


def markdown_lines(lines):
    fence = None
    for number, line in lines:
        if fence is not None:
            stripped = line.strip()
            char, length = fence
            if len(stripped) >= length and stripped == char * len(stripped):
                fence = None
                yield number, line, "close", None
            else:
                yield number, line, "code", None
            continue
        match = FENCE_OPEN.match(line)
        if match and not (match.group(1)[0] == "`" and "`" in match.group(2)):
            fence = (match.group(1)[0], len(match.group(1)))
            yield number, line, "open", match.group(2)
            continue
        yield number, line, "text", None
