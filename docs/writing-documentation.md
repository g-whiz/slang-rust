# Writing Documentation

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-12 |
| Applies to | Every Markdown document and all API documentation in this repository |

## Abstract

This document defines how documentation in this repository is written. It covers the requirements language, the structure of documents, the rules for prose, examples, diagrams, and formal notation, the properties that make a document dependable for a coding agent, the rules for API documentation written with rustdoc, the conventions for spelling and punctuation, and the maintenance of documentation as the code changes. The primary readers of Markdown documents are coding agents. The primary readers of API documentation are Rust developers who use the crates. The rules exist so that every reader reaches the same understanding of a document, and each rule says whether a tool or a reviewer checks it.

## 1. Introduction

This repository contains Rust crates for using the Slang shader language. Its Markdown documentation is human readable, but its primary audience is coding agents. An agent reads a document to plan a change, then implements the change from what the document says. The secondary audience is the humans who review those changes. The API documentation has a different primary audience, which Section 7.1 defines. Section 2.2 defines the terms that every document uses.

A human who finds a gap or an ambiguity in a document usually asks someone. An agent usually resolves it by guessing, and the guess becomes code. Documents must therefore be unambiguous, complete, and consistent in their terminology. The goal that the Guide for Internet Standards Writers sets for standards applies to every document here: "It is important that multiple readers and implementers of a standard have the same understanding of a document" [RFC2360].

The conventions here adapt to Markdown the documents that govern how Requests for Comments (RFCs) are written: [RFC2119], [RFC8174], [RFC7322], and [RFC2360]. They add practices from RFCs that are widely regarded as readable. Section 7 adds the conventions of the Rust ecosystem for API documentation [RUSTDOC] [RUST-API].

This guide governs the content of API documentation. It does not cover code style, commit messages, or the content of any particular design. The instruction file `AGENTS.md` and the standards document `docs/rust-coding-standards.md` govern code style.

## 2. Requirements Language

### 2.1. Key Words

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.2. Terminology

Each term in this section keeps exactly this meaning in every document in the repository.

- agent: A coding agent that reads documents in this repository to plan and implement changes.
- author: Any human or agent that writes or edits a document or API documentation.
- reviewer: Any human or agent that checks a change, including its documentation, before the change is merged.
- user: A developer who depends on a crate from this repository, chooses its Cargo features, and calls its API.
- role: The party that a requirement binds, such as a crate, its build script, the user, or an agent editing the repository.
- change: One logical change to the repository, such as a commit or a single task, together with its documentation.
- design document: A document that specifies the behaviour, interfaces, and operation of a component. This repository uses the term for every specification.
- guide: A document that sets conventions for the repository. This document is a guide.
- standards document: A document that sets the rules that code in the repository follows, such as `docs/rust-coding-standards.md`.
- reference list: A document that lists external sources for later consultation, with a topic index, such as `docs/rust-references.md`.
- README: The file `README.md` at the root of the repository, which is the landing page of the project on GitHub.
- instruction file: The file `AGENTS.md`, which every agent reads before it works in the repository. The file `CLAUDE.md` contains only the line `@AGENTS.md`, which imports the instruction file.
- API documentation: The documentation comments, written with `///` for an item and `//!` for a crate or module, that rustdoc renders into the reference documentation of a crate.
- public item: An item that code outside its crate can name. The rustc lint `missing_docs` checks these items.
- doctest: A code example in API documentation that `cargo test --doc` compiles and, unless the example is marked `no_run`, runs.
- safety comment: A line comment that starts with `// SAFETY:` and sits directly above an `unsafe` block or an `unsafe impl`.
- enforced rule: A rule that a named tool or lint checks on every change.
- judgement rule: A rule that a reviewer checks, because no deterministic tool checks it.
- normative text: Text that states a requirement.
- explanatory text: Text that gives background, rationale, or tutorial material and states no requirement.

### 2.3. Using the Key Words

An author uses key words only where a requirement matters for correctness, for interoperability between components, or to limit harmful behaviour, as Section 6 of [RFC2119] advises. An author MUST NOT use a key word to impose a method where the method does not matter, because agents treat every key word as binding, and a needless requirement constrains every later change. Lowercase "must" and "should" are ordinary English and carry no requirement level [RFC8174]. Normative text can also be written without key words.

An author MUST follow every MUST, MUST NOT, SHOULD, and SHOULD NOT in the same paragraph with the consequence of violating it, because without the consequence an agent has no basis for judging an edge case or for deciding whether an exception to a SHOULD is justified. The TCP specification shows the form: "The window size MUST be treated as an unsigned number, or else large window sizes will appear like negative windows and TCP will not work (MUST-1)" [RFC9293]. The JavaScript Object Notation (JSON) specification follows the same practice for SHOULD [RFC8259].

An author MUST write the role that a requirement binds as the subject of the sentence, following Section 2.2 of [RFC9110], because a requirement in the passive voice can be read as binding any component, and two agents then implement it in two places or in none. This example illustrates a requirement with its role and its consequence. It shows the form only and describes no existing behaviour:

```markdown
The safe crate MUST return an error, and MUST NOT panic, when Slang reports that a shader fails to compile, or else one invalid shader stops the user's whole application.
```

## 3. Document Structure

### 3.1. Design Document Skeleton

Design documents live in the directory `docs/design/`. A design document contains these sections in this order, adapted from Section 4 of [RFC7322].

