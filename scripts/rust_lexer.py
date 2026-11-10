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
import sys
from bisect import bisect_left
from pathlib import Path

WORD = re.compile(r"\w+")
SPACE = re.compile(r"\s+")
STRING_STOP = re.compile(r'[\\"]')
QUOTE_STOP = re.compile(r"['\n]")
BLOCK_STOP = re.compile(r"/\*|\*/")
FENCE_OPEN = re.compile(r"^\s*(`{3,}|~{3,})(.*)$")
RAW_PREFIXES = ("r", "br", "cr")
BOM = "\ufeff"
UNPRINTABLE = re.compile(r"[^\x20-\x7e]")
NAMED_ESCAPES = {"\n": "\\n", "\r": "\\r", "\t": "\\t"}
OS_ERRORS = (
    (FileNotFoundError, "no such file"),
    (NotADirectoryError, "not a directory"),
    (PermissionError, "permission denied"),
)


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
        start = 1 if text.startswith(BOM) else 0
        self.text = " " * start + text[start:]
        self.newlines = [m.start() for m in re.finditer("\n", self.text)]
        self.tokens = tokenize(self.text, start)

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


def escape(text):
    return UNPRINTABLE.sub(escape_character, text)


def escape_character(match):
    char = match.group(0)
    if char in NAMED_ESCAPES:
        return NAMED_ESCAPES[char]
    code = ord(char)
    if code <= 0xFF:
        return f"\\x{code:02x}"
    if code <= 0xFFFF:
        return f"\\u{code:04x}"
    return f"\\U{code:08x}"


def display(path):
    return escape(path)


def emit(text, stream=None):
    print(escape(text), file=sys.stdout if stream is None else stream)


def os_error(error):
    if isinstance(error, IsADirectoryError) or (error.filename is not None and os.path.isdir(error.filename)):
        return "is a directory"
    for kind, text in OS_ERRORS:
        if isinstance(error, kind):
            return text
    return error.strerror or "operating system error"


def git(*args):
    try:
        result = subprocess.run(
            ["git", *args], capture_output=True, check=False, env={**os.environ, "LC_ALL": "C"}
        )
    except OSError as error:
        raise SetupError(f"cannot run git: {os_error(error)}") from None
    if result.returncode != 0:
        if b"not a git repository" in result.stderr:
            raise SetupError("not inside a git repository")
        raise SetupError(f"git {args[0]} exited with status {result.returncode}")
    return result.stdout.decode("utf-8", "surrogateescape")


def repository_files(*patterns):
    root = git("rev-parse", "--show-toplevel").rstrip("\n")
    os.chdir(root)
    output = git("ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", *patterns)
    names = sorted(n for n in output.split("\0") if n)
    return [n for n in names if Path(n).is_file()]


def tokenize(text, start=0):
    tokens = []
    i = shebang_end(text, start)
    while i < len(text):
        if text[i].isspace():
            i = SPACE.match(text, i).end()
            continue
        token = next_token(text, i)
        tokens.append(token)
        i = token.end
    return tokens


def shebang_end(text, start):
    if not text.startswith("#!", start) or text[start + 2:].split("\n", 1)[0].lstrip().startswith("["):
        return start
    end = text.find("\n", start)
    return len(text) if end < 0 else end


def next_token(text, i):
    if text.startswith("//", i):
        return line_comment(text, i)
    if text.startswith("/*", i):
        return block_comment(text, i)
    if text[i] == '"':
        return Token("literal", i, string_end(text, i))
    if text[i] == "'":
        return quote_token(text, i)
    match = WORD.match(text, i)
    if match is None:
        return Token("punct", i, i + 1)
    return word_token(text, i, match.end())


def unterminated(text, start, what):
    line = text.count("\n", 0, start) + 1
    return SourceError(f"unterminated {what} at line {line}")


def line_comment(text, i):
    end = text.find("\n", i)
    end = len(text) if end < 0 else end
    if end > i and text[end - 1] == "\r":
        end -= 1
    doc = None
    if text.startswith("///", i) and not text.startswith("////", i):
        doc = "outer"
    elif text.startswith("//!", i):
        doc = "inner"
    return Token("comment", i, end, "line", doc)


def block_comment(text, i):
    depth = 1
    j = i + 2
    while depth:
        match = BLOCK_STOP.search(text, j)
        if match is None:
            raise unterminated(text, i, "block comment")
        depth += 1 if match.group(0) == "/*" else -1
        j = match.end()
    doc = None
    if text.startswith("/**", i) and not text.startswith(("/***", "/**/"), i):
        doc = "outer"
    elif text.startswith("/*!", i):
        doc = "inner"
    return Token("comment", i, j, "block", doc)


def word_token(text, i, j):
    word = text[i:j]
    after = text[j:j + 1]
    if word in RAW_PREFIXES and after in ('"', "#"):
        return raw_token(text, i, j)
    if word in ("b", "c") and after == '"':
        return Token("literal", i, string_end(text, j))
    if word == "b" and after == "'":
        return Token("literal", i, char_end(text, j))
    return Token("word", i, j)


def raw_token(text, i, j):
    k = j
    while k < len(text) and text[k] == "#":
        k += 1
    if text.startswith('"', k):
        close = '"' + "#" * (k - j)
        end = text.find(close, k + 1)
        if end < 0:
            raise unterminated(text, i, "raw string literal")
        return Token("literal", i, end + len(close))
    if text[i:j] == "r" and k - j == 1:
        identifier = WORD.match(text, k)
        if identifier is not None:
            return Token("word", i, identifier.end())
    return Token("word", i, j)


def string_end(text, start):
    i = start + 1
    while True:
        match = STRING_STOP.search(text, i)
        if match is None:
            raise unterminated(text, start, "string literal")
        if match.group(0) == '"':
            return match.end()
        i = match.end() + 1


def quote_token(text, start):
    if text.startswith("\\", start + 1) or is_short_char(text, start):
        return Token("literal", start, char_end(text, start))
    label = WORD.match(text, start + 1)
    if label is None:
        raise unterminated(text, start, "character literal")
    return Token("lifetime", start, label.end())


def is_short_char(text, start):
    return text[start + 2:start + 3] == "'" and text[start + 1] != "\n"


def char_end(text, start):
    if text.startswith("\\", start + 1):
        first = start + 3
    elif is_short_char(text, start):
        return start + 3
    else:
        first = start + 1
    match = QUOTE_STOP.search(text, first)
    if match is None or match.group(0) != "'":
        raise unterminated(text, start, "character literal")
    return match.end()


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
    raise unterminated(source.text, tokens[index].start, "attribute")


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