1. Title (REQUIRED). A level-one heading in title case.
2. Metadata (REQUIRED). A small table directly under the title with the fields Status, Date, and Applies to.
3. Abstract (REQUIRED). One self-contained paragraph with no citations.
4. Introduction (REQUIRED). The problem, the purpose of the component, the audience, the scope, and the non-goals. It is self-contained and separable from the abstract.
5. Requirements Language (REQUIRED when the document uses key words). The paragraph from Section 2.1, verbatim.
6. Terminology (REQUIRED). Entries in the form `term: definition`. It MAY be a subsection of Requirements Language.
7. Body (REQUIRED). One or more numbered sections, each named for its topic.
8. Security Considerations (REQUIRED). The content that Section 3.2 defines.
9. Operational Considerations (REQUIRED). The content that Section 3.3 defines.
10. References (REQUIRED when the document cites anything). Normative and Informative subsections that contain reference entries only.
11. Appendices (OPTIONAL). Long examples, implementation pitfalls, and requirement summary tables.
12. Change Log (REQUIRED). The record that Section 9 defines.

An author MUST keep this order, because agents and reviewers find content by position and heading, and they miss content in an unexpected place. Appendix A gives a skeleton that an author can copy.

### 3.2. Security Considerations

A security considerations section never says "none" and never says only that security was not considered [RFC2360]. Following Section 5 of [RFC3552], it states the threat model, which is the set of capabilities an attacker is assumed to have. It then states which attacks are out of scope and why, and the residual risk to the user after the mitigations. A reviewer MUST reject a security considerations section that omits any of these three parts, because an agent that reads an empty or partial section concludes that there is no security property to preserve.

For a crate in this repository, the section covers each of these topics, or states why a topic does not apply:

- Unsafe code and foreign function interface (FFI) invariants: the conditions the crate relies on from the native Slang library, and the conditions it guarantees to its callers.
- Untrusted shader source and file paths: what happens when the user compiles shader source from an untrusted party, and which files the compiler can read through include paths and module search paths.
- Build scripts that run native builds: what a build script runs on the user's machine, such as a native build of Slang from source, and what it reads from the environment.
- Dependencies: the crates and native libraries the crate depends on, and how their versions are fixed.

### 3.3. Operational Considerations

This section takes the place of the Internet Assigned Numbers Authority (IANA) considerations in an RFC. For a crate, it covers these items:

- Every Cargo feature, with whether it is on by default and what it enables.
- Every environment variable that a build script reads, with the behaviour when it is unset.
- The supported platforms and graphics backends.
- The minimum supported Rust version (MSRV).
- The failure modes, with the error type or variant that reports each one and the recovery for each.
- The logs the crate emits, if any.

An author MUST state the default of every option, including every Cargo feature and every environment variable, because an agent that finds no default chooses one, and different agents choose differently. An author MUST make every option that enables a security check default to on, as Section 2.10 of [RFC2360] requires, so that a user who omits a setting gets the secure behaviour.

### 3.4. Shorter Documents

Other document types use a subset of the skeleton in Section 3.1, in the same order. This table shows which sections each type requires.

| Section | Design document | Guide | Standards document | Reference list | README | Instruction file |
| --- | --- | --- | --- | --- | --- | --- |
| Title | Required | Required | Required | Required | Required | Required |
| Metadata | Required | Required | Required | Required | Omitted | Omitted |
| Abstract | Required | Required | Required | Optional | Omitted | Omitted |
| Introduction | Required | Required | Required | Required | Optional | Optional |
| Requirements Language | If key words | If key words | If key words | If key words | If key words | If key words |
| Terminology | Required | Required | Required | Optional | Optional | Optional |
| Body | Required | Required | Required | Required | Required | Required |
| Security Considerations | Required | Required | Required | Optional | Optional | Optional |
| Operational Considerations | Required | Optional | Optional | Optional | Optional | Optional |
| References | If citing | If citing | If citing | If citing | If citing | If citing |
| Appendices | Optional | Optional | Optional | Optional | Optional | Optional |
| Change Log | Required | Required | Required | Required | Omitted | Omitted |

An author omits an optional section that would be empty, because a section that says only "not applicable" adds length and no information. The README and the instruction file have no change log because their history is the history of the repository. The README has no metadata table because it is the first page a visitor sees on GitHub, and it opens with what the project is.

### 3.5. Enforced and Judgement Rules

A guide or a standards document states, for each rule, the tool or lint that enforces it, or that the rule is a judgement rule. An author MUST assign every rule that a deterministic tool can check to that tool and MUST NOT leave it to a reviewer, because a reviewer misses findings that a tool reports every time, and a reviewer's effort is better spent on what only a reviewer can judge. When a tool could check a rule but no tool in the repository does yet, the document calls the rule a judgement rule until a tool covers it. In this guide, a rule that names no tool is a judgement rule.

## 4. Prose

### 4.1. Narrative and Lists

Lists are for enumeration only. A list is permitted for the steps of a procedure, for term and definition entries, for field or parameter definitions, and for enumerations of concrete items such as files, options, or states. Everything else is prose, in paragraphs of a few sentences that each carry one idea. An author MUST NOT use a list for rationale, design discussion, consequences, or any other connected argument, because a list drops the words that connect ideas, such as "because", "unless", and "therefore", and the agent then has to guess how the items relate. Tables are permitted for comparisons across options and for summaries, as Section 3.4 shows.

### 4.2. Sentences

Sentences are short, in the active voice, and in the present tense.

### 4.3. Order of Ideas

A document states the problem before the design, as Section 2.1 of [RFC1034] describes the limits of the host table before it introduces the Domain Name System. A section states the rule before the rationale.

Normative text is concise, and an author keeps it apart from explanatory text, as Section 2.4 of [RFC2360] recommends. Rationale goes in its own paragraph or subsection. A subsection that holds only explanatory text says so in its first sentence, as each subsection of Section 1 of [RFC6455] does with the sentence "This section is non-normative". Concise normative text leaves fewer places where two statements can conflict.

### 4.4. Terms and Abbreviations

A document defines every term in its Terminology section before it uses the term, in the form `term: definition` that Section 1.1 of [RFC8446] uses. An author expands an abbreviation on first use, except for universally known ones such as HTTP, TCP, and URL, following Section 3.6 of [RFC7322].

Each concept has one name across the whole repository, and that name is the industry-standard term where one exists. An author MUST NOT introduce a synonym for a concept that already has a name, because an agent reads two names as two concepts and builds two things.

### 4.5. Say It Once

The HTTP/1.1 specification [RFC2616] is the standard example of repetition. Its editors later called it unreadable, and the rewrite of its caching text removed about ten pages that said the same things in slightly different ways. An author MUST state each requirement in exactly one place and cite that place by section number elsewhere, because two statements of one requirement diverge, and an agent that finds both cannot tell which one is current.

## 5. Examples, Diagrams, and Formal Notation

### 5.1. Examples

Every behaviour that a document describes in abstract terms has a complete example that a reader can run or verify. The examples include abnormal cases, as Section 5.4 of [RFC3986] does with its separate normal and abnormal examples. Each example has a one-line lead-in that says what it shows. Code and commands go in fenced code blocks with a language tag. An example that shows a form, such as an API that does not exist yet, says in its lead-in that it is an illustration, because an agent otherwise takes the names in it as real.

This example shows a lead-in followed by a command in a fenced code block, as it appears in the Markdown source:

````markdown
This command lists the recipes in the current project:

```sh
just --list
```
````

### 5.2. Diagrams

Diagrams, in American Standard Code for Information Interchange (ASCII) text or Mermaid, and state tables supplement the prose and never replace it. When a diagram and the prose disagree, the prose is authoritative, as Section 3.3 of [RFC2360] states, and the reviewer treats the disagreement as a defect in the diagram.

### 5.3. Formal Notation

A format that software parses, such as a configuration file, a file that a build script reads, or the input of a code generator, has its authoritative definition in a schema or a formal grammar, such as JSON Schema or Augmented Backus-Naur Form (ABNF) [RFC5234]. The schema lives in the repository, and the prose cites it by path. The prose explains purpose and meaning that the schema cannot express. An author MUST NOT restate a schema field by field in prose, because the restatement diverges from the schema, and an agent may then implement the prose instead of the schema.

## 6. Writing for Agents

### 6.1. Citable Structure

Sections are numbered so that an agent can cite them, as in "Section 4.5". Each section covers one concept. A document refers to other content by section number, never by phrases such as "see above" or "as mentioned". The instruction file and the README are the exceptions, because their headings are unnumbered and agents cite them by heading.

An author SHOULD NOT renumber or rename an existing section, because other documents cite it, and each citation then points to the wrong place. A change that renumbers a section updates every citation to it.

### 6.2. Explicit Behaviour

A document states how each role handles input outside what the document defines, as Section 2.8 of [RFC2360] recommends. An author MUST state that handling, because an agent left to choose tends to accept the input silently, and tolerated errors become behaviour that other components depend on [RFC9413].

### 6.3. Exact Identifiers

File paths, commands, identifiers, Cargo feature names, and environment variable names are written exactly, in code spans, such as `docs/writing-documentation.md`. An agent copies them as written, so a paraphrase or an altered form becomes a defect.

### 6.4. Plain Language

A document avoids humour, idiom, and metaphor, because an agent can read figurative language literally, and a human whose first language is not English can misread it.

### 6.5. Labelled Requirements

The TCP specification labels each requirement with an identifier, such as MUST-1 or SHLD-2, and summarizes all of them in a table in an appendix [RFC9293]. An author SHOULD label every requirement in a design document longer than about 3,000 words and summarize the labels in a table in an appendix, because without labels a reviewer cannot check an implementation requirement by requirement, and an agent cannot cite the requirement that its change satisfies. A label is never reused for a different requirement.

## 7. API Documentation

This section governs the API documentation of every crate in the repository. Each of Sections 7.1 to 7.11 ends with a "Checked by" statement that names the tool or lint that enforces the rule, or says that the rule is a judgement rule, as Section 3.5 requires. Section 7.12 summarizes these checks.

### 7.1. Audience and Plain Language

The reader of API documentation is a user who knows Rust but may know nothing about Slang, shaders, or graphics processing unit (GPU) programming. An author writes API documentation in plain English for that reader, in short sentences.

An author MUST define in a short clause, or link to a definition of, every term from graphics or compilers on its first use in the documentation of each item. Examples are shader, entry point, SPIR-V, and uniform buffer. A reader often reaches one item from a search and reads nothing else, and a reader who does not know the term cannot use the item.

Checked by: judgement.

### 7.2. Comments in Code

Rust source files in this repository contain only these comments:

- API documentation on every public item, except generated raw bindings, which Section 7.9 exempts.
- A `# Safety` section on every unsafe function and every unsafe trait, whatever its visibility, as Section 7.6 defines, except in generated raw bindings under the conditions of Section 7.9.
- A safety comment on every `unsafe` block and every `unsafe impl`, as Section 7.6 defines, except in generated raw bindings under the conditions of Section 7.9.

In other files, such as the `justfile`, scripts, and configuration files, the only comments are text addressed to a human, such as the help comment of a recipe, the usage text of a script, or the description of a setting.

An author MUST NOT write any other comment in Rust source, including a line comment that explains code, a `TODO`, or a `FIXME`, because the compiler does not check a comment, so a stale comment contradicts the code, and an agent that reads both tends to trust the comment.

Checked by: rustc `missing_docs` for API documentation on public items. `clippy::undocumented_unsafe_blocks` for safety comments. `check-rust-comments` for the absence of other comments in Rust source: it reports every line comment other than a `///` comment, a `//!` comment, or a safety comment, every block comment, including `/** */` and `/*! */`, and every comment that contains `TODO` or `FIXME`. It checks generated files like any other file. The comments in other files are a judgement rule.

### 7.3. Shape of an Item's Documentation

The API documentation of an item starts with a one-line summary sentence, then a blank line, then the body. The summary sentence says what the item does. The first paragraph of the body says when to use the item. An author MUST keep the summary to one sentence in its own paragraph, because rustdoc shows the first paragraph in module listings and search results, and a long or vague first paragraph hides what the item is for.

The body uses these standard sections, in this order, and omits a section that has nothing to say:

1. `# Examples`: the examples that Section 7.5 defines.
2. `# Errors`: the conditions that Section 7.6 defines.
3. `# Panics`: the conditions that Section 7.6 defines.
4. `# Safety`: the contract that Section 7.6 defines.
5. `# Platform support`: the platforms or backends on which the item is unavailable or behaves differently, as Section 7.8 defines.

An author MUST use these headings exactly, as level-one headings, each at most once and in this order, and MUST NOT give one of these topics another heading, such as `# Failures`, because readers look for the standard names, and the Clippy lints in Section 7.6 find the `# Errors`, `# Panics`, and `# Safety` sections by their headings. An author MUST NOT add any other heading, of any level, to the documentation of an item, because a reader who knows the standard sections then has to read every heading to find the contract. These heading rules do not apply to crate-level and module-level documentation, which is written with `//!`, and Section 7.4 defines the headings of crate-level documentation.

This illustration shows the shape. The names in it are examples of the form and do not describe an existing API:

````rust
impl Session {
    /// Compiles one Slang module from source text.
    ///
    /// Use this when the shader source is a string in memory, such as source
    /// that your program generates. A module is one file of Slang source, like
    /// a Rust module. To load a module from a file, use [`Session::load_module`].
    ///
    /// # Examples
    ///
    /// This example needs the Slang shared library at run time, so it is compiled but not run.
    ///
    /// ```no_run
    /// # use slang_compiler::Session;
    /// let session = Session::new()?;
    /// let module = session.compile_module("shapes", "float area(float r) { return 3.14 * r * r; }")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`CompileError`] when the source has a syntax error or a type
    /// error. The error holds the messages that Slang reported.
    pub fn compile_module(&self, name: &str, source: &str) -> Result<Module, CompileError> {
        self.compile_source(name, source)
    }
}
````

Checked by: the summary sentence and the first paragraph are judgement rules. `clippy::too_long_first_doc_paragraph` is in the Clippy `nursery` group, which this repository does not enable. `clippy::unnecessary_safety_doc` rejects a `# Safety` section on a safe item. `check-doc-headings` checks the headings in the `///` documentation of every item: it reports a heading that is not a standard heading, including a standard heading with the wrong case or level, a standard heading out of order, and a standard heading that appears twice. It ignores lines inside fenced code blocks, so a hidden doctest line that starts with `#` is not a heading.

### 7.4. Crate-Level and Module-Level Documentation

Every crate has crate-level documentation, written with `//!` at the top of `src/lib.rs`. It contains, in this order:

1. A summary sentence that says what the crate does.
2. A paragraph that says where the crate fits among the crates of this repository and when to use it instead of a sibling crate.
3. A complete example of the most common task, written as Section 7.5 defines.
4. A `# Features` section, as Section 7.8 defines.
5. A `# Platform support` section that lists the supported platforms and backends.
6. When the crate links to the native Slang library, a section that says how the crate finds or builds that library, or a link to the crate that does.

Every public module has module-level documentation, written with `//!` at the top of the module file. It says what the module groups together and when a user goes to it, and it links to the module's main items.

An author MUST write crate-level documentation with this content, because it is the first page a user sees on docs.rs, and a user who cannot tell from it which crate to use picks one by trial.

Checked by: `rustdoc::missing_crate_level_docs` for the presence of crate-level documentation. Rustc `missing_docs` for the presence of documentation on public modules. `check-feature-docs` for the presence of the `# Features` section in the crate-level documentation of every library, as Section 7.8 describes. The rest of the content is a judgement rule.

### 7.5. Examples and Doctests

An author SHOULD give every public item an example that shows why a user calls it, not only how, because without an example a user has to infer the intended use from the signature alone. An author MAY link to the example of a related item instead of repeating it, because repeated examples diverge (Section 4.5).

Every example is a doctest that `cargo test --doc` compiles and, unless the example is marked `no_run`, runs. An author MUST write examples with the `?` operator and MUST NOT use `unwrap` or `expect` in them, because users copy examples into their programs, and a copied `unwrap` turns a recoverable error into a panic. The example ends with a hidden line that returns `Ok`, such as `# Ok::<(), Box<dyn std::error::Error>>(())`, and has no visible `fn main`. Hidden lines, which start with `# `, hold only setup that does not help the reader, such as `use` declarations.

When an example needs something that exists only when the example runs, such as a GPU, a display, a file, or a shared library that the dynamic loader must find, an author MUST mark the example `no_run` and MUST NOT mark it `ignore`, because rustdoc still compiles a `no_run` example, so a renamed or removed item breaks the build, while an `ignore` example is never compiled and becomes wrong without any warning. The lead-in sentence of the example states the reason, as the illustration in Section 7.3 does. A `no_run` example cannot avoid a failed build of the crate itself, such as when the native Slang library is missing at build time, because rustdoc still compiles and links the example against the crate.

The one exception is an example that cannot compile as a doctest, such as a call of a procedural macro that resolves a path against the directory of the Rust file that calls it, because in a doctest that file is the source file that holds the documentation, where the path of the example does not resolve. An author MAY write such an example as a `text` block and MUST state the reason in its lead-in sentence, because a doctest of it would fail on every build. A compiled example of the same use, such as an example program, SHOULD exist, because nothing checks the `text` block.

A code block that is not Rust names its language, such as `toml`, `slang`, or `text`, because rustdoc compiles a code block with no language tag as Rust.

Checked by: `cargo test --doc`, which compiles every doctest and runs every doctest not marked `no_run`. `cargo nextest run` does not run doctests, so `cargo test --doc` runs separately. `rustdoc::invalid_rust_codeblocks` and `rustdoc::invalid_codeblock_attributes` report malformed blocks and misspelled attributes. `clippy::needless_doctest_main` rejects a visible `fn main`. Whether each public item has an example is a judgement rule, because the lint `rustdoc::missing_doc_code_examples` is available only on the nightly toolchain. No tool lints the code inside a doctest: `cargo test --doc` does not report warnings in doctest bodies, and Clippy does not lint doctests. The use of `?` instead of `unwrap`, and the style of example code in general, are therefore judgement rules. `check-doctest-attributes` reports a code block whose attributes include `ignore` or an attribute that starts with `ignore-`, such as `ignore-windows`, in the `///` and `//!` comments of every Rust file and in every `README.md` file. It splits the info string of a code block into attributes the way rustdoc does, at commas, spaces, and tabs, so a variant such as `rust,ignore` is found. Whether the lead-in of a `no_run` example states the reason, and whether that reason is correct, are judgement rules.

### 7.6. Errors, Panics, and Safety

These sections state the contract of an item:

- `# Errors`: Every public function that returns `Result` lists each condition that produces an error and the error type or variant that reports it, as an intra-doc link.
- `# Panics`: Every public function that can panic lists each condition that causes the panic.
- `# Safety`: Every unsafe function and every unsafe trait, whatever its visibility, lists each condition that the caller, or the implementer of the trait, must uphold. The section says that breaking any of the conditions is undefined behaviour.

A function that can stop the process on purpose, such as a function that aborts when a foreign exception reaches it, states this in a paragraph of the body of its documentation. The paragraph begins with the word "Aborts". It states each condition under which the process aborts, that the process aborts instead of returning an error, and what it writes before it aborts. Section 7.3 has no standard heading for an abort, and `check-doc-headings` rejects every other heading, so an author MUST NOT give the abort a heading of its own, or the check fails.

When every call of a crate into a foreign library can abort for the same reason, the crate-level documentation states that general case once, in a paragraph that begins with "Aborts". A function then carries its own paragraph only when the abort has been observed for its call, because a reader who reaches that function from a search does not read the crate page and needs to know that this call aborts.

A safety comment sits directly above every `unsafe` block and every `unsafe impl`. It names each condition that the unsafe operation requires, citing the `# Safety` section of the function it calls or the documented contract of the Slang API, and states why each condition holds at that point in the code. It does not restate what the code does.

An author MUST write these sections and comments with this content, because a caller who does not know an error or panic condition cannot handle it, and a caller or maintainer who does not know a safety condition can cause undefined behaviour that no test detects. Generated raw bindings are the one exception to the `# Safety` sections and safety comments, under the conditions of Section 7.9.

This illustration shows a safety comment. The names in it are examples of the form:

```rust
// SAFETY: `self.raw` is non-null and points to a live Slang session, because
// `Session::new` checked it for null and `self` holds one reference to it.
// `release` requires exactly that, and `Drop` runs once, so the reference
// is released once.
unsafe { release(self.raw) }
```

Checked by: `clippy::missing_errors_doc` and `clippy::missing_panics_doc` for the presence of `# Errors` and `# Panics`. `clippy::missing_safety_doc` for the presence of `# Safety` on public unsafe functions. `clippy::undocumented_unsafe_blocks` for the presence of safety comments. `check-doc-headings` for the absence of a heading for an abort. The sentence that states an abort, the `# Safety` section on a private unsafe function and on an unsafe trait, and the content of every section and safety comment, are judgement rules, because the repository leaves the Clippy option `check-private-items` off (Section 4.3 of the standards document).

### 7.7. Intra-Doc Links

Every mention of an item, whether from the same crate, the standard library, or a dependency, is an intra-doc link, such as ``[`Session`]``. An author MUST link every mentioned item, because the reader of one item's documentation needs a single click to reach every type it names. A web address is written in angle brackets, such as `<https://shader-slang.org>`, so that rustdoc renders it as a link.

Checked by: `rustdoc::broken_intra_doc_links`, `rustdoc::private_intra_doc_links`, and `rustdoc::bare_urls`, which fail the documentation build when it runs with `CARGO_BUILD_WARNINGS=deny`. `clippy::doc_markdown` reports identifiers written outside code spans. Whether a mention in a code span is also a link is a judgement rule.

### 7.8. Cargo Features and Platform Support

The `# Features` section of the crate-level documentation lists every Cargo feature of the crate, with whether it is on by default and what it enables. It names each feature in a code span. A crate that has no features still has the section, and the section says so. An item that exists only when a feature is enabled says so in its body, in the form "Available only with the `example-feature` feature." The name `example-feature` is an illustration and names no planned feature. An author MUST document every feature this way, because a user who cannot find what a feature does enables features by trial, and an agent that finds no default chooses one (Section 3.3).

An item that is unavailable or behaves differently on a platform or a graphics backend has a `# Platform support` section that names each such platform or backend and the difference.

Checked by: `check-feature-docs`, which reads the features of each library from `cargo metadata`, including the feature that an optional dependency defines. It reports a library whose crate-level documentation has no `# Features` section, a feature other than `default` that the section does not name in a code span, and a code span in the section that looks like a feature name but names no feature of the package. What the section says about each feature, including its default and what it enables, the sentence on an item that needs a feature, and the content of each `# Platform support` section are judgement rules.

### 7.9. Generated Raw Bindings

A crate of generated raw bindings, such as `slang-compiler-sys`, has full crate-level documentation as Section 7.4 defines. That documentation also says that the generated items mirror the Slang API, links to the Slang documentation for their meaning, and points users to the safe crate.

The generated items are exempt from item documentation, from `# Safety` sections and safety comments, and from the naming and API lints that generated code cannot meet.

The exemption from `# Safety` sections and safety comments holds only when the crate-level documentation states one safety contract that applies to every generated function and thunk. The contract says what the caller must guarantee, which is that every pointer it passes is valid, correctly typed, live, and obtained from Slang. It states the threading and lifetime rules of Slang that a caller follows. It says that a call that breaks the contract is undefined behaviour. An author MUST write this contract before the exception lists a lint of safety documentation, because the generated unsafe code has no safety documentation of its own, so without the contract a caller has no statement of what makes a call sound. The exemption exists because the generator cannot write a contract for each item, since the headers state none, and the generator emits no comments at all.

The exemption is one `#[expect]` attribute with a reason. It sits on the declaration of the generated module and lists each lint that fires on that module. Section 4.4 of the standards document `docs/rust-coding-standards.md` lists the lints that typically fire and says how the set is determined at each regeneration. An author MUST NOT list a lint that does not fire, because rustc reports such an expectation through `unfulfilled_lint_expectations`, which fails the check. An author MUST NOT apply the exemption to the whole crate or to hand-written code, because a broad exemption hides every undocumented hand-written item from the lints. Hand-written items in the same crate follow Sections 7.2 to 7.8 in full.

This illustration shows the form of the exemption, with a typical set of lints for bindings that contain thunks:

```rust
#[expect(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    missing_docs,
    clippy::exhaustive_structs,
    clippy::exhaustive_enums,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks,
    reason = "generated bindings mirror the Slang API, which names and documents each item, and the crate documentation states their shared safety contract"
)]
pub mod generated;
```

Checked by: `clippy::allow_attributes` and `clippy::allow_attributes_without_reason` require the exemption to be an `#[expect]` with a reason. Rustc warns through `unfulfilled_lint_expectations` when an expected lint no longer fires, so a stale exemption is reported. Whether the crate-level documentation states the shared safety contract, and what the contract says, are judgement rules.

### 7.10. README and Crate Documentation

The README describes the project as a whole: what it is, the list of crates with one sentence and a documentation link for each, how to obtain the native Slang library, the build requirements, and the licence. Crate-level documentation is the API reference of one crate. Each fact lives in one of the two places, and the other links to it (Section 4.5). An author MUST NOT copy API reference material into the README, because rustdoc does not check the copy, so it diverges from the code.

Rust code in the README is compiled as a doctest by including the README in a crate. This illustration shows the form, with a path that depends on where the crate lives:

```rust
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub struct ReadmeDoctests;
```

Checked by: `cargo test --doc` for Rust code in the README. `scripts/check-prose` for the language of the README, as Section 8 describes. The division of content is a judgement rule.

### 7.11. Language in API Documentation

The rules of Section 8 apply to the prose of API documentation. Code spans and code examples are exempt. The section headings in Sections 7.3 and 7.4 keep their standard names and are exempt from the title-case rule in Section 8.

Checked by: `check-prose`, which reads the `///` and `//!` comments of every Rust file, for the rules of Section 8 that it checks: ASCII punctuation, which excludes em dashes, en dashes, and smart quotes, the absence of semicolons, and the spellings in `scripts/prose-wordlist.txt`. It skips code spans and fenced code blocks. The other rules of Section 8, such as spelling outside the word list, the serial comma, and title case, are judgement rules in API documentation, as they are in Markdown.

### 7.12. Summary of Checks

This table summarizes how each rule in this section is checked.

| Rule | Section | Checked by |
| --- | --- | --- |
| Plain English, terms defined on first use | 7.1 | Judgement |
| API documentation on every public item | 7.2 | rustc `missing_docs` |
| No comments in Rust source other than those listed | 7.2 | `check-rust-comments` |
| Only comments addressed to a human in other files | 7.2 | Judgement |
| One-sentence summary, then when to use | 7.3 | Judgement |
| Only the standard headings in item documentation, each once and in order | 7.3 | `check-doc-headings` |
| No `# Safety` on a safe item | 7.3 | `clippy::unnecessary_safety_doc` |
| Crate-level documentation exists | 7.4 | `rustdoc::missing_crate_level_docs` |
| Module documentation exists | 7.4 | rustc `missing_docs` |
| A `# Features` section exists in crate-level documentation | 7.4 | `check-feature-docs` |
| Other content of crate and module documentation | 7.4 | Judgement |
| Every public item has an example | 7.5 | Judgement |
| Examples compile and run | 7.5 | `cargo test --doc` |
| Well-formed code blocks | 7.5 | `rustdoc::invalid_rust_codeblocks`, `rustdoc::invalid_codeblock_attributes` |
| No visible `fn main` in examples | 7.5 | `clippy::needless_doctest_main` |
| `?` instead of `unwrap`, and the style of example code | 7.5 | Judgement |
| No code block marked `ignore` or `ignore-` | 7.5 | `check-doctest-attributes` |
| The lead-in states why an example is `no_run` or a `text` block | 7.5 | Judgement |
| `# Errors` exists | 7.6 | `clippy::missing_errors_doc` |
| `# Panics` exists | 7.6 | `clippy::missing_panics_doc` |
| An abort is stated in a body paragraph that begins with "Aborts", in the crate documentation for the general case and on each function whose abort was observed | 7.6 | Judgement |
| `# Safety` exists on public unsafe functions | 7.6 | `clippy::missing_safety_doc` |
| `# Safety` exists on private unsafe functions and on unsafe traits | 7.6 | Judgement |
| Safety comment exists | 7.6 | `clippy::undocumented_unsafe_blocks` |
| Content of the contract sections and safety comments | 7.6 | Judgement |
| Links resolve and web addresses are links | 7.7 | `rustdoc::broken_intra_doc_links`, `rustdoc::private_intra_doc_links`, `rustdoc::bare_urls` |
| Identifiers in code spans | 7.7 | `clippy::doc_markdown` |
| Every mentioned item is linked | 7.7 | Judgement |
| Every feature, and no other name, in a code span in the `# Features` section | 7.8 | `check-feature-docs` |
| Default and effect of each feature, items that need a feature, and platform support | 7.8 | Judgement |
| Exemption for generated bindings is an `#[expect]` with a reason | 7.9 | `clippy::allow_attributes`, `clippy::allow_attributes_without_reason` |
| Exemption applies to generated items only | 7.9 | Judgement |
| A shared safety contract in the crate documentation covers generated unsafe code that has no `# Safety` section or safety comment | 7.9 | Judgement |
| Rust code in the README compiles | 7.10 | `cargo test --doc` |
| Language of the README | 7.10 | `check-prose` |
| Division of content between README and crate documentation | 7.10 | Judgement |
| ASCII punctuation, no semicolons, and the listed spellings in API documentation | 7.11 | `check-prose` |
| Other language rules in API documentation | 7.11 | Judgement |

The names in backticks that start with `check-` are `just` recipes. Each runs a script of the same name under `scripts/`, and `just check` runs every one of them, as Section 7 of `docs/design/tooling.md` specifies. In a Rust file, the checkers that read documentation read only `///` and `//!` comments. They do not read documentation written as a `#[doc = ...]` attribute, so for such documentation the rules that they enforce are judgement rules.

A lint enforces a rule only when the workspace lint configuration enables it and the checks treat its warnings as errors. Several of these lints are off by default, including rustc `missing_docs`, `rustdoc::missing_crate_level_docs`, and the lints in the Clippy `pedantic` and `restriction` groups. The standards document `docs/rust-coding-standards.md` defines the lint configuration. Until the configuration enables a lint, the rule it checks is a judgement rule, as Section 3.5 states.

These commands run the checks in this section:

```sh
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features
CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features
CARGO_BUILD_WARNINGS=deny cargo test --doc --workspace --all-features
```

`CARGO_BUILD_WARNINGS=deny` makes each command fail on a warning, including a rustdoc lint warning in `cargo doc`. It does not reach the code inside a doctest, which no tool lints (Section 7.5). Section 4.8 of the standards document gives the full set of checks and the limits of these commands.

These recipes run the checkers of this section on every Rust file, and `check-doctest-attributes` also on every `README.md` file. Each also takes the paths of the files to check, and `check-feature-docs` takes the paths of Cargo manifests:

```sh
just check-rust-comments
just check-doc-headings
just check-doctest-attributes
just check-feature-docs
just check-prose
```

With no paths, `check-prose` also checks every Markdown file.

## 8. Language and Punctuation

Prose uses Canadian English spelling, as in colour, behaviour, organize, and licence as a noun. Consistent spelling lets an agent find every occurrence of a word with a single search.

Punctuation is ASCII. Prose uses no em dashes or en dashes, and an author writes a hyphen or a full stop instead. Prose uses no semicolons, and an author uses a full stop or a joining word instead. Code is exempt from this rule. Quotation marks are straight, never smart quotes. Non-ASCII characters appear only in code blocks and code spans where an example needs them [RFC7997]. Lists of three or more items take the serial comma, following Section 3.2 of [RFC7322].

Headings are in title case. A citation tag has square brackets with no space, such as [RFC2119], and a matching entry in the References section. A document may instead resolve a whole family of tags through one pointer, stated in its References section, to the reference list. Section 21 of `docs/rust-coding-standards.md` does this for its `[IR-<n>]` and `[S-<n>]` tags. A citation of a section reads "Section 6 of [RFC2119]". A Uniform Resource Identifier (URI) in a reference entry is enclosed in angle brackets. Dates use the International Organization for Standardization (ISO) 8601 format, such as 2026-10-30.

The script `scripts/check-prose` checks every Markdown file, and the `///` and `//!` documentation comments of every Rust file, for non-ASCII characters, which include em dashes, en dashes, and smart quotes, for a UTF-8 byte order mark, for semicolons, and for the non-Canadian spellings listed in `scripts/prose-wordlist.txt`. It skips fenced code blocks and code spans, and it reports a code fence left open at the end of a file. The pre-commit hook runs it on every changed Markdown file and every changed Rust file, and the Claude Code hook runs it after every edit of a Markdown file or a Rust file. Spelling outside the word list, the serial comma, title case, and the citation and date formats are judgement rules.

This command checks one file:

```sh
just check-prose docs/writing-documentation.md
```

## 9. Maintenance

An author updates documentation in the same change as the code it describes, never in a later one. A document that no longer describes the code is a defect of the same severity as the code bug it would cause, because the next agent to read it implements what it says. Each document that has a change log records every change to the document as a new entry, newest first, with the date and a one-sentence description.

A reviewer checks only judgement rules, because the tools in Section 7.12 and Section 8 check the rest on every change. A reviewer checks these items for every change:

1. Every document and every item of API documentation that mentions the changed behaviour, files, commands, features, or configuration is updated in the same change.
2. The change log of each touched document has a new entry.
3. Every new term appears in the Terminology section.
4. Every new requirement names its role and states its consequence.
5. Every example in a Markdown document that the change affects still runs or verifies.
6. Every section citation still points to the intended section.
7. Every changed security considerations section states the threat model, the attacks out of scope, and the residual risk (Section 3.2).
8. Every changed public item meets each rule that Section 7.12 marks as a judgement rule.
9. Every changed document meets the judgement rules of Section 8.

## 10. Security Considerations

This document is procedural and defines no component. Its first security consideration is that an agent fills a gap in an ambiguous document by guessing, and a guess about a security setting can produce an insecure default. The requirements in Section 3.3, that every option states its default and that every security check defaults to on, prevent this. Its second security consideration is that an incomplete `# Safety` section or safety comment lets a caller or a later maintainer break a condition that unsafe code relies on, which causes undefined behaviour. The requirements in Section 7.6 prevent this. The threat model is an author who leaves a setting or a safety condition unspecified, not an attacker. Attacks on the repository itself, such as a malicious change to a document, are out of scope because this guide governs writing and not access control. The residual risk is a document or an item of API documentation that a reviewer accepts without these requirements, and the content of a safety contract, which no tool checks.

## 11. References

### 11.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC7322] Flanagan, H. and S. Ginoza, "RFC Style Guide", RFC 7322, September 2014, <https://www.rfc-editor.org/info/rfc7322>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 11.2. Informative References

- [RFC1034] Mockapetris, P., "Domain names - concepts and facilities", STD 13, RFC 1034, November 1987, <https://www.rfc-editor.org/info/rfc1034>.
- [RFC2360] Scott, G., "Guide for Internet Standards Writers", BCP 22, RFC 2360, June 1998, <https://www.rfc-editor.org/info/rfc2360>.
- [RFC2616] Fielding, R., Gettys, J., Mogul, J., Frystyk, H., Masinter, L., Leach, P., and T. Berners-Lee, "Hypertext Transfer Protocol -- HTTP/1.1", RFC 2616, June 1999, <https://www.rfc-editor.org/info/rfc2616>.
- [RFC3552] Rescorla, E. and B. Korver, "Guidelines for Writing RFC Text on Security Considerations", BCP 72, RFC 3552, July 2003, <https://www.rfc-editor.org/info/rfc3552>.
- [RFC3986] Berners-Lee, T., Fielding, R., and L. Masinter, "Uniform Resource Identifier (URI): Generic Syntax", STD 66, RFC 3986, January 2005, <https://www.rfc-editor.org/info/rfc3986>.
- [RFC5234] Crocker, D., Ed. and P. Overell, "Augmented BNF for Syntax Specifications: ABNF", STD 68, RFC 5234, January 2008, <https://www.rfc-editor.org/info/rfc5234>.
- [RFC6455] Fette, I. and A. Melnikov, "The WebSocket Protocol", RFC 6455, December 2011, <https://www.rfc-editor.org/info/rfc6455>.
- [RFC7997] Flanagan, H., Ed., "The Use of Non-ASCII Characters in RFCs", RFC 7997, December 2016, <https://www.rfc-editor.org/info/rfc7997>.
- [RFC8259] Bray, T., Ed., "The JavaScript Object Notation (JSON) Data Interchange Format", STD 90, RFC 8259, December 2017, <https://www.rfc-editor.org/info/rfc8259>.
- [RFC8446] Rescorla, E., "The Transport Layer Security (TLS) Protocol Version 1.3", RFC 8446, August 2018, <https://www.rfc-editor.org/info/rfc8446>.
- [RFC9110] Fielding, R., Ed., Nottingham, M., Ed., and J. Reschke, Ed., "HTTP Semantics", STD 97, RFC 9110, June 2022, <https://www.rfc-editor.org/info/rfc9110>.
- [RFC9293] Eddy, W., Ed., "Transmission Control Protocol (TCP)", STD 7, RFC 9293, August 2022, <https://www.rfc-editor.org/info/rfc9293>.
- [RFC9413] Thomson, M. and D. Schinazi, "Maintaining Robust Protocols", RFC 9413, June 2023, <https://www.rfc-editor.org/info/rfc9413>.
- [RUST-API] The Rust Library Team, "Rust API Guidelines: Documentation", <https://rust-lang.github.io/api-guidelines/documentation.html>.
- [RUSTDOC] The Rust Project, "The rustdoc book", <https://doc.rust-lang.org/rustdoc/>.

## Appendix A. Design Document Skeleton

This skeleton is a compliant design document with placeholders in angle brackets. An author copies it into `docs/design/` and replaces every placeholder.

````markdown
# <Component Name> Design

| Field | Value |
| --- | --- |
| Status | <Draft or Living document> |
| Date | <YYYY-MM-DD> |
| Applies to | <crate, directory, or component> |

## Abstract

<One self-contained paragraph: what the component is, what problem it solves, and what this document specifies. No citations.>

## 1. Introduction

<The problem, then the purpose of the component, its audience, its scope, and its non-goals.>

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

- <term>: <definition>

## 3. <First Topic>

<Rule, then rationale in its own paragraph, then an example with a one-line lead-in.>

## 4. Security Considerations

<Threat model, attacks out of scope and why, and residual risk. Cover unsafe code and FFI invariants, untrusted shader source and file paths, build scripts that run native builds, and dependencies.>

## 5. Operational Considerations

<Every Cargo feature and every environment variable with its default, supported platforms and backends, the minimum supported Rust version, failure modes with the error type that reports each and the recovery, and the logs the crate emits.>

## 6. References

### 6.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 6.2. Informative References

- <entry>

## Change Log

- <YYYY-MM-DD>: <One-sentence description.>
````

## Change Log

- 2026-11-12: Corrected in Section 7.5 the reason why a call of a procedural macro that resolves a path against the file that calls it cannot be a doctest. In a doctest, that file is the source file that holds the documentation.
- 2026-11-11: Added to Section 7.5 the exception for an example that cannot compile as a doctest, such as a call of a procedural macro that resolves a path against its invoking file, which is a `text` block whose lead-in states the reason, and extended its row in Section 7.12.
- 2026-11-11: Added to Section 7.6 that a function that can abort the process states each condition of the abort in a paragraph of the body that begins with "Aborts", with no heading of its own, that the crate-level documentation states the general case once in such a paragraph, and that a function carries its own paragraph only when its abort was observed, and added its row to Section 7.12.
- 2026-11-10: Widened the exemption of Section 7.9 for generated raw bindings to `# Safety` sections and safety comments, on the condition that the crate-level documentation states one shared safety contract, and added cross-references in Sections 7.2 and 7.6 and a row in Section 7.12, and replaced the feature name in the example sentence of Section 7.8 with an illustrative one.
- 2026-11-10: Recorded in Section 8 that the Claude Code hook also runs the prose checker after an edit of a Rust file.
- 2026-11-10: Moved the rules that the new custom checkers and the extended prose checker enforce from judgement to enforced in Sections 7.2 to 7.5, 7.8, 7.11, 7.12, and 8, and made the heading rule of Section 7.3 a MUST NOT that matches the heading checker.
- 2026-11-10: Initial version.
