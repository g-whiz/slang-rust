# Rust Coding Standards

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-10 |
| Applies to | All Rust code in this repository, and the agents that write or review it |

## Abstract

This document sets the rules that all Rust code in this repository follows. It separates enforced rules, which a lint or tool checks on every change, from judgement rules, which a reviewer checks because no tool can. For the enforced rules, it specifies the intended workspace lint configuration and the other tools, and it explains what a failing check means and how to fix it or record an exception. For the judgement rules, it gives each rule a stable identifier, an imperative statement, and the reason, grouped by topic: API design, type-driven design, errors and panics, ownership, unsafe code, the foreign function interface, build scripts, COM-style interfaces, concurrency and async, testing, dependencies, and simplicity. It also records how conflicts between sources were resolved, lists outdated advice to avoid, and says how to extend the standard. The primary readers are the coding agents that write Rust here and the review agents that check their work.

## 1. Introduction

The crates in this repository wrap the Slang shader compiler, a C++ library with an interface in the style of the Component Object Model (COM), and build higher-level libraries on top of it. Defects in such code fall into two classes. The first class is mechanical, such as an `unwrap` on a fallible path, a missing safety comment, or a lossy cast, and a deterministic tool finds it every time. The second class is a design defect, such as a child object that can outlive its parent, an error that loses the failed operation, or a `Send` implementation that the native library does not support, and only a reviewer can find it. This document assigns every rule to one of the two classes, following Section 3.5 of the writing guide: a rule that a deterministic tool can check is checked by that tool and never by a reviewer.

### 1.1. Scope

This document applies to every Rust crate, build script, example, test, and tool in the repository. No crate has been written yet. The one crate directory present, `crates/slang-compiler-sys`, is an abandoned sketch that this standard does not describe and that will be rewritten. The Project Status section of `AGENTS.md` gives the details. The planned crates are:

- `slang-compiler-sys`: the raw bindings to the native Slang compiler library. The binding generator produces them, and they are checked in. Its build script links the native library, and its manifest declares `links = "slang-compiler"`.
- `slang-compiler`: the safe crate that wraps `slang-compiler-sys` in idiomatic Rust.
- `slango`: a high-level convenience library with a feature set like SlangPy and design patterns like vulkano. Its runtime backend is wgpu, and its design leaves room for a slang-rhi backend later.
- `slang-slint`: an extension to Slint that provides a view backed by a Slang render pipeline. It uses `slango` and supports wgpu only.
- The binding generator: a tool that parses the Slang headers with libclang and writes the raw bindings.
- Examples of varying complexity, which double as a testbed.

Bindings for slang-rhi (`slang-rhi-sys` and `slang-rhi`) are deferred and out of scope. Rules ERR-1 and ERR-2 apply to library crates only, because the binding generator and the examples are applications, which may report errors through `Box<dyn std::error::Error>`.

This document does not govern documentation, commit messages, or the design of any particular crate. The writing guide covers documentation, and the design documents in `docs/design/` cover each crate. This document does not name `just` recipes or define continuous integration (CI) workflows. `docs/design/tooling.md` specifies them.

Every lint name, lint group, and default level in this document was verified against the Rust 1.99.1 stable toolchain, including Cargo and Clippy, for crates on edition 2024 with Cargo resolver 3, unless Section 4.7 says otherwise or the text says that a statement is not yet verified. The whole configuration of Sections 4.2 and 4.3 was run on a scratch workspace with that toolchain, cargo-nextest 0.9.151, and cargo-deny 0.20.2, and it produced no diagnostic for an unknown, renamed, or removed lint. The commands of Section 4.8 for the other tools were run on a scratch workspace with cargo-semver-checks 0.51.0, cargo-hack 0.6.46, cargo-machete 0.9.2, cargo-llvm-cov 0.9.1, typos 1.52.1, taplo 0.10.0, actionlint 1.7.12, and the nightly toolchain 1.102.0 of 2026-11-09 with Miri. Statements in this document that describe what a lint or a tool reported come from those runs.

### 1.2. How to Use This Document

An agent that writes code reads Sections 3 and 4 once, and then the sections for the topics its change touches. An agent whose check fails reads the row for the failing lint in Section 4.5 and the procedure in Section 4.4. A reviewer loads Appendix A, which lists every judgement rule with its identifier and a one-line statement, and reads the full rule in Sections 4.4 to 16 when a finding needs it. Section 19 says what to do when a design question falls outside every rule.

Code contains only the comments that Section 7.2 of the writing guide lists, and API documentation follows Section 7 of the writing guide. This document does not repeat those rules.

### 1.3. Sources and Precedence

A judgement rule cites the sources that support it, when it has any. The reference list, `docs/rust-references.md`, defines every source identifier, holds the catalogue entry of every source, and indexes the sources by topic. A citation of the form `[IR-<n>]` names an entry of the curated list of idiomatic Rust resources, frozen at the upstream commit that Section 6.2 of the reference list records, so a later change to the upstream list does not move a citation. A citation of the form `[S-<n>]` names a supplementary source outside that list, such as a Rust project book, a guide to `-sys` crates, or an organizational guideline, chosen mainly for the foreign function interface (FFI), `-sys` crates, and unsafe code. Section 1.2 of the reference list defines both forms, and Section 21 says where the entry of every cited source is. Every rule is written in this document's own words, and none is a quotation.

When sources disagree, the source in the higher tier wins:

1. The Rust Reference [S-5], the standard library documentation [S-6], and the Rust API Guidelines [IR-8]. The other books that the Rust project publishes, such as the Rustonomicon [S-1], the Cargo Book [S-3], the Edition Guide [S-7], and the Clippy book [IR-4], rank with them for the tool or feature that each one documents.
2. Organizational standards, such as those of Canonical [IR-2], Sentry [IR-17], Microsoft [S-10], Google [IR-10], and Ferrous Systems [IR-13].
3. Individual articles, talks, and books.

Within a tier, the newer source wins. A decision that this document records takes precedence over every source, and Section 17 gives the reasoning for each decision that settled a disagreement.

## 2. Requirements Language

### 2.1. Key Words

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.2. Terminology

This document uses the terms that Section 2.2 of the writing guide defines, including agent, reviewer, public item, safety comment, enforced rule, and judgement rule, and adds these:

- writing guide: The guide `docs/writing-documentation.md`.
- binding generator: The tool in this repository that parses the Slang headers with libclang and writes the raw bindings of the `-sys` crate.
- `-sys` crate: A crate that contains only the raw declarations of a native library and the build script that links it. Here, `slang-compiler-sys`.
- safe crate: A crate that wraps a `-sys` crate in an API through which safe Rust cannot cause undefined behaviour. Here, `slang-compiler`.
- library crate: Any crate in the repository other than the binding generator and the examples.
- native library: The compiled Slang library that a `-sys` crate links.
- interface: An object type of the Slang API, in the style of COM. Code reaches it through a pointer to an object whose first field points to a vtable, and a reference count controls its lifetime.
- vtable: A table of function pointers through which the methods of an interface are called.
- thunk: A generated function that forwards a call to an entry of a foreign vtable.
- interface pointer: A raw pointer to an object that implements an interface.
- owning interface pointer: A Rust value that holds exactly one reference count on an interface and releases it when dropped.
- borrowed interface pointer: An interface pointer used without a reference count of its own, which is valid only while an owning interface pointer keeps the object alive.
- foreign handle: Any Rust value that holds a pointer to an object that native code owns, including an owning interface pointer.
- result code: The integer status that a Slang function returns to report success or failure.
- blob: An interface that holds a buffer of bytes.
- diagnostics blob: A blob that holds the messages that Slang produced for an operation, such as compile errors and warnings.
- universally unique identifier (UUID): The 128-bit identifier of an interface, which `QueryInterface` takes to select the interface.
- lint: A named check in rustc, rustdoc, or Clippy.
- workspace lint table: The `[workspace.lints]` tables of the root `Cargo.toml`, which Section 4.2 specifies.
- the check: Running the tools of Section 4.8 on a change with warnings treated as errors.
- lint exception: An `#[expect]` attribute that accepts the finding of one lint at one site.
- rule identifier: The label of a judgement rule, such as ERR-2, which Section 3 defines.

## 3. Rule Identifiers and Form

Every judgement rule has a rule identifier made of a topic prefix and a number. The prefixes are:

| Prefix | Topic | Section |
| --- | --- | --- |
| LINT | Lint exceptions and the lint configuration | 4.4 |
| API | API design | 5 |
| TYPE | Type-driven design | 6 |
| ERR | Errors and panics | 7 |
| OWN | Ownership and signatures | 8 |
| UNS | Unsafe code | 9 |
| FFI | Foreign function interface | 10 |
| SYS | Build scripts of `-sys` crates | 11 |
| COM | COM-style interfaces | 12 |
| CONC | Concurrency and async | 13 |
| TEST | Testing | 14 |
| DEP | Dependencies and Cargo features | 15 |
| SIMP | Simplicity | 16 |

Identifiers are never reused or renumbered. A withdrawn rule keeps its identifier, and its statement is replaced by the word "Withdrawn" and the date. A new rule takes the next unused number in its topic. This matters because commits, reviews, and other documents cite rules by identifier, and a reused or renumbered identifier makes an old citation point to a different rule.

Each rule has an imperative statement, the reason, and the sources that support it, if any. The statement is normative and binds the agent that writes or changes the code, and a reviewer checks each change against it and cites the identifier in each finding. Where a tool enforces part of a rule, the rule covers only the part that the tool cannot check. A code snippet in a rule is an illustration of the form, and its names do not describe an existing API.

## 4. Lint and Tool Configuration

### 4.1. Status of the Configuration

None of the configuration in this section exists in the repository yet. This section is its specification. An agent that creates the workspace manifest MUST add the tables of Sections 4.2 and 4.3 as written here, or update this section in the same change, because an agent that reads a rule here assumes that a tool enforces it. As Section 7.12 of the writing guide states, a lint enforces a rule only once the configuration enables it and the check treats its warnings as errors. Until then, each rule in this section is a judgement rule, as Section 3.5 of the writing guide requires.

### 4.2. Workspace Lint Table

The root `Cargo.toml` contains this configuration:

```toml
[workspace]
resolver = "3"

[workspace.lints.rust]
unsafe_code = "deny"
unsafe_op_in_unsafe_fn = "deny"
missing_docs = "warn"
missing_debug_implementations = "warn"
unreachable_pub = "warn"
unnameable_types = "warn"
elided_lifetimes_in_paths = "warn"
unused_lifetimes = "warn"
redundant_lifetimes = "warn"
trivial_casts = "warn"
trivial_numeric_casts = "warn"
unused_qualifications = "warn"
let_underscore_drop = "warn"
macro_use_extern_crate = "warn"
ffi_unwind_calls = "warn"
ambiguous_negative_literals = "warn"
redundant_imports = "warn"

[workspace.lints.rustdoc]
all = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
cargo = { level = "warn", priority = -1 }
multiple_crate_versions = "allow"

unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
unreachable = "warn"
todo = "warn"
unimplemented = "warn"
dbg_macro = "warn"
print_stdout = "warn"
print_stderr = "warn"
map_err_ignore = "warn"
let_underscore_must_use = "warn"
unused_result_ok = "warn"

undocumented_unsafe_blocks = "warn"
multiple_unsafe_ops_per_block = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
mem_forget = "warn"

allow_attributes = "warn"
allow_attributes_without_reason = "warn"

exhaustive_enums = "warn"
exhaustive_structs = "warn"
partial_pub_fields = "warn"
mod_module_files = "warn"
```

Every member crate's `Cargo.toml` contains this table and no other lint keys, because Cargo rejects a member that inherits the workspace lints and also sets a lint of its own:

```toml
[lints]
workspace = true
```

The baseline is the Clippy group `all`, which contains the groups `correctness`, `suspicious`, `complexity`, `perf`, and `style`, together with the groups `pedantic` and `cargo`. The `nursery` group is off. The `restriction` group is never enabled as a whole, and Section 4.5 gives the purpose of each restriction lint in the table. The rustdoc group `all` already contains `rustdoc::missing_crate_level_docs`, `rustdoc::unescaped_backticks`, and `rustdoc::private_doc_tests`, so the table names no rustdoc lint individually.

The groups have priority -1, so each individual lint, at the default priority 0, overrides its group. Without the lower priority, `clippy::lint_groups_priority` reports the table and the check fails. The lint is in the `correctness` group, so it reports as a warning here, as the next paragraph explains.

The levels are `warn`, and the check runs with `CARGO_BUILD_WARNINGS=deny`, so every warning fails the check. Setting `clippy::all` to `warn` lowers the `correctness` group from its default `deny` to `warn`, which the check still treats as a failure. The two `deny` entries are soundness rules that also stop a plain `cargo build`.

An agent MUST NOT add `#![deny(warnings)]` to source and MUST NOT pass `-D warnings` through `RUSTFLAGS` or after `--` on the Clippy command line, because a new compiler release adds warnings and then breaks the build for users, and `-D warnings` in the compiler flags invalidates the build cache.

### 4.3. Clippy Configuration File

The file `clippy.toml` at the root of the workspace contains this configuration:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-print-in-tests = true
avoid-breaking-exported-api = false
doc-valid-idents = ["SlangPy", "DirectX12", "MoltenVK", ".."]
```

This table says why each option has its value:

| Option | Reason |
| --- | --- |
| `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-print-in-tests` | A panic or a print in a test reports the failure where it happens, so the restriction lints apply to non-test code only. |
| `avoid-breaking-exported-api = false` | Every crate is below 1.0, so Clippy also reports the findings whose fix changes the public API. Section 4.5 lists the findings that this setting adds. |
| `doc-valid-idents` | Names that `clippy::doc_markdown` flagged and that are not identifiers, so they stay outside code spans. The value `".."` keeps Clippy's default list. `Slang` and `Vulkan` are not flagged, because they are not written in CamelCase, and `WebGPU` is in the default list. A name is added only after the lint flags it. |

The file leaves `check-private-items` at its default, `false`. With `true`, Clippy would require `# Errors` and `# Panics` sections on private items, which Section 7.2 of the writing guide does not allow. The `# Safety` section on a private unsafe function therefore stays a judgement rule, as Section 7.6 of the writing guide states.

Clippy reads `clippy.toml` and rejects an unknown key with an error, so a misspelled option fails the check instead of being ignored. Every key above was accepted on the toolchain of Section 1.1.

Clippy reads the minimum supported Rust version (MSRV) from `rust-version` in the manifest (DEP-4), so `clippy.toml` does not set `msrv`.

### 4.4. Responding to a Failing Check

An agent whose check fails follows these steps:

1. Read the explanation of the lint. For a Clippy lint, run `cargo clippy --explain <lint>`, such as the command after this list.
2. Read the row of the lint in Section 4.5 and the rule it enforces.
3. Fix the code as the lint and the rule direct.
4. If the finding is wrong at this site, add a lint exception as this section defines.

This command explains one Clippy lint:

```sh
cargo clippy --explain unwrap_used
```

An agent MUST record every lint exception as `#[expect(lint, reason = "...")]` and MUST NOT use `#[allow]`, because an `#[expect]` whose lint stops firing produces `unfulfilled_lint_expectations`, which fails the check and so removes stale exceptions, while an `#[allow]` hides the lint silently for as long as it stays. `clippy::allow_attributes` and `clippy::allow_attributes_without_reason` enforce the form and the presence of a reason.

A lint exception has to be fulfilled in every target and feature combination that is checked, or `unfulfilled_lint_expectations` fails the check in each combination where the lint does not fire. This happens with code under `#[cfg]`, with code behind a Cargo feature, and with lints whose result depends on the target, such as the cast lints on `usize` (TYPE-5). An agent SHOULD fix such code instead of adding a lint exception, or the check fails on a target or a feature combination that the agent did not build.

**LINT-1.** Put each lint exception on the narrowest item or statement that needs it, and write a reason that states why the lint is wrong at that site, in terms a reviewer can check. Reason: a broad exception hides later findings in the same scope, and a reason such as "not needed" cannot be checked.

**LINT-2.** Change the tables of Sections 4.2 and 4.3 only in the same change that updates Sections 4.5 to 4.7 and the rule that the lint enforces. Never change a table to silence a finding at one site. Reason: the tables and this document describe one configuration, and a table changed alone makes the document promise a check that no longer runs.

Three kinds of lint exception are expected and follow fixed forms:

- A crate that calls foreign code opts out of `unsafe_code` at its crate root with `#![expect(unsafe_code, reason = "...")]`. The planned crates that do so are `slang-compiler-sys` and `slang-compiler`. Any other crate that needs `unsafe` records the reason in its design document first. The workspace denies `unsafe_code` instead of forbidding it, because `forbid` cannot be overridden even by `#[expect]`. A crate adds the opt-out together with its first `unsafe` code, because the expectation is unfulfilled, and fails the check, in a crate that contains none.
- The module of generated bindings carries an `#[expect]` with a reason that lists each lint that fires on it, as Section 7.9 of the writing guide defines and the next paragraph describes.
- An example or the binding generator that prints to the terminal opts out of `clippy::print_stdout` or `clippy::print_stderr` at its crate root. A build script carries no exception for `clippy::print_stdout`, because Clippy does not apply that lint to build scripts, so the expectation would be unfulfilled and fail the check.

An `#[expect]` on the `mod` declaration of the generated module covers every lint that fires in the module's file, including the lints that fire on `#[allow]` attributes inside the file. On generated code in the style of bindgen, these lints fired: `non_camel_case_types`, `non_snake_case`, `non_upper_case_globals`, `missing_docs`, `clippy::exhaustive_structs`, and `clippy::exhaustive_enums`. When the generated code contains its own `#[allow(...)]` attributes, `clippy::allow_attributes` and `clippy::allow_attributes_without_reason` fire as well, and when the module brings in its code with `include!`, `missing_debug_implementations` fires as well. It is not yet verified whether `clippy::enum_variant_names` and `clippy::upper_case_acronyms` fire on generated enums written as Rust enums, which `avoid-breaking-exported-api = false` would allow. The set changes when the Slang headers change, so the agent that regenerates the bindings runs the check and sets the list to the lints that fire. Code that `slint::include_modules!` generates probably needs the same kind of exception, which is not yet verified.

### 4.5. Enforced Lints by Purpose

These tables group the enforced lints by purpose. The Level column says where the lint comes from: "table" for an entry of Section 4.2, a Clippy group name for a lint that its group enables, and "default" for a lint that rustc enables by default. The last column names the judgement rule that the lint supports, where one exists.

Baseline groups:

| Lint or group | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::all` | table | Code that is wrong, suspicious, needlessly complex, slow, or unidiomatic. Apply the suggestion. | - |
| `clippy::pedantic` | table | A stricter check of idiom, documentation, or casts fired. Apply the suggestion or add a lint exception. | - |
| `clippy::cargo` | table | A feature name is negative or redundant, or a dependency uses a `*` version. The next row covers missing metadata. | DEP-1 |
| `clippy::cargo_common_metadata` | `cargo` | A published member lacks one of `description`, `license`, `repository`, `readme`, `keywords`, and `categories`, and `readme = false` counts as missing. Add the fields, or set `publish = false` on a member that is never published, such as the examples crate and the binding generator, which the lint then skips. The lint reports every workspace member whichever crate is checked, so a lint exception in one crate cannot accept it. | - |

Panics and unfinished code:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::unwrap_used`, `clippy::expect_used` | table | An `unwrap` or `expect` outside tests. Return an error, or use the form of ERR-6 for a site that cannot fail. The test options of Section 4.3 do not cover examples or build scripts, so an example returns `Result` from `main` and uses `?`, and a build script does the same or uses the form of ERR-6. | ERR-5, ERR-6 |
| `clippy::panic`, `clippy::unreachable` | table | A `panic!` or `unreachable!` outside tests. Return an error, or add a lint exception that states the invariant. | ERR-5, ERR-6 |
| `clippy::todo`, `clippy::unimplemented` | table | Unfinished code. Finish it before the change is committed. | ERR-6 |
| `clippy::dbg_macro` | table | A leftover `dbg!`. Remove it. | - |
| `clippy::print_stdout`, `clippy::print_stderr` | table | A library prints to the terminal. Return the information to the caller instead. | - |

Errors and discarded results:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::map_err_ignore` | table | A `map_err` closure discards the original error. Keep it as the `source()`. | ERR-1, ERR-3 |
| `clippy::let_underscore_must_use` | table | `let _ = expr;` discards a `#[must_use]` value such as a `Result`. Handle it, or use the discard form of ERR-5. The lint does not detect `_ = expr;` or `let _name = expr;`. | ERR-5 |
| `clippy::unused_result_ok` | table | `expr.ok();` discards a `Result`. Handle it. The suggested fix, `let _ = expr;`, then needs the discard form of ERR-5. | ERR-5 |
| rustc `unused_must_use` | default | A `Result` or another `#[must_use]` value is ignored. | ERR-5 |
| `clippy::result_unit_err` | `style` | A public function returns `Result<_, ()>`. Use the crate's `Error`. | ERR-2 |
| `clippy::result_large_err` | `perf` | The error type is larger than the threshold, 128 bytes by default. Box the large fields. | ERR-1 |
| `clippy::missing_errors_doc`, `clippy::missing_panics_doc` | `pedantic` | API documentation lacks an `# Errors` or `# Panics` section (Section 7.6 of the writing guide). A public function that uses the `expect` form of ERR-6 also triggers `clippy::missing_panics_doc`. | ERR-6 |

Unsafe code and FFI:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| rustc `unsafe_code` | table | `unsafe` in a crate without the crate-root opt-out of Section 4.4. Use a safe alternative. | UNS-1 |
| rustc `unsafe_op_in_unsafe_fn` | table | An unsafe operation in the body of an `unsafe fn` outside an `unsafe` block. Wrap it in a block with a safety comment. | UNS-6 |
| `clippy::undocumented_unsafe_blocks` | table | An `unsafe` block or `unsafe impl` has no safety comment directly above it. | UNS-6 |
| `clippy::multiple_unsafe_ops_per_block` | table | An `unsafe` block holds more than one unsafe operation. Split it, so that each safety comment covers one operation. A vtable call written as `((*(*this).vtable).method)(this)` counts as three operations, so use the helper of COM-13. | UNS-2, UNS-6, COM-13 |
| `clippy::unnecessary_safety_comment`, `clippy::unnecessary_safety_doc` | table | Safety text on safe code. Remove it. | - |
| `clippy::missing_safety_doc` | `style` | A public `unsafe fn` has no `# Safety` section. | UNS-6 |
| `clippy::mem_forget` | table | `mem::forget` on a type with a destructor. Use `ManuallyDrop`, which the lint does not report, as `into_raw` does (COM-11). | UNS-4 |
| `clippy::not_unsafe_ptr_arg_deref` | `correctness` | A safe public function dereferences a raw pointer argument. Make it `unsafe`, or take a reference. | UNS-3 |
| `clippy::mut_from_ref` | `correctness` | A function returns `&mut T` from `&T`. | UNS-7 |
| rustc `invalid_reference_casting` | default | A cast from `&T` to `&mut T`. | UNS-7 |
| `clippy::ptr_as_ptr`, `clippy::ref_as_ptr`, `clippy::borrow_as_ptr`, `clippy::ptr_cast_constness`, `clippy::cast_ptr_alignment` | `pedantic` | A pointer cast written with `as`. Use `.cast()`, `ptr::from_ref`, or `&raw const`. | UNS-7 |
| rustc `ffi_unwind_calls` | table | A call to a foreign function with the `"C-unwind"` application binary interface (ABI). Use `"C"` unless FFI-7 permits unwinding. | FFI-7 |
| rustc `improper_ctypes`, `improper_ctypes_definitions` | default | A type without a C layout in a foreign signature. | FFI-3 |
| rustc `clashing_extern_declarations` | default | One foreign symbol declared with two signatures. | FFI-3 |
| rustc `missing_abi` | default | An `extern` block or function without an ABI string. | FFI-3 |
| rustc `dangling_pointers_from_temporaries` | default | A pointer taken from a temporary, such as `CString::new(s)?.as_ptr()`. Bind the value to a variable first. | FFI-4 |
| rustc `static_mut_refs` | default | A reference to a `static mut`. | CONC-4 |

Types and conversions:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::cast_possible_truncation`, `clippy::cast_sign_loss`, `clippy::cast_possible_wrap`, `clippy::cast_precision_loss`, `clippy::cast_lossless`, `clippy::checked_conversions` | `pedantic` | A numeric `as` cast that can lose information, or that `From` or `TryFrom` replaces. `clippy::cast_possible_truncation` fires on `usize as u32` on 64-bit targets, so use `try_from` there (TYPE-5). | TYPE-5 |
| rustc `trivial_casts`, `trivial_numeric_casts` | table | A cast to a type that the value already has or coerces to. Remove it. When `trivial_casts` fires on a pointer cast, the pedantic pointer-cast lints fire with it, and one fix clears both: `ptr::from_ref`, `&raw const`, or `.cast()`. | - |
| `clippy::fn_params_excessive_bools`, `clippy::struct_excessive_bools` | `pedantic` | A function or struct has several `bool` parameters or fields. | TYPE-2 |

API surface:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| rustc `missing_docs` | table | A public item has no API documentation (Section 7.2 of the writing guide). The lint also requires a `//!` line at the top of the root of every target, including each integration-test binary, example, build script, and binary. | - |
| rustc `missing_debug_implementations` | table | A public type does not implement `Debug`. | API-7 |
| rustc `unreachable_pub` | table | A `pub` item that the crate root does not export. Use `pub(crate)`. | API-10 |
| rustc `unnameable_types` | table | A public signature uses a type that users cannot name. | API-2 |
| rustc `private_interfaces`, `private_bounds` | default | A private type in a public signature. | API-10 |
| `clippy::exhaustive_enums`, `clippy::exhaustive_structs` | table | A public enum or struct without a decision about `#[non_exhaustive]`. Add `#[non_exhaustive]`, or a lint exception in the cases that API-9 lists. `clippy::exhaustive_structs` also fires on unit marker structs, and does not fire on a tuple struct with a private field. | API-9 |
| `clippy::partial_pub_fields` | table | A struct mixes public and private fields. | API-10 |
| `clippy::wrong_self_convention`, `clippy::should_implement_trait`, `clippy::from_over_into`, `clippy::new_without_default`, `clippy::new_ret_no_self` | `style` | A method prefix does not match its receiver, a method is named like a standard trait method, `Into` is implemented instead of `From`, a `new` without arguments has no `Default`, or `new` does not return `Self`. | API-1, API-4, API-5 |
| `clippy::must_use_candidate`, `clippy::return_self_not_must_use` | `pedantic` | A pure public function, or a builder method that returns `Self`, lacks `#[must_use]`. Add it. `clippy::must_use_candidate` was the most frequent finding on new code. | API-6 |

Lifetimes and signatures:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| rustc `elided_lifetimes_in_paths` | table | A type that has a lifetime is written without it. Write `Type<'_>`. | OWN-3 |
| rustc `unused_lifetimes`, `redundant_lifetimes` | table | A lifetime parameter that is unused or equal to another. | OWN-3 |
| `clippy::needless_lifetimes`, `clippy::elidable_lifetime_names` | `complexity`, `pedantic` | A named lifetime that elision covers. | OWN-3 |
| `clippy::ptr_arg` | `style` | A `&String`, `&Vec<T>`, or `&PathBuf` parameter. | OWN-1 |
| `clippy::needless_pass_by_value` | `pedantic` | A by-value parameter that the function only reads. | OWN-1 |
| `clippy::needless_range_loop` | `style` | An index loop that an iterator replaces. | OWN-7 |

Lint exceptions, imports, and layout:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::allow_attributes` | table | An `#[allow]` attribute. Use `#[expect]` (Section 4.4). | LINT-1 |
| `clippy::allow_attributes_without_reason` | table | A lint attribute without `reason`. | LINT-1 |
| rustc `unfulfilled_lint_expectations` | default | An `#[expect]` whose lint no longer fires. Remove it. | LINT-1 |
| `clippy::lint_groups_priority` | `correctness` | A lint group in a `[lints]` table has the same priority as one of its lints. | LINT-2 |
| `clippy::mod_module_files` | table | A module file named `foo/mod.rs` under `src/`. Rename it to `foo.rs`. The lint does not fire on `mod.rs` files under `tests/`, or on the layout of TEST-10. | - |
| `clippy::wildcard_imports`, `clippy::enum_glob_use` | `pedantic` | A glob import outside a test module. | API-10 |
| `clippy::non_std_lazy_statics` | `pedantic` | `lazy_static` or `once_cell` where `std::sync::LazyLock` serves. | DEP-2 |
| rustc `unused_qualifications` | table | A path qualified more than it needs. | - |
| rustc `redundant_imports` | table | An import of an item that is already in scope. Remove it. | - |
| rustc `ambiguous_negative_literals` | table | A negative literal followed by a method call, such as `-1i32.abs()`, where the method applies before the minus sign. Add parentheses. | - |
| rustc `let_underscore_drop` | table | `let _ =` drops a value with a destructor at once, such as a guard. Bind a guard that must stay alive to a named variable such as `_guard`. A deliberate discard follows ERR-5. | UNS-4, ERR-5 |
| rustc `macro_use_extern_crate` | table | `#[macro_use] extern crate`. Import macros with `use`. | - |
| rustc `unexpected_cfgs` | default | A `cfg` name that no `cargo::rustc-check-cfg` instruction declares. | SYS-1 |

Concurrency:

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::await_holding_lock`, `clippy::await_holding_refcell_ref` | `suspicious` | A lock guard or `RefCell` borrow is held across `.await`. | CONC-7 |
| `clippy::let_underscore_future` | `suspicious` | `let _ =` on a future, which drops it without running it. | CONC-7 |
| `clippy::let_underscore_lock` | `correctness` | `let _ =` on a lock guard, which releases the lock at once. | CONC-2 |
| `clippy::arc_with_non_send_sync` | `suspicious` | `Arc` of a type that is not `Send` and `Sync`. | FFI-9 |
| rustc `invalid_atomic_ordering` | default | An ordering that is invalid for the operation, such as `Release` on a load. | CONC-5 |

Expected findings on new code: these lints fired on realistic code in the run of Section 1.1 and need no change to the configuration. The last four fire on the public API only because of `avoid-breaking-exported-api = false` (Section 4.3).

| Lint | Level | A failure means | Rule |
| --- | --- | --- | --- |
| `clippy::manual_assert` | `pedantic` | An `if` block that only panics. Write `assert!`, with the message form of ERR-5. | ERR-5 |
| `clippy::items_after_statements` | `pedantic` | An item, such as a `fn` or a `const`, declared after statements in a block. Move it to the start of the block. | - |
| `clippy::implicit_hasher` | `pedantic` | A public function or `impl` that accepts `HashMap` or `HashSet` with the default hasher only. Make it generic over the hasher, or add a lint exception. | - |
| `clippy::unreadable_literal` | `pedantic` | A long numeric literal without `_` separators. Add them, as in `1_000_000`. | - |
| `clippy::many_single_char_names` | `pedantic` | Several single-character names in one scope. Give them descriptive names. | - |
| `clippy::len_without_is_empty` | `style` | A public type has `len` and no `is_empty`. Add `is_empty`. | - |
| `clippy::trivially_copy_pass_by_ref` | `pedantic` | A small `Copy` value passed by reference. Pass it by value. | OWN-1 |
| `clippy::unnecessary_wraps` | `pedantic` | A function that always returns `Ok` or `Some`. Return the value itself. | - |
| `clippy::enum_variant_names` | `style` | Enum variants that repeat the name of the enum or share a prefix or suffix. Rename them. | API-3 |
| `clippy::upper_case_acronyms` | `style` | A name written in capitals, such as `UUID`. Write it in Rust case, such as `Uuid`. | API-3 |

Documentation: the rustdoc group `all` of Section 4.2 and the documentation lints of Clippy enforce the rules of Section 7 of the writing guide, and Section 7.12 of the writing guide names the rule that each one enforces.

Dependencies: `clippy::multiple_crate_versions` is set to `allow` because the wgpu stack pulls in duplicate versions of transitive crates that no workspace change can remove, and `cargo deny` owns the policy on duplicate versions (Section 4.8).

### 4.6. Lints Deliberately Not Enabled

These lints exist and were verified, and this standard leaves them off for the reason given. The rule in the last column covers the topic as a judgement rule.

| Lint | Reason it is off | Rule |
| --- | --- | --- |
| Clippy `nursery` group | The group holds lints that are unfinished and prone to false positives, and the project keeps it off. | - |
| `clippy::non_send_fields_in_send_ty` (nursery) | The group is off. | FFI-9 |
| `clippy::significant_drop_in_scrutinee`, `clippy::significant_drop_tightening` (nursery) | The group is off. | CONC-2 |
| `clippy::fallible_impl_from` (nursery) | The group is off. | API-4 |
| `clippy::redundant_clone`, `clippy::needless_collect` (nursery) | The group is off. | OWN-6, OWN-7 |
| `clippy::future_not_send` (nursery) | The group is off, and the lint enforces the opposite of CONC-6. | CONC-6 |
| `clippy::derive_partial_eq_without_eq` (nursery) | The group is off. | API-7 |
| `clippy::too_long_first_doc_paragraph` (nursery) | The group is off. | Section 7.3 of the writing guide |
| `clippy::missing_const_for_fn`, `clippy::use_self` (nursery) | The group is off. A `const fn` is also a public promise that is hard to withdraw. | - |
| `clippy::transmute_undefined_repr` (nursery) | The group is off. | TYPE-6 |
| `clippy::or_fun_call` (nursery) | The group is off. | - |
| rustc `non_exhaustive_omitted_patterns` | It is unstable (tracking issue 89554), so it cannot be used on the stable toolchain. It would report new variants in a match on a `#[non_exhaustive]` enum from a sibling crate. | TYPE-7 |
| rustc `unqualified_local_imports` | It is unstable (tracking issue 138299), so it cannot be used on the stable toolchain. | - |
| Clippy `restriction` group as a whole | Several of its lints contradict each other, and `clippy::blanket_clippy_restriction_lints` warns against enabling it. | - |
| `clippy::as_conversions` | It bans every `as`, including pointer casts that FFI code needs. The pedantic cast lints cover numeric casts. | TYPE-5 |
| `clippy::indexing_slicing`, `clippy::arithmetic_side_effects` | They flag every index and every arithmetic operation, most of which are safe in numeric and graphics processing unit (GPU) code. | TYPE-5, TYPE-9 |
| `clippy::unneeded_field_pattern` | It forbids naming ignored fields as `field: _`, which TYPE-7 requires. | TYPE-7 |
| `clippy::self_named_module_files` | It requires `foo/mod.rs`, the opposite of `clippy::mod_module_files`. | - |
| `clippy::impl_trait_in_params` | It forbids `impl Trait` arguments, which API-13 uses at the public edge. | API-13 |
| `clippy::single_char_lifetime_names` | It forbids `'a` even where one borrow source makes a longer name useless. | OWN-3 |
| `clippy::error_impl_error` | It forbids a type named `Error` that implements `std::error::Error`, which ERR-1 requires. | ERR-1 |
| `clippy::tests_outside_test_module` | It fires on every `#[test]` in `tests/`, where TEST-1 and TEST-10 put integration tests. | TEST-10 |
| `clippy::arbitrary_source_item_ordering` | It imposes an order of items that this standard does not define. Its `clippy.toml` options exist, and the standard does not set them. | - |
| `clippy::module_name_repetitions` | It is in `restriction`, not in `pedantic` as older sources state, and this standard does not adopt it. | - |
| `clippy::unwrap_in_result`, `clippy::get_unwrap` | They duplicate `clippy::unwrap_used` and `clippy::expect_used`, so one site would need two lint exceptions. | ERR-6 |
| `clippy::panic_in_result_fn` | It also rejects `assert!` and `debug_assert!` in functions that return `Result`, which ERR-5 permits for invariants. | ERR-5 |
| `clippy::wildcard_enum_match_arm` | It also fires where a `_` arm is required, on `#[non_exhaustive]` enums from another crate. | TYPE-7 |
| `clippy::clone_on_ref_ptr`, `clippy::str_to_string` | They enforce spelling preferences only. | - |
| `clippy::missing_assert_message` | Every `assert_eq!` in tests would need a message. | - |
| `clippy::field_scoped_visibility_modifiers`, `clippy::multiple_inherent_impl` | They forbid `pub(crate)` fields and feature-gated `impl` blocks, which API-10 and DEP-1 allow. | API-10 |
| `clippy::std_instead_of_core` and its family | They matter only for `no_std` crates. | - |
| `clippy::missing_inline_in_public_items` | It asks for `#[inline]` on every public item. | SIMP-4 |
| rustc `missing_copy_implementations` | It suggests `Copy` on handle types that own a resource. | API-7 |
| rustc `single_use_lifetimes` | Its suggestions conflict with descriptive lifetime names. | OWN-3 |
| rustc `unused_crate_dependencies` | It reports false positives across the targets of one package. `cargo machete` covers unused dependencies (Section 4.8). | DEP-2 |
| rustc `unused_results` | It requires every result to be used, including results of no interest. | - |
| rustc `if_let_rescope`, `tail_expr_drop_order`, `impl_trait_overcaptures`, and the other `rust-2024-compatibility` lints | They help a migration to edition 2024, and the crates start on edition 2024. | - |
| rustdoc `missing_doc_code_examples` | It works only on the nightly toolchain (tracking issue 101730). | Section 7.5 of the writing guide |

### 4.7. Lints to Verify Before Adoption

This section records the outcome of checking each lint fact that was first unverified. An agent that wants to adopt a lint that this document does not list as verified MUST first check it on the current toolchain, because a lint name that does not exist, or a lint that the stable toolchain does not support, checks nothing, and the document then promises a check that never runs.

Each entry that this section first recorded was run on the toolchain of Section 1.1. Each outcome is recorded where it applies:

- `clippy::or_fun_call` is in `nursery`, so it is off (Section 4.6).
- rustc `non_exhaustive_omitted_patterns` and `unqualified_local_imports` are unstable, so they cannot be used on the stable toolchain (Section 4.6).
- rustc `ambiguous_negative_literals` and `redundant_imports` exist, are stable, and are allow by default. The configuration enables both (Sections 4.2 and 4.5).
- `clippy::arbitrary_source_item_ordering`, `clippy::tests_outside_test_module`, and `clippy::module_name_repetitions` are in `restriction` and are not adopted (Section 4.6).
- `clippy::unwrap_in_result`, `clippy::panic_in_result_fn`, and `clippy::wildcard_enum_match_arm` are in `restriction` and stay off (Section 4.6).
- `clippy::from_over_into` is in `style`, so `clippy::all` enables it (Section 4.5).
- `clippy::cargo_common_metadata` behaves as its row in Section 4.5 states.
- `CARGO_BUILD_WARNINGS=deny` fails `cargo doc` and `cargo test --doc` on warnings, with the limits that Section 4.8 states.
- Clippy reads `clippy.toml` and rejects unknown keys (Section 4.3).
- `clippy::string_to_string`, `clippy::transmute_int_to_char`, `clippy::transmute_int_to_float`, and `clippy::unchecked_duration_subtraction` do not exist. `clippy::unchecked_time_subtraction` exists in `pedantic`, which the configuration enables (Section 18).
- rustdoc `missing_doc_code_examples` works only on the nightly toolchain (Section 4.6).
- rustc `temporary_cstring_as_ptr` no longer exists, because `rustc -W help` does not list it. Its replacement, `dangling_pointers_from_temporaries`, is on by default at `warn` (Section 4.5). Do not use the old name.

### 4.8. Other Tools

Each deterministic check that is not a lint belongs to one tool. The development container provides every tool in this table, and the table marks each one as available. No recipe runs any of them yet, and none of their configuration exists in the repository (Section 4.1). Milestone M0 adds the recipes and the configuration. Until the check runs a tool, each rule it would enforce stays a judgement rule, as Section 3.5 of the writing guide states.

| Tool | What it owns | Status |
| --- | --- | --- |
| `cargo fmt` | Formatting, with the default rustfmt settings | Available |
| `cargo clippy` | Every lint of Section 4.2 | Available |
| `cargo nextest run` | Unit and integration tests, including the binding tests of TEST-8. It does not run doctests. | Available |
| `cargo test --doc` | Doctests (Section 7.5 of the writing guide) | Available |
| `cargo doc` | The rustdoc lints (Section 7.12 of the writing guide) | Available |
| `cargo deny` | Licences compatible with GPL-3.0-or-later, security advisories, banned and duplicate crates, and the allowed sources of crates, configured in `deny.toml` as this section requires | Available |
| `cargo insta` | Reviewing changes to snapshot tests (TEST-2) | Available |
| `cargo semver-checks` | Breaking changes to the public API against the last release | Available |
| `cargo hack` | That every combination of Cargo features compiles (DEP-1), with `--feature-powerset`, and that the workspace builds with the toolchain of `rust-version` (DEP-4) | Available |
| `cargo machete` | Unused dependencies | Available |
| `cargo llvm-cov` | Test coverage | Available |
| `typos` | Spelling mistakes in identifiers and text | Available |
| `taplo` | Formatting and validity of Tom's Obvious Minimal Language (TOML) files | Available |
| `actionlint` | GitHub Actions workflow files | Available |
| Miri | Undefined behaviour in unsafe code under test (TEST-7), run through nextest on the nightly toolchain | Available |

These commands run the checks of the tools from `cargo fmt` to `cargo deny`:

```sh
cargo fmt --all --check
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features
CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --no-tests=warn
CARGO_BUILD_WARNINGS=deny cargo test --doc --workspace --all-features
CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features
cargo deny check
```

These commands run the checks of the tools from `cargo hack` to Miri. The first two check the features, and the third checks the MSRV:

```sh
CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --all-targets --each-feature
CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --all-targets --feature-powerset
CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --all-targets --rust-version
cargo machete
cargo llvm-cov nextest --workspace --all-features --no-tests=warn
cargo +nightly miri nextest run --workspace --no-tests=warn
typos
taplo fmt --check
actionlint
```

The command for `cargo semver-checks` is chosen with the first release, because the tool fails on a crate that has no published version. The `coverage` recipe adds `--profile gpu --lcov --output-path target/lcov.info` to the `cargo llvm-cov` command, so that it writes an LCOV report and prints a summary. That command fails with "no coverage data found" while no crate contains a function, so no CI job runs it before a crate has code. Every command that runs nextest passes `--no-tests=warn`, because nextest otherwise fails a run that selects no test, and only a command-line option or an environment variable changes that. Miri runs through nextest, which runs each test in its own process, because `cargo miri test` aborts the whole test binary at the first foreign call that Miri does not support.

`CARGO_BUILD_WARNINGS=deny` is stable and needs no `-Z` flag. It fails `cargo check`, `cargo hack check`, `cargo clippy`, and `cargo nextest run` on any warning. It fails `cargo doc` on any rustdoc lint warning, so the documentation command needs no `RUSTDOCFLAGS` setting. It fails `cargo test --doc` on warnings in the library build, but not on warnings in the code inside a doctest, which no tool reports (Section 7.5 of the writing guide).

The doctest command runs on the whole workspace, which skips packages without a library target. The form `cargo test --doc -p <package>` fails with "no library targets found" on a package that has only binary targets.

An agent that creates `deny.toml` MUST include `GPL-3.0-or-later` itself in the licence allow-list, because the workspace's own crates carry that licence, and MUST give every path dependency between workspace members a version, because `cargo deny check` fails on the workspace otherwise, and Cargo refuses to package or publish a crate whose path dependency has no version. The setting `bans.allow-wildcard-paths = true` is no alternative, because it exempts only crates with `publish = false`.

This section names no `just` recipes. `docs/design/tooling.md` specifies the recipes and the CI workflows, and decides how these commands are grouped.

## 5. API Design

**API-1.** Name a conversion method by its cost and ownership: `as_` for a free view from borrowed to borrowed, `to_` for an expensive conversion or a by-value conversion of a `Copy` type, and `into_` for a conversion that consumes `self`. Name a getter after its field, without a `get_` prefix, and use `get` alone only when the type has one obvious thing to return. Reason: the name then tells the caller the cost and the ownership without the documentation, and `clippy::wrong_self_convention` checks only the receiver, not the cost. [IR-8] [IR-17]

**API-2.** Give a collection-like type the methods `iter`, `iter_mut`, and `into_iter`, and name each iterator type after the method that returns it, such as `entry_points()` returning `EntryPoints`. Return a named iterator type from a public function, and implement `FusedIterator`, `DoubleEndedIterator`, and `ExactSizeIterator` on it whenever they hold. Return `impl Iterator` only from crate-private functions, and return a `Vec` when the function computes every element anyway. Reason: a named type is a stable part of the API that can gain trait implementations later, while an `impl Iterator` return cannot gain `Clone` or `Debug` without a breaking change. [IR-8] [IR-17] [IR-83] [IR-93]

**API-3.** Keep one name for each concept across all crates, keep one word order in compound names as the standard library does, such as `ParseTargetError` after `ParseBoolError` and not `TargetParseError`, and give a Slang concept the name it has in the Slang API, converted to Rust case. Reason: two names read as two concepts, and a reader who knows Slang finds items by their Slang names. [IR-8] [IR-2]

**API-4.** Implement `From` only for a conversion that is lossless, infallible, and never panics, and `TryFrom` for every other conversion. Do not implement an infallible `From` from a raw type into a type that carries an invariant. Put a conversion on the more specific of the two types. Reason: a `From` that panics or substitutes a fallback hides a failure from the caller, and the lint that would catch a panicking `From` is in the disabled nursery group (Section 4.6). [IR-8] [IR-53] [IR-90] [S-10]

**API-5.** Write constructors as static inherent methods: `new` for the primary constructor, names such as `with_capacity` for secondary ones, `from_` for a conversion that is `unsafe` or takes extra arguments, and a domain verb such as `open` where one is idiomatic. Implement `Default` only when a default value is valid and meaningful. Reason: users look for constructors under these names, and a `Default` that produces an invalid value bypasses the validating constructor. [IR-8] [IR-5] [IR-82] [IR-92]

**API-6.** Use a builder for a type with four or more construction options or with optional configuration: `Foo::builder()` returns `FooBuilder`, its setters are infallible, and its `build()` validates and returns a `Result`. Make setters take `&mut self` and return `&mut Self`, and make the builder consuming, with setters that take and return `self`, when `build` must take ownership of what the builder holds, such as an owning interface pointer. For a small set of independent options, use a `#[non_exhaustive]` options struct with public fields and a `Default` implementation instead. Reason: Rust has no keyword arguments, a builder can gain options without a breaking change, and a validating `build` reports every error in one place. Section 17 records the choice between consuming and non-consuming builders. [IR-8] [IR-2] [IR-44] [IR-63] [S-10]

**API-7.** Implement the common traits on every public type where they apply. Implement `Debug` with output that is never empty, and show a handle's identity, not its internals, through `finish_non_exhaustive`. Implement `Clone` where a copy is meaningful, `Copy` only for plain values and never for a type that owns a resource, `PartialEq`, `Eq`, `Hash`, and `Ord` where the type has a natural equality or order, and `Display` for a type that a user reads. Reason: the orphan rule stops users from adding these implementations later, and a `Copy` resource handle lets two values release one resource. [IR-8] [IR-82] [S-10]

**API-8.** Seal every public trait that users are not meant to implement, with a private supertrait in a private module, and say in the trait's documentation that it is sealed. Reason: a sealed trait can gain methods without a breaking change, and an unsealed one cannot. [IR-8] [IR-10]

**API-9.** When `clippy::exhaustive_enums` or `clippy::exhaustive_structs` asks for a decision, add `#[non_exhaustive]` to every public enum or struct that mirrors a Slang type, describes errors, or holds options. Add a lint exception instead only to a type whose set of values its meaning closes, such as a choice between two directions, or to a struct that other crates must construct with a struct literal, such as a hand-written `#[repr(C)]` vtable for an interface that Rust code implements. Reason: Slang adds enum values and options between releases, and `#[non_exhaustive]` lets the crates follow without a breaking release, at the cost of a wildcard arm in users' matches, while on a struct it stops every other crate from writing a struct literal of it. [IR-5] [IR-3] [IR-90]

**API-10.** Keep struct fields private unless the struct is a passive record of data, keep each public item reachable through exactly one path, and do not write glob re-exports or preludes. Share code between modules with `pub(crate)`, not `pub`. Reason: every public field and path is a promise that holds until the next breaking release, and `#[doc(hidden)]` hides an item from the documentation without withdrawing that promise. [IR-8] [IR-36] [S-10]

**API-11.** Decide for each public trait whether it serves as a trait object or only as a generic bound, and record the decision in the trait's documentation. For a trait meant for `dyn`, keep it object-safe, exclude generic methods with `where Self: Sized`, and add a test that compiles a `&dyn Trait`. Otherwise, do not constrain its design for object safety. Reason: object safety is hard to add later without a breaking change, and a trait that never needs `dyn` loses useful generic methods if it is designed for it. [IR-8] [IR-81] [IR-7]

**API-12.** Express an abstraction as a generic bound in a function signature, not as a stored `Box<dyn Trait>`, and keep trait objects for small behaviours such as callbacks. Do not put derivable trait bounds such as `Clone` or `Debug` on a struct definition. Ask for the least restrictive closure bound, in the order `FnOnce`, `FnMut`, `Fn`. Reason: a generic bound keeps static dispatch, a bound on a struct definition spreads to every user and adding one later is a breaking change, and a stricter closure bound rejects closures that would work. [IR-8] [IR-81] [IR-3]

**API-13.** Use generic parameter types only at the public edge: `impl AsRef<Path>` for a path, `&str` for text that the function only reads, and `impl Into<String>` for text that the function stores. Make the generic function a thin wrapper that converts its arguments and calls a private non-generic function, and take `&Path`, `&str`, or `String` inside the crate. Reason: the generic form spares callers a conversion, and the non-generic inner function is compiled once instead of once for each argument type. Section 17 records the resolution between the sources. [IR-48] [IR-69] [IR-40] [IR-63] [IR-7]

This illustration shows a thin public wrapper over a concrete inner function:

```rust
use std::io;
use std::path::Path;

pub fn read_source(path: impl AsRef<Path>) -> io::Result<String> {
    read_source_inner(path.as_ref())
}

fn read_source_inner(path: &Path) -> io::Result<String> {
    std::fs::read_to_string(path)
}
```

**API-14.** Return results as values, and return a tuple or a struct for several results, instead of filling `&mut` out-parameters. Convert every out-parameter of the Slang API into a return value in the safe crate. Take a caller's `&mut` buffer only when the caller reuses it across calls, and then also provide a function that returns a new value. Reason: a returned value states its ownership in the signature and costs no more in most cases, and the buffer form exists only for measured reuse. [IR-8] [IR-38]

**API-15.** Implement `Deref` and `DerefMut` only on smart pointers. Never implement them to reuse the methods of an inner type, and never on a type that carries an invariant over its inner value. Reason: `Deref` changes method resolution implicitly, gives no subtyping, and exposes every method of the target, including the ones that break the wrapper's invariant. [IR-8] [IR-5] [IR-28]

**API-16.** Keep `Drop` infallible and non-blocking: release resources on a best-effort basis and ignore errors there. Provide a consuming `close(self)` that returns a `Result` for teardown whose errors matter, and an explicit method for a wait, such as a wait for the GPU to finish. Put cleanup that must `.await` in an explicit async method, because `Drop` cannot be async. Reason: a panic in `Drop` during unwinding aborts the process, and a destructor that blocks stalls the caller at an unpredictable point. [IR-8] [IR-13] [IR-88]

## 6. Type-Driven Design

**TYPE-1.** Validate a value once, in the one constructor of a newtype with a private field, which returns a `Result`, and implement `TryFrom` and `FromStr` by calling that constructor. Normalize the value in the constructor, so that derived `Eq` and `Hash` see one representation, and keep every mutating method invariant-preserving. Offer construction without validation only when an invalid value can cause undefined behaviour and the check is too costly for a measured hot path, and then only as an `unsafe fn` whose name ends in `_unchecked`. Reason: a value that is valid by construction cannot reach code that forgot to check it. [IR-82] [IR-28] [IR-10] [S-10]

**TYPE-2.** Replace a `bool` parameter or field that carries meaning with a two-variant enum, even when it is the only one, and replace a string that selects among fixed options with an enum that implements `FromStr`. Reason: a call such as `compile(source, true)` hides its meaning, an enum can gain a third state, and the pedantic lints flag only functions and structs with several `bool` values. [IR-41] [IR-68] [IR-48] [IR-90]

**TYPE-3.** Model fields that depend on each other as one enum, so that an invalid combination cannot be written. Use bounded types such as `NonZeroU32` for values with a restricted range, and do not store a value that can be derived from other fields. Reason: an invalid combination that compiles is eventually built, and a stored derived value diverges from its source after a later change. [IR-92] [IR-29] [IR-28] [IR-93]

**TYPE-4.** Use typestate, with a state type parameter and transitions that consume `self`, for a lifecycle in which some operations are valid only in some states and misuse is a programming error, such as a command encoder that becomes a finished command buffer. Return a `Result` from a fallible transition, offer constructors only for the start state, mark state types `#[must_use]`, and do not derive `Clone` or `Copy` on them. Use a run-time state check instead when the state depends on data known only at run time, or when typestate would add type parameters to more than a few public items. Reason: typestate turns a wrong call order into a compile error, but its type parameters spread through signatures, so it is worthwhile only where misuse is likely and the states are few. [IR-50] [IR-10] [IR-23] [IR-42]

**TYPE-5.** Convert integers with `From` when the value always fits and with `TryFrom` when it may not, and handle the `TryFromIntError`. Use `as` only for an intended truncation or a pointer cast, under a lint exception on the pedantic cast lint that fires. Convert between `usize` and a fixed-width integer with `try_from`, never with `as` under a lint exception. Write intended wrapping with `wrapping_*` methods, and use `checked_*` arithmetic for every size, count, or offset that comes from Slang, from a shader, or from a user. Reason: `as` truncates and wraps silently, and integer overflow wraps silently in release builds, which turns a malformed size into an out-of-bounds access in unsafe code. A lint exception on a `usize` cast must hold on every checked target (Section 4.4). `clippy::cast_possible_truncation` fires on `usize as u32` on 64-bit targets, which was verified on x86_64, and it is not yet verified whether it fires on 32-bit targets. [IR-74] [IR-92] [IR-3] [IR-22]

**TYPE-6.** Convert an integer to an enum with an explicit `match` inside a `TryFrom` implementation that returns an error for an unknown value, and convert an enum to an integer with an explicit `match`. Never use `transmute` or `as` in either direction. Reason: an integer with no matching variant is undefined behaviour as a Rust enum, and `as` silently changes the integer value when the order of the variants changes. [IR-53] [S-4]

**TYPE-7.** Match every enum that the same crate defines without a `_` arm, grouping variants with `|`. In a hand-written `PartialEq`, `Hash`, `Debug`, or `Clone`, destructure `self` fully and name each ignored field as `field: _`. Reason: a new variant or field then fails to compile at every site that must handle it. Section 17 records why `clippy::unneeded_field_pattern` is off. [IR-90] [IR-41] [IR-2]

**TYPE-8.** Give quantities with different units different newtypes, such as a byte size and an element count, store each as an integer in one base unit, and convert only at the boundary. Use an enum for a choice of exactly one value, and the `bitflags` crate for flags that combine. Reason: a byte count passed where an element count is expected reads past the end of a buffer, and a newtype makes that mistake a compile error at no run-time cost. [IR-45] [IR-82] [IR-8] [IR-10]

**TYPE-9.** Read from a slice with `get`, slice patterns, or iterators when the index or the length comes from Slang, from a shader, or from a user, and index directly only where a check in the same function proves the bound. Reason: an index from outside the crate can be out of range, and the resulting panic violates ERR-5. `clippy::indexing_slicing` is off because it also flags the many safe indexing sites in numeric code (Section 4.6). [IR-90] [IR-92] [IR-87]

## 7. Errors and Panics

**ERR-1.** Give each library crate one public error type, designed like `std::io::Error`: an opaque struct named `Error` with private fields, a method `kind()` that returns a `Copy`, `#[non_exhaustive]` enum named `ErrorKind`, and the alias `pub type Result<T> = std::result::Result<T, Error>`. When the error comes from Slang, the struct carries the result code and the diagnostic text, with an accessor for each. It also carries the operation that failed and, when another error caused it, that error as its `source()`. The type implements `std::error::Error`, `Debug`, and `Display`, and it is `Send`, `Sync`, and `'static`. Reason: the struct can gain fields and the kind can gain variants without a breaking change, callers match on the kind and not on text, and Slang reports one result code from many calls, so the struct, not a variant per cause, carries the context of the call. [IR-86] [IR-56] [IR-8] [S-10]

This illustration shows the shape of ERR-1. The kinds and the field types are examples of the form:

```rust
use std::error::Error as StdError;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    Compile,
    InvalidArgument,
    NotFound,
    Unrecognized,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    operation: &'static str,
    code: i32,
    diagnostics: Option<String>,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn code(&self) -> i32 {
        self.code
    }

    #[must_use]
    pub fn diagnostics(&self) -> Option<&str> {
        self.diagnostics.as_deref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot {}", self.operation)?;
        if let Some(text) = &self.diagnostics {
            write!(f, ":\n{text}")?;
        }
        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|error| error as &(dyn StdError + 'static))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
```

**ERR-2.** Do not return `anyhow::Error`, `Box<dyn Error>`, `String`, or `()` as the error of a public function in a library crate, and do not expose a type from a dependency or from a `-sys` crate in the public error. Reason: a type-erased error takes away the caller's ability to inspect it, and a foreign type in the error makes every upgrade of that dependency a breaking change. [IR-56] [IR-2] [IR-32] [IR-86] [S-10]

**ERR-3.** Attach the failed operation at each fallible call site, with `map_err` or a constructor of the crate's `Error`, and implement `From<Other> for Error` only when the conversion loses nothing that the call site knows. Reason: Slang returns the same result code from many functions, and a blanket `From` lets `?` discard which call failed. Section 17 records the resolution between the sources. [IR-34] [IR-56] [IR-2]

**ERR-4.** Write the `Display` text of an error in lowercase, as a short phrase without trailing punctuation that states what failed, such as ``cannot compile module `shapes` ``. Do not repeat the text of the `source()` error in it, and append the Slang diagnostic text, when there is one, after the phrase. Reason: error reporters print the source chain themselves, so repeated text appears twice, while the diagnostic text is the error's own content and the first thing a user needs. [IR-8] [IR-34] [IR-2]

**ERR-5.** Return an error for every failure that a caller, an input, a shader, the environment, or Slang can cause, and panic only for a bug in the crate itself, with the invariant checked next to the code that relies on it. Give every intentional panic a message that begins with `internal error:`, and do not rely on a panic unwinding, because an application can build with `panic = "abort"`. Discard a `#[must_use]` value only as `let _ = expr;` under a lint exception for `clippy::let_underscore_must_use` whose reason says why the value does not matter, with a second lint exception for `let_underscore_drop` when the value has a destructor, because both lints fire on it. Never discard such a value as `_ = expr;`, `let _name = expr;`, or `drop(expr)`. Reason: a library that panics on input forces its failure policy onto every application, and a panic is not a recoverable error channel. No lint detects `_ = expr;` or `let _name = expr;`, and `drop(expr)` is detected only by `dropping_copy_types` when the type is `Copy`, so a discard in those forms hides a failure from every tool, while the `let _ =` form makes each discard carry a reason that a reviewer can check. [IR-87] [IR-2] [IR-56] [S-10]

**ERR-6.** Classify every `unwrap`, `expect`, `panic!`, and `unreachable!` outside tests as one of three kinds, and write each kind in its accepted form. The first kind, a deliberate stop of the process, does not appear in a library crate, and an application uses it only at start-up. The second kind, a site that cannot fail, is written as a type that proves it where one exists, such as `NonZeroU32::MIN`, and otherwise as `expect` with a message that begins with `internal error:` and states the invariant (ERR-5), under a lint exception whose reason states the invariant. A public function that contains such an `expect` also triggers `clippy::missing_panics_doc`, so give it a `# Panics` section, which is preferred, or a second lint exception for that lint. The third kind, error handling that is still missing, is never committed. Reason: otherwise only the author knows which kind an `unwrap` is, and the restriction lints make every accepted site state its reason. [IR-76] [IR-2] [IR-89]

This illustration shows the accepted form of a site that cannot fail:

```rust
use std::num::NonZeroU32;

#[expect(clippy::expect_used, reason = "the literal 4 is not zero")]
fn default_sample_count() -> NonZeroU32 {
    NonZeroU32::new(4).expect("internal error: the literal 4 is not zero")
}
```

## 8. Ownership and Signatures

**OWN-1.** Take a parameter by reference, as `&str`, `&[T]`, `&Path`, or `&T`, when the function only reads it, and by value when the function stores or consumes it. Do not take a reference and clone it inside to store it. Reason: the caller then decides where copies happen, and a hidden clone puts an allocation behind a signature that promises only a read. [IR-8] [IR-40] [IR-79] [IR-57]

**OWN-2.** Return an owned value when the function builds new data, a borrow when the result is part of `self` or of an argument, and `Cow<'_, str>` when the function changes its input only on some paths. Reason: a borrow avoids a copy, an owned value avoids tying the result to a temporary, and `Cow` allocates only on the path that changes the data. [IR-79] [IR-58]

**OWN-3.** Rely on lifetime elision, and do not add a lifetime parameter to a public type unless the type is a short-lived view, such as a borrowing iterator or a mapped range of a buffer. When a signature has two or more borrow sources, name each lifetime after its owner, such as `'session` and `'module`. Reason: a lifetime parameter on a type spreads to every type and function that uses it, and a descriptive name shows which value a borrow must not outlive. Section 17 records why `clippy::single_char_lifetime_names` is off. [IR-80] [IR-30] [IR-2]

**OWN-4.** Model every rule of the native API of the form "X must not be destroyed while Y lives" in the types. Tie a short-lived child to its parent with a lifetime, such as a `PhantomData<&'a Parent>` field, and make a long-lived child, which users store, hold a counted reference to its parent, as vulkano-style handles do. Never track whether the parent is alive with a run-time flag. Reason: a child that outlives its parent is a use after free that the borrow checker cannot see through a raw pointer, and a lifetime costs nothing at run time, while a counted reference costs one count change per child. [IR-46] [IR-5] [IR-10] [S-1]

**OWN-5.** Use `Arc` for a heavyweight object that many others keep alive, such as a device or a session, and for state that threads share, but not for per-frame data. Prefer `Arc<str>` and `Arc<[T]>` to `Arc<String>` and `Arc<Vec<T>>`. Represent a tree or a graph with an arena and copyable index identifiers, guarded against stale identifiers where nodes are removed, and not with `Rc<RefCell<...>>` links. Reason: shared ownership removes lifetime parameters where the sharing is real, but its counting costs on hot paths, and `Rc<RefCell>` graphs move borrow checks to run time and leak through cycles. [IR-80] [IR-54] [S-10]

**OWN-6.** Do not call `.clone()` to satisfy the borrow checker. Restructure the code, split a struct into parts that can be borrowed separately, or move a value out with `mem::take`, `mem::replace`, or `Option::take`. Reason: the clone costs an allocation and the copies diverge silently, and the conflict usually shows that the struct groups unrelated data. [IR-5] [IR-60]

**OWN-7.** Prefer iterator adapters to index loops, collect an iterator of `Result` into `Result<Vec<_>, _>`, and accept `impl IntoIterator` for an input sequence that the function only iterates. Write a `for` loop instead when the body needs an early return or the chain becomes hard to read, and split a long chain into named functions. Reason: adapters avoid off-by-one errors, but readability is the goal, and a rewrite from one form to the other for speed needs a measurement first. [IR-23] [IR-43] [IR-83]

## 9. Unsafe Code

**UNS-1.** Write `unsafe` code only to call foreign code, to build a new abstraction that safe code cannot express, or for a performance gain that a benchmark shows, and use a safe standard-library alternative everywhere else. Reason: unsafe code moves the proof of memory safety from the compiler to a reviewer for as long as the code exists. [IR-2] [IR-10] [S-10]

**UNS-2.** Put each unsafe abstraction in the smallest module that can uphold its invariant, keep every field and helper that the invariant depends on private to that module, and gather each invariant into one private helper that holds the unsafe operation. Build the rest of the API in safe code on top of it. Reason: safe code in the same module can break an invariant that an unsafe block relies on, so the module is the boundary that a reviewer audits. [IR-5] [S-1] [IR-27] [IR-61]

**UNS-3.** Make a safe function sound for every possible input. When a wrapper cannot guarantee a requirement, make it an `unsafe fn` with a `# Safety` section, and mark a function `unsafe` only when misuse can cause undefined behaviour, not because it is dangerous in another way. Reason: a library is sound only if no safe code can reach undefined behaviour through its public API, and `unsafe` used as a general warning weakens the contract. [S-1] [S-6] [S-10] [IR-3]

**UNS-4.** Never let soundness depend on a destructor running. A value whose `Drop` never runs, because of `mem::forget`, a reference cycle, or `Box::leak`, may leak, but it never leaves native state pointing at freed memory. Use a closure-scoped API, such as `with_target(|target| ...)`, for a temporary change to native state. Reason: leaking is safe in Rust, so a guard that undoes a native state change in `Drop` turns a leak into a use after free. [IR-46] [S-1] [IR-27]

**UNS-5.** Do not let soundness depend on the behaviour of a generic parameter's `Deref`, `PartialEq`, `Eq`, `Hash`, `Ord`, `Index`, or `Drop` implementation, or on a callback not panicking. When unsafe code needs such cooperation, require it through an `unsafe trait`. Reason: safe code can implement these traits incorrectly, and a safe implementation can never be allowed to cause undefined behaviour. [IR-3]

**UNS-6.** Write every `# Safety` section and safety comment in the form that Section 7.6 of the writing guide defines. On an `unsafe impl Send` or `unsafe impl Sync`, the safety comment states why the native object tolerates use from other threads and cites the documentation that says so. Use an `unsafe trait` when implementers promise something, an `unsafe fn` in a trait when callers must meet a precondition, and both when both apply. Reason: the safety documentation is the only place where the proof obligation of unsafe code is written down, and a vague condition cannot be checked. [S-6] [IR-10] [IR-27]

**UNS-7.** Convert a raw pointer to a reference only after a null check, and only when the target is aligned, initialized, and valid for the whole lifetime of the reference. Do not hold a `&mut T` to memory that native code or another handle can access while the reference lives, and use `MaybeUninit` for an out-parameter that native code fills. Do not create a pointer from an integer, and do not compare addresses from different allocations. Reason: a reference must be valid when it is created, not only when it is used, and violations of aliasing and pointer provenance are undefined behaviour even when the code appears to work. [S-5] [S-6] [S-1]

## 10. Foreign Function Interface

**FFI-1.** Keep `slang-compiler-sys` to generated declarations, constants, vtable structs, UUID constants, and thunks, plus the build script that links the native library, with no higher-level abstraction. Give the safe crate its own Rust types, and do not re-export `-sys` types from it except through the raw-pointer methods of COM-11. Reason: the `-sys` crate then changes only when Slang changes, and every decision about ownership, lifetimes, and errors lives in the one crate that users depend on. [S-3] [S-4] [S-10] [IR-7] [IR-13]

**FFI-2.** Declare `links = "slang-compiler"` in the manifest of the `-sys` crate, emit every link directive from its build script, and do not write `#[link]` attributes in Rust source. Reason: Cargo allows one package for each `links` value, which prevents two copies of the native library in one build, and the build script alone then decides between static and dynamic linking and sets the search paths. [S-3]

**FFI-3.** Declare foreign functions in `unsafe extern "C"` blocks with the ABI string written out, mark an item `safe fn` only if every possible argument is sound, use `#[repr(C)]` on every type that crosses the boundary, and use the C types of `core::ffi`, such as `c_char` and `c_int`. Use `Option<NonNull<T>>` and `Option<unsafe extern "C" fn(...)>` for pointers and function pointers that may be null. The binding generator emits these forms, so this rule binds the generator. Reason: the compiler trusts these declarations without checking them against the C++ headers, so a wrong one is undefined behaviour at run time, and only the `Option` forms carry the null case in the type. [S-1] [S-5] [S-7] [IR-10] [IR-13]

**FFI-4.** Read a string from Slang as `&CStr`, and at once tie its lifetime to the object that owns the memory or copy it into an owned `String`. Handle invalid UTF-8 as an error or as a documented lossy conversion. Pass a string to Slang as a `CString` bound to a variable that lives until the call returns, report an interior NUL byte as an error, and use `c"..."` literals for constant strings. Reason: `CStr::from_ptr` returns a reference with an unbounded lifetime, so the compiler cannot prevent a use after Slang frees the string, and `dangling_pointers_from_temporaries` catches only a pointer taken from a temporary. [IR-5] [S-6] [S-1]

**FFI-5.** Build a slice from a pointer and a length that Slang returns only after a null check: treat a null pointer with a zero length as an empty slice and a null pointer with any other length as an error. Interpret the length as a count of elements, not bytes, unless the Slang documentation says otherwise, and tie the slice's lifetime to the object that owns the memory. Reason: a slice is a reference, so a null or dangling pointer is undefined behaviour as soon as the slice exists, and a byte count used as an element count reads past the end of the buffer. [S-5] [IR-10] [IR-35]

**FFI-6.** Check every pointer that Slang returns for null before using it, with `NonNull::new`, and use `NonNull::new_unchecked` only where the Slang documentation guarantees a non-null value. Turn a null that signals absence or failure into `None` or an error. Reason: dereferencing a null pointer is undefined behaviour, not a crash that a test reliably shows. [IR-13] [S-6] [S-1]

**FFI-7.** Never let a panic leave Rust through a foreign call or a callback. Declare every callback that Slang calls as an `extern "C"` function, and use `"C-unwind"` only when the Slang side is built to unwind. In a callback that reports failure through a result code, catch a panic with `catch_unwind`, return a failure code, and make the next call into the owning object fail. Otherwise, let the abort of a plain `extern "C"` function stop the process. State in the crate documentation that `catch_unwind` does nothing when the application builds with `panic = "abort"`. Reason: since Rust 1.81 a panic that reaches an `extern "C"` boundary aborts the process, a foreign exception that enters Rust is undefined behaviour, and continuing after a caught panic can leave state inconsistent. [S-1] [S-5] [S-9] [S-10] [IR-13] [IR-87]

This illustration shows a callback that turns a panic into a failure code. It does not show how the owning object records the failure:

```rust
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

const RESULT_OK: i32 = 0;
const RESULT_FAIL: i32 = -1;

unsafe extern "C" fn on_message(user_data: *mut c_void, len: usize) -> i32 {
    // SAFETY: the registering code passes a pointer to a live `Vec<usize>` and
    // unregisters the callback before the vector is dropped.
    let lengths = unsafe { &mut *user_data.cast::<Vec<usize>>() };
    match catch_unwind(AssertUnwindSafe(|| lengths.push(len))) {
        Ok(()) => RESULT_OK,
        Err(_) => RESULT_FAIL,
    }
}
```

**FFI-8.** Pass the context of a callback as a user-data pointer, keep the context alive until the callback is unregistered, unregister it in the `Drop` of the value that registered it, and synchronize access when Slang can call it from another thread. Reason: a callback that fires after its context is freed dereferences freed memory, and Miri cannot see calls that come from native code. [S-1]

**FFI-9.** Make a type that wraps a foreign handle neither `Send` nor `Sync` by default, which its raw pointer or `NonNull` field already ensures. Add `unsafe impl Send` or `unsafe impl Sync` to one specific type only after the Slang documentation establishes that the object tolerates that use, and never write a generic wrapper that makes any type `Send`. Reason: the compiler cannot check an `unsafe impl Send`, a native object that is not thread-safe corrupts its state when two threads call it, and the nursery lint that flags `Send` over fields that are not `Send` is off. [S-1] [IR-27] [S-10] [IR-13]

**FFI-10.** Represent an open-ended set of C values, such as result codes and flags, as a `#[repr(transparent)]` integer newtype with associated constants in the `-sys` crate, never as a Rust `enum`, and convert it in the safe crate with an explicit `match` that handles unknown values. Reason: Slang can return a value that the bindings do not declare, and such a value in a Rust `enum` is undefined behaviour. [S-4] [S-2]

**FFI-11.** Free memory with the library that allocated it. Never build a `Vec`, `String`, or `Box` over memory that Slang allocated, and never pass memory that Rust allocated to Slang for Slang to free. Reason: two allocators that free each other's memory cause double frees and heap corruption. [IR-10] [IR-13] [IR-5]

## 11. Build Scripts

**SYS-1.** Emit `cargo::rerun-if-changed` for every file that the build script reads and `cargo::rerun-if-env-changed` for every environment variable that it reads, using the `cargo::` form of the instructions, and declare every custom `cfg` with `cargo::rustc-check-cfg` next to the instruction that sets it. Reason: a script that emits no rerun instruction reruns whenever any file of the package changes, a missing instruction leaves a stale build after its input changes, and an undeclared `cfg` triggers `unexpected_cfgs`. [S-3]

**SYS-2.** Read environment variables in the build script when it runs, with `std::env::var` or `std::env::var_os`, and never with `env!` or `option_env!`. Reason: `env!` and `option_env!` read the value when the build script itself is compiled, so a later change of the variable has no effect until the script is rebuilt.

**SYS-3.** Read the target platform from the `CARGO_CFG_TARGET_*` environment variables, splitting the multi-valued ones at commas, and never from `cfg!` or `#[cfg]` in the build script. Reason: the build script runs on the host, which differs from the target when cross-compiling. [S-3]

**SYS-4.** Check the exit status of every process that the build script starts, and on failure end the build script by returning, from a `main` that returns `Result`, an error that names the command, its exit status, and its error output. Reason: an ignored failure of a native build surfaces later as an obscure link error or a stale library, and the lints `clippy::panic` and `clippy::expect_used` reject the other ways to stop a build script, apart from the form of ERR-6.

**SYS-5.** Write only inside `OUT_DIR`, do not assume that it is empty, and never write to the source tree, download files, or install packages. Reason: registry sources are read-only, builds have to work offline and in sandboxes, and a build script that changes the user's system is a security risk. [S-3] [S-4]

**SYS-6.** Check that the Slang the build finds matches the pinned version, which is a Vulkan software development kit (SDK) version together with a Slang commit, and stop the build with a message that names both versions when it does not. The design document defines how the build script reads the version. Reason: the checked-in bindings describe one version of the headers, and a different library behind them gives mismatched layouts and undefined behaviour at run time.

**SYS-7.** Obtain Slang in exactly one of two ways. By default, link dynamically to the Slang of an installed Vulkan SDK. As an opt-in through a Cargo feature, build Slang from source and link it statically. Do not add a prebuilt download or any other way to find the library. Reason: two documented paths can be tested on every CI platform, and a download in a build script runs unreviewed code and breaks offline builds. [S-4] [S-10]

## 12. COM-Style Interfaces

No published source covers reference-counted COM-style interfaces from Rust. The rules in this section are therefore project-defined and derive from no external source. COM-5 cites a defect report only as evidence of the failure that the rule prevents. Where a rule applies a rule from another section, it names that rule.

**COM-1.** Hold every interface in one owning interface pointer type, generic over the interface, that owns exactly one reference: its `Clone` calls `AddRef` and returns a new owner, and its `Drop` calls `Release` once. No other code calls `AddRef` or `Release`. Reason: one type that pairs every `AddRef` with one `Release` makes a leaked or double-released reference impossible in safe code, and a reviewer audits the pairing in one place.

This illustration shows the core of an owning interface pointer over a vtable with only the two counting functions:

```rust
use std::ptr::NonNull;

#[derive(Debug)]
#[repr(C)]
struct Vtable {
    add_ref: unsafe extern "C" fn(this: *mut Object) -> u32,
    release: unsafe extern "C" fn(this: *mut Object) -> u32,
}

#[derive(Debug)]
#[repr(C)]
struct Object {
    vtable: *const Vtable,
}

#[derive(Debug)]
struct Owned {
    raw: NonNull<Object>,
}

impl Owned {
    /// Adopts one reference that the caller owns.
    ///
    /// # Safety
    ///
    /// - `raw` points to a live object whose `vtable` field points to a valid vtable.
    /// - The caller owns one reference to the object and transfers it to the returned value.
    unsafe fn from_raw(raw: NonNull<Object>) -> Self {
        Self { raw }
    }

    fn vtable(&self) -> &Vtable {
        // SAFETY: `from_raw` requires a live object, and `self` owns a reference
        // that keeps it alive.
        let object = unsafe { self.raw.as_ref() };
        // SAFETY: `from_raw` requires `vtable` to point to a valid vtable, which
        // lives as long as the object.
        unsafe { &*object.vtable }
    }
}

impl Clone for Owned {
    fn clone(&self) -> Self {
        // SAFETY: the object is live while `self` exists, and the returned value
        // owns the reference that `add_ref` adds.
        unsafe { (self.vtable().add_ref)(self.raw.as_ptr()) };
        Self { raw: self.raw }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: `self` owns exactly one reference, and `drop` runs at most once,
        // so the reference is released once.
        unsafe { (self.vtable().release)(self.raw.as_ptr()) };
    }
}
```

**COM-2.** Distinguish owned from borrowed interface pointers at every boundary. Adopt a pointer that Slang returns with a reference already added without calling `AddRef`, call `AddRef` on a pointer that Slang returns without a reference before the safe crate keeps it, and take an interface that a function only uses during a call as a borrow of its owning pointer, without changing the count. The design document records, for each Slang function that the safe crate calls, which of these cases applies. Reason: one wrong assumption about who owns a reference either leaks the object or frees it while it is still in use.

**COM-3.** Wrap `QueryInterface` as a checked cast that takes the UUID of the target interface from its type and returns an owning interface pointer to the target or an error. Never convert between interface pointer types with a pointer cast. Reason: a pointer cast to an interface that the object does not implement calls the wrong vtable entries, which is undefined behaviour.

**COM-4.** Take every interface UUID from a constant that the binding generator produces from the Slang headers and attaches to the interface type through a trait, and never type a UUID by hand. Reason: a mistyped UUID compiles and then fails only at run time, or selects the wrong interface.

**COM-5.** Make every object that Slang creates from a parent hold an owning interface pointer to that parent, such as a module to its session, so that the parent lives as long as any child. Declare the child's own pointer field before the parent field, so that the child is released first. Reason: an existing third-party binding segfaults when a session is dropped while objects created from it are still alive [SLANG-RS-26], and Rust drops fields in declaration order. This rule applies OWN-4 to interfaces.

This illustration shows the field order, with `Arc` standing in for the owning interface pointer:

```rust
use std::sync::Arc;

#[derive(Debug)]
struct RawSession;

#[derive(Debug)]
struct RawModule;

#[derive(Clone, Debug)]
pub struct Session {
    raw: Arc<RawSession>,
}

#[derive(Debug)]
pub struct Module {
    raw: Arc<RawModule>,
    session: Session,
}
```

**COM-6.** Expose the contents of a blob as `&[u8]` borrowed from the owning interface pointer to the blob, so that the slice cannot outlive the blob, and copy into an owned buffer only when the caller asks. Decode a text blob with explicit handling of invalid UTF-8. Reason: a slice that outlives its blob points to freed memory, and a copy on every access wastes the allocation that a borrow avoids. This rule applies FFI-4 and FFI-5 to blobs.

**COM-7.** Map Slang result codes to the crate's `Error` in exactly one function, pass every result code that the crate receives through it, and keep the raw code in the error. Map a code that the function does not recognize to a kind that says so, never to success, and do not compare raw result codes anywhere else. Reason: one mapping keeps the classification of codes consistent, and a code compared by hand in a second place diverges from the mapping when it changes.

**COM-8.** Pass an out-parameter for the diagnostics blob to every Slang function that accepts one, and always collect the blob. On failure, put its text into the returned `Error`. On success, return any warnings it holds to the caller with the value. Never drop the blob unread or print it. Reason: the diagnostics are the only explanation that Slang gives for a failure, and warnings on success are information that the user needs.

**COM-9.** Never edit the generated vtable structs, UUID constants, or thunks by hand. Change the binding generator, regenerate, and commit both. Reason: a hand edit disappears at the next regeneration, and the test of TEST-8 that regenerates and compares the bindings fails on it.

**COM-10.** Give the methods of an interface wrapper `&self` receivers, never `&mut self`, and express thread safety through `Send` and `Sync` as FFI-9 decides. Reason: other owning interface pointers can reach the same object through `AddRef`, so `&mut self` would promise exclusive access that the object does not have.

**COM-11.** Give every interface wrapper three raw-pointer methods: `unsafe fn from_raw`, which adopts one reference, `fn into_raw(self)`, which gives up ownership without calling `Release`, and `fn as_raw(&self)`, which returns the pointer without changing the count. Implement `into_raw` with `ManuallyDrop`, not `mem::forget`. Reason: users combine these crates with other native code, and without these methods they reimplement the reference counting in unsafe code of their own.

**COM-12.** Design the safe API around Rust types and ownership, not around the interface hierarchy. Do not mirror interface inheritance as a trait hierarchy, and reach the methods of a base interface through the checked cast of COM-3 or through explicit methods. Reason: a mirrored hierarchy brings in C++ patterns that Rust expresses poorly, such as downcasts and implicit upcasts, while users of the safe crate need the operations, not the hierarchy.

**COM-13.** Give each interface wrapper one small helper method that returns a reference to its vtable, such as the `vtable` method in the illustration of COM-1, and call every vtable entry through it, so that each call is one unsafe operation in its own `unsafe` block. Reason: a direct call such as `((*(*this).vtable).method)(this)` is three unsafe operations, two dereferences and a call, which `clippy::multiple_unsafe_ops_per_block` rejects, and splitting it at every call site repeats the same safety comments in every method.

## 13. Concurrency and Async

**CONC-1.** Decide for each public type whether it is `Send` and `Sync`, state the decision in its documentation when it is not both, and add a compile-time assertion test for each auto trait that it does implement. A type that holds no foreign handle is `Send` and `Sync` unless it holds a value tied to one thread, and a type that holds a foreign handle follows FFI-9. Reason: auto traits change silently when a field changes, and a lost `Send` is a breaking change that no other test detects. [IR-8] [S-10] [IR-13]

**CONC-2.** Bind a lock guard to its own `let` with the shortest scope that covers the protected work, and never call a lock method in the scrutinee of a `match` or `while let`. Do not lock a `Mutex` again, or take the write lock of an `RwLock`, while a guard from the same lock is alive. Reason: a temporary in the scrutinee of a `match` or `while let` lives until the end of the whole statement, so a guard taken there is held in every arm. Edition 2024 shortened only the lifetime of an `if let` scrutinee, and the nursery lints that detect this case are off. [IR-27] [IR-84] [S-7]

This illustration shows the form to avoid. The second lock in the `None` arm waits for the guard that the scrutinee still holds:

```rust
use std::sync::{Mutex, PoisonError};

fn refresh(cache: &Mutex<Option<String>>, load: impl Fn() -> String) {
    match cache.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        Some(_) => {}
        None => {
            let value = load();
            *cache.lock().unwrap_or_else(PoisonError::into_inner) = Some(value);
        }
    }
}
```

This illustration shows the accepted form, with one guard in its own binding:

```rust
use std::sync::{Mutex, PoisonError};

fn refresh(cache: &Mutex<Option<String>>, load: impl Fn() -> String) {
    let mut guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.is_none() {
        *guard = Some(load());
    }
}
```

**CONC-3.** Handle a poisoned lock on purpose: recover the guard with `PoisonError::into_inner` when a panic cannot leave the protected state invalid, and otherwise return an error. Reason: an `unwrap` on the result of `lock()` turns one panic into a panic in every later caller. [IR-27] [IR-2]

**CONC-4.** Do not start threads that the caller cannot see. Accept work in batches and let the caller choose the parallelism. Keep library state out of `static` items, initialize state that must exist once with `OnceLock` or `LazyLock`, and never call `std::env::set_var` or `std::env::remove_var`. Reason: hidden threads impose the library's concurrency policy on the application, two versions of a crate in one build each get their own statics, and changing the environment while other threads run is unsound on some platforms. [IR-3] [S-10] [S-7] [IR-27]

**CONC-5.** Use `Relaxed` atomics only for counters and flags that guard no other data, use `Release` stores with `Acquire` loads to publish data through a flag or a pointer, and treat `SeqCst` as a reason to re-examine the algorithm. Do not read two atomics as one consistent snapshot. Reason: `Relaxed` creates no ordering between threads, and `SeqCst` is correct only where some weaker ordering already is. [IR-27]

**CONC-6.** Keep the core API blocking, and offer `async` functions only where the operation waits for something outside the process, such as a wgpu buffer mapping, device polling, or file input and output. Do not put `Send + 'static` bounds on library traits or futures unless the library itself spawns tasks, and use native `async fn` in traits instead of the `async-trait` crate unless dynamic dispatch requires it. Reason: compiling a shader is work for the central processing unit (CPU), the bounds come from the `spawn` function of a runtime and shut out single-threaded applications, and native async traits need no boxing. [IR-77] [IR-78] [IR-17]

**CONC-7.** Document the cancel safety of every public `async fn`: what is lost, or still in flight in wgpu or on the GPU, when the future is dropped at each `.await`. Design each one so that a drop at any `.await` leaves every handle usable, and do not hold a lock guard or temporarily invalid state across an `.await`. Reason: dropping a future is how async Rust cancels, so every `.await` is a point where the operation can stop, and the lints of Section 4.5 catch only guards of known types. [IR-88]

## 14. Testing

**TEST-1.** Put unit tests in a `#[cfg(test)] mod tests` in the file they test, integration tests in `tests/` that use only the crate's public API under its crate name, and examples in `examples/`. Name a test after the function and the condition, such as `compile_module_rejects_empty_source`, without the word "should". Reason: integration tests then exercise the API as a user sees it, and a consistent name states what a failing test checked. [IR-17] [IR-10] [IR-63]

**TEST-2.** Cover text and structured output, such as diagnostic messages and reflection data, with `insta` snapshot tests, review their changes with `cargo insta review`, commit the snapshot files, and make the output under test deterministic. Reason: a snapshot shows a reviewer the exact change in output, and output that depends on hash order or timing makes the test flaky. [IR-17] [IR-1]

**TEST-3.** Tag every test that needs the native Slang library, a GPU adapter, or a hardware GPU, so that a run can exclude it with a filter, using the mechanism that the design document defines. Run the tests that need a hardware GPU locally only. Reason: CI does not exist yet. The planned CI targets are Linux with lavapipe, Windows with the Windows Advanced Rasterization Platform (WARP), and macOS with hosted Metal, and the design document for CI will define them. An untagged test that needs other hardware would fail there.

**TEST-4.** Add a regression test to every bug fix that fails without the fix and passes with it. Reason: a fix without a test can return at the next refactor, and the failing run proves that the test covers the bug.

**TEST-5.** Write each example as a complete program that builds in every check, and run it in CI when its tags allow, so that the examples double as tests. Reason: an example that is only built can still fail when it runs, and the examples are the testbed of the project. [IR-49]

**TEST-6.** Test the failure paths, not only success: malformed shader source, invalid UTF-8, interior NUL bytes, missing files, and each branch of code that takes a lock. Assert that each returns the documented error kind, and inject Slang failures through the fake implementation of TEST-7. Reason: failure paths rarely run in normal use, so users find their bugs, and a panic on malformed input violates ERR-5. [IR-36] [IR-84] [S-10]

**TEST-7.** Test the unsafe code of the safe crate under Miri, with a fake implementation of the interfaces, written in Rust with its own vtables, in place of the native library. Do not treat a clean Miri run as a proof of soundness. Reason: Miri cannot call native code, but it detects undefined behaviour in the crate's reference counting, null handling, and lifetimes when they run against the fake, and it checks the executions that the tests run, not all executions. Miri is installed, and no recipe runs it yet (Section 4.8). [S-8] [S-10] [IR-27]

**TEST-8.** Keep two tests for the bindings: one that runs the binding generator into a temporary location and fails if the result differs from the checked-in bindings, and one that references the native symbols, so that a broken link fails a test instead of a user's build. Reason: checked-in bindings diverge silently from the generator and the headers, and the linker resolves a symbol only when code uses it. [S-3] [S-4]

**TEST-9.** Do not write tests that restate constants or mirror the implementation, and use property tests for conversions that round-trip, with their regression files in version control. Reason: a test that restates the implementation passes whatever the code does, and a property test that shrinks its failures finds inputs that the author did not think of. [S-10] [IR-13]

**TEST-10.** Give each crate exactly one integration-test binary, `tests/<suite>/main.rs`, with the tests in sibling module files and the shared helpers in a module declared in `main.rs` as `#[cfg(test)] mod common;`, and put no other Rust file directly in `tests/`. Reason: each Rust file directly in `tests/` is a separate test binary, so a helper module shared by several of them is dead code in each binary that does not use a given helper. `dead_code` then fires in that binary, while an `#[expect(dead_code)]` is unfulfilled in the binary that uses the helper, so no lint exception passes the check. A helper outside `#[cfg(test)]` code also triggers `clippy::unwrap_used` and `unreachable_pub`, because the test options of Section 4.3 cover only `#[test]` functions and `#[cfg(test)]` code. The single-binary layout produced no findings in the run of Section 1.1.

## 15. Dependencies and Cargo Features

**DEP-1.** Make every Cargo feature additive, so that every combination compiles, and name it after what it enables. A feature never removes behaviour or selects between options that exclude each other. Section 7.8 of the writing guide defines how features are documented. Reason: Cargo unifies the features that different dependents request, so a subtractive or exclusive feature breaks a build that combines them. [IR-8] [S-10] [S-4]

**DEP-2.** Add a dependency only when it replaces code or risk that outweighs its own cost, prefer the standard library, such as `std::sync::LazyLock`, and widely used crates, and declare every version once in `[workspace.dependencies]`. Reason: each dependency adds build time, licence obligations, and supply-chain risk, and two versions of one crate in a build have incompatible types. [IR-5] [IR-1] [S-10] [IR-3]

**DEP-3.** Treat every dependency whose types appear in a public signature as a public dependency, whose version is part of the crate's own. The GPU stack, wgpu and the crates around it, is below version 1.0, so the requirement of the Rust API Guidelines that a stable crate expose only stable dependencies (C-STABLE) cannot be met yet. Instead, keep every crate in this repository below 1.0, keep wgpu types out of `slang-compiler` and `slang-compiler-sys`, confine them to `slango` and `slang-slint`, and release a breaking wgpu upgrade as a breaking change of those crates. Reason: a breaking change of a public dependency is a breaking change of the crate, whether or not the crate's own code changed. [IR-8] [S-10] [IR-86]

**DEP-4.** Declare the MSRV as `rust-version` in the workspace manifest, and do not use language or library features newer than it. Develop on the latest stable toolchain, and use the nightly toolchain only for checkers such as Miri, never to build the crates. Reason: the declared version is a promise to users, and Clippy reads it to hold back suggestions that need a newer compiler. [S-10] [IR-4]

## 16. Simplicity

**SIMP-1.** Start with concrete types, and add a generic parameter, a trait, or a layer of indirection only when two implementations exist now. Remove an abstraction that gives no benefit now. Reason: predictions of future needs rarely hold, and an abstraction built on a prediction fixes a design before its requirements are known. [IR-93] [IR-72] [IR-81]

**SIMP-2.** Do not add a backend abstraction to `slango` before a second backend exists. Keep the code that depends on wgpu behind a small number of module boundaries, so that extracting a trait for a slang-rhi backend later changes only those modules. Reason: the right shape of a backend trait is known only once two backends exist, and contained wgpu code keeps that later change local. [IR-81] [IR-93]

**SIMP-3.** Write the examples of a new public API from the caller's side before implementing it, and rework the API until the common case is short and obvious. Reason: an API that is merely functional produces long and awkward examples, and the examples show the problem before other code depends on the API. [IR-63] [IR-44] [IR-93]

**SIMP-4.** Prefer plain functions and types to macros, and change code for performance only after a measurement shows the need. Reason: macros and hand-optimized code are harder to read, review, and change, and an unmeasured optimization usually targets the wrong code. [IR-93] [IR-72] [IR-51]

## 17. Resolved Conflicts Between Sources

This table records each disagreement between sources, or between a source and a lint, and the resolution this standard adopts. The precedence of Section 1.3 decides where the tiers differ, and the needs of this project decide where they do not.

| Conflict | Sources | Resolution | Reason |
| --- | --- | --- | --- |
| Blanket `From` conversions into the error, or context at each call site | `From` for `?`: [IR-56], [IR-86], [S-10]. Context per call site and no blanket `From`: [IR-34], [IR-2] | Context at each call site, and `From` only when it loses nothing (ERR-3) | One Slang result code comes from many calls, so a blanket `From` loses which call failed. |
| Error enum, or opaque struct with a kind | Enum: [IR-32], [IR-34], [IR-63]. Struct with kind: [IR-86], [S-10] | Opaque struct with a `#[non_exhaustive]` kind (ERR-1) | A result code with diagnostic text fits a struct, and the struct can grow without a breaking change. |
| Design traits for object safety, or only when `dyn` is needed | Object-safe if `dyn` is possible: [IR-8]. Only when needed: [IR-81] | Decide per trait and record it, with a compile test for `dyn` traits (API-11) | The tier 1 source asks for an early decision, and the tier 3 source shows the cost of designing every trait for `dyn`. |
| Consuming or non-consuming builders, or an options struct | Non-consuming: [IR-8], [IR-5], [IR-73]. Consuming: [IR-68], [IR-2]. Options struct: [IR-63] | Non-consuming by default, consuming when `build` takes ownership, and an options struct for a few independent options (API-6) | This follows [IR-8], whose exception for ownership covers builders that hold owning interface pointers. |
| Builder errors at each step, at the end, or in `build` | Deferred to execution: [IR-44]. Validating `build`: [IR-8], [IR-63] | A validating `build` that returns `Result` (API-6) | Tier 1 wins, and one fallible step is easy to call and to document. |
| Shared ownership to avoid lifetimes, arenas, or lifetimes on children | `Rc` and `Arc`: [IR-80]. Arena: [IR-54]. Lifetime on child: [IR-46] | A lifetime for short-lived children, a counted parent reference for stored children, and an arena for graphs (OWN-4, OWN-5) | Each tool fits one shape, and all three sources agree on avoiding lifetimes in stored public types. |
| Generic `AsRef` parameters everywhere, or only at the edge | Everywhere: [IR-63]. At the edge with a concrete inner function: [IR-69], [IR-40], [IR-48] | At the public edge only (API-13) | It keeps the convenience for callers without compiling the body once for each argument type. |
| Stored strings as `impl Into<String>`, `String`, or `&str` | `Into<String>`: [IR-55], [IR-57]. `String` by value: [IR-40]. Borrow: [IR-48] | `impl Into<String>` at the public edge, `String` inside, and `&str` for reads (API-13, OWN-1) | It follows the rule that a function takes ownership only of what it stores [IR-8]. |
| Module files `foo/mod.rs` or `foo.rs` | `foo/mod.rs`: [IR-2]. `foo.rs`: [IR-36] | `foo.rs`, enforced by `clippy::mod_module_files` | `foo.rs` is the default layout since edition 2018, and the house style of [IR-2] predates it. |
| Enabling `unsafe_code` | Do not enable: [IR-17]. Forbid in safe crates: [IR-22] | Deny for the workspace, with a crate-root `#[expect]` in the crates that call foreign code (Section 4.4) | Members cannot override workspace lints, and `forbid` cannot be overridden even by `#[expect]`. |
| Naming ignored fields, or `clippy::unneeded_field_pattern` | Name ignored fields: [IR-90], [IR-2]. The lint forbids it | Name them, with the lint off (TYPE-7) | A named field makes a new field a compile error at every manual implementation. |
| `as` casts | Ban all with `clippy::as_conversions`. Pedantic cast lints: [IR-74], [IR-92] | Pedantic cast lints, with a lint exception for intended casts (TYPE-5) | FFI code needs pointer casts that the blanket ban rejects. |
| Lifetime names | Always descriptive: [IR-2]. `'a` for a single borrow: [IR-30]. `clippy::single_char_lifetime_names` forbids `'a` | Elide where possible, descriptive names for two or more sources, lint off (OWN-3) | A long name for the only borrow adds nothing. |
| `impl Trait` in parameters | Use it: [IR-8], [IR-2]. `clippy::impl_trait_in_params` forbids it | Use it at the public edge, lint off (API-13) | Tier 1 favours the generic form. |
| A type named `Error` | `Error` and `Result` per crate: [IR-2]. `clippy::error_impl_error` forbids the name | Name it `Error`, lint off (ERR-1) | It matches `std::io::Error` and the convention that users expect. |
| Discarding a result with `.ok()` | `.ok()` to discard: [IR-2]. `clippy::unused_result_ok` forbids it | `let _ =` under a lint exception for `clippy::let_underscore_must_use` (ERR-5), with `clippy::unused_result_ok` on to reject `.ok()` | One form, with a stated reason, makes each discard reviewable, and both lints detect their form. |
| `#![deny(warnings)]` in source | Recommended: [IR-68]. Anti-pattern: [IR-5]. `CARGO_BUILD_WARNINGS=deny`: [IR-4] | `CARGO_BUILD_WARNINGS=deny` in the check (Section 4.2) | New compiler warnings would break users' builds, and `-D warnings` invalidates the build cache. |
| Iterator return types | `impl Iterator`: [IR-83]. Named types: [IR-17], [IR-8]. `Vec`: [IR-93] | Named types in public APIs, `Vec` for computed results, `impl Iterator` in private code (API-2) | Tier 1 names iterator types after their methods, and a named type can gain traits. |
| `Send` by default, or thread-affine handles | Public types `Send`: [S-10]. Raw-pointer handles neither: [S-1], [IR-27] | Neither for foreign handles until the Slang documentation proves otherwise (FFI-9) | Tier 1 wins, and a wrong `Send` is undefined behaviour while a missing one is only an inconvenience. |
| Checked-in or build-time bindings | Build time: [S-2]. Pre-generated: [S-4], [S-10]. Compare test: [S-3] | Checked in, generated by the binding generator, and verified by a compare test (TEST-8) | It is a project decision: users need no libclang, and the test keeps the files exact. |
| Documenting `-sys` items | Do not document them: [S-4]. Document every unsafe function: [S-10] | Lint exceptions on the generated module only (Section 7.9 of the writing guide) | Generated items mirror the Slang documentation, while hand-written items stay fully documented. |
| Panics in callbacks | Convert with `catch_unwind`: [S-1]. Last resort only: [S-10]. Abort at `extern "C"`: [S-5] | Catch only to return a failure code, otherwise abort (FFI-7) | Both positions hold in their case, and tier 1 states the abort. |
| Async by default, or blocking | Async by default: [IR-77]. Costs of async: [IR-78], [IR-88] | A blocking core with async at the edges (CONC-6) | Shader compilation is CPU-bound, and async bounds would spread through the API. |
| `#[non_exhaustive]` | Use with caution: [IR-5]. Standard mitigation: [IR-3] | Use it on types that mirror Slang or can grow (API-9) | Slang adds values between releases. |
| `Deref` on a read-only newtype | Allowed: [IR-6]. Anti-pattern outside smart pointers: [IR-5], [IR-8] | Smart pointers only (API-15) | Tier 1 wins. |
| Static and dynamic linking features | Neither as default: [S-4]. Build from bundled source with `cc` only: [S-10] | Dynamic linking to an installed SDK by default, static build from source as an opt-in feature (SYS-7) | It is a project decision, and the opt-in feature keeps every feature additive. |

## 18. Outdated Advice Not to Follow

Some advice in the sources is obsolete in 2026. This table lists it with its replacement.

| Outdated advice | Found in | Replacement |
| --- | --- | --- |
| The `try!` macro | [IR-8], [IR-56] | The `?` operator |
| `error-chain`, `failure`, and `quick-error` | [IR-0], [IR-56], [IR-67], [IR-51] | The error design of ERR-1, hand-written or with `thiserror` |
| `Error::description` and `Error::cause` | [IR-8], [IR-56] | `Display` and `Error::source` |
| `Box<Error>` without `dyn` | [IR-48], [IR-56] | `Box<dyn Error>`, in applications only (ERR-2) |
| `Result<(), ()>` in a public API | [IR-52] | The crate's `Error` (ERR-2) |
| `lazy_static` and `once_cell` | [IR-48], [IR-51] | `std::sync::LazyLock` and `std::sync::OnceLock` |
| The `async-trait` crate where native async traits serve | [IR-17], [IR-1] | Native `async fn` in traits, stable since Rust 1.75 (CONC-6) |
| Futures 0.1 combinator chains | [IR-39], [IR-0] | `async` and `.await` |
| `extern` blocks without `unsafe` | [IR-5], [IR-9], [IR-13] | `unsafe extern "C"` blocks (FFI-3) |
| `#[no_mangle]` and `#[export_name]` without `unsafe` | [IR-5], [IR-13] | `#[unsafe(no_mangle)]` and `#[unsafe(export_name = "...")]` |
| An `unsafe fn` body as an implicit unsafe block | [S-7] | Explicit `unsafe` blocks, enforced by `unsafe_op_in_unsafe_fn` |
| References to a `static mut` | [S-7] | Atomics, `Mutex`, `OnceLock`, or `&raw mut` for FFI |
| `std::env::set_var` as a safe function | [S-7] | An unsafe function in edition 2024, avoided entirely (CONC-4) |
| `mem::uninitialized` | [S-5] | `MaybeUninit` (UNS-7) |
| `#![deny(warnings)]` in source, `-D warnings` in `RUSTFLAGS` | [IR-68], [IR-17] | `CARGO_BUILD_WARNINGS=deny` (Section 4.2) |
| `#![warn(...)]` at the top of each `lib.rs` | [IR-17], [IR-48], [IR-63] | `[workspace.lints]` (Section 4.2) |
| Braces added only to end a borrow before non-lexical lifetimes | [IR-60] | Nothing. Borrows end at their last use. |
| A guard in an `if let ... else` scrutinee held through `else` | [IR-84] | Fixed by edition 2024 for `if let` only. `match` and `while let` still hold it (CONC-2). |
| The `Captures` trick and `impl Trait + 'a` for return types | [S-7] | Edition 2024 capture rules, with `use<...>` for precise capturing |
| A panic across `extern "C"` described as undefined behaviour | [IR-13], [S-1] | An abort since Rust 1.81, still avoided (FFI-7) |
| A run-time "parent alive" flag in `Rc<UnsafeCell<bool>>` | [IR-46] | A lifetime or a counted parent reference (OWN-4) |
| A drop guard that undoes native state | [IR-46] | A closure-scoped API (UNS-4) |
| `crossbeam` scoped threads | [IR-61] | `std::thread::scope` |
| The `FnBox` trick for `Box<FnOnce>` | [IR-6] | `Box<dyn FnOnce>` |
| `TryFrom` described as unstable | [IR-48], [IR-53], [IR-44] | `TryFrom`, stable and in the prelude since edition 2021 |
| `use std::convert::TryInto` | [IR-74] | Nothing, because the edition 2021 prelude contains it |
| `extern crate` and `#[macro_use] extern crate` | [IR-0], [S-4] | `use` paths |
| `foo/mod.rs` module files | [IR-2] | `foo.rs` (Section 17) |
| `#![warn(missing_doc_code_examples)]` on stable Rust | [IR-37] | Nothing on stable. Examples are a judgement rule (Section 7.5 of the writing guide). |
| The `cargo:` instruction prefix in build scripts | [S-2], [S-4] | `cargo::`, which needs Cargo 1.77 or later (SYS-1) |
| `#[link]` in Rust source when the build script links | [S-3] | Link directives from the build script only (FFI-2) |
| The lint names `temporary_cstring_as_ptr`, `clippy::string_to_string`, and `clippy::unchecked_duration_subtraction` | [IR-5], [IR-92] | `dangling_pointers_from_temporaries` for the first, as Section 4.7 records. The other two do not exist. `clippy::unchecked_time_subtraction` exists in `pedantic`, which the configuration enables. |

## 19. Extending the Standard

When an agent meets a design question that no rule covers, it follows these steps:

1. Find the topic in the reference list, `docs/rust-references.md`, read the sources it lists, and weigh them by the precedence of Section 1.3.
2. Write a proposed rule with the next unused identifier of its topic, or a new prefix if no topic fits, an imperative statement, the reason, and its sources.
3. Decide how the rule is enforced. If a lint or a tool can check it, verify that the lint exists on the current toolchain with `cargo clippy --explain` or `rustc -W help`, and add the lint to Sections 4.2 and 4.5 instead of adding a judgement rule.
4. Add a judgement rule to its section and to Appendix A, and add an entry to the change log.

An agent MUST add the rule in the same change as the code that first relies on it, because a reviewer cannot check code against a rule that does not exist yet, and a rule deferred to a later change is forgotten.

## 20. Security Considerations

This document defines rules, not a component, but its rules decide the security properties of every crate in the repository. The threat model has four parts. First, an application passes shader source, module names, and file paths that an attacker controls to the safe crate, and the attacker chooses their content but not the code of the application or of this repository. Second, an agent introduces a defect into unsafe code or into an FFI declaration, which turns a malformed input or a misuse into undefined behaviour. Third, a build script runs on the user's machine with the user's permissions and, for the static build, runs a native build of Slang from source. Fourth, a dependency contains a defect or a published vulnerability.

The rules address each part. For untrusted input, TYPE-5, TYPE-9, and FFI-5 keep sizes and indexes from input in range, and ERR-5 turns malformed input into an error instead of a panic. The design document of the safe crate states which files Slang can read through include paths and module search paths, as Section 3.2 of the writing guide requires. For unsafe code, Sections 9, 10, and 12 constrain every unsafe operation, its documentation, and its review, and the lints of Section 4.5 find unsafe code without a safety comment. For build scripts, SYS-5 forbids downloads, installs, and writes outside `OUT_DIR`, and SYS-6 and SYS-7 fix where the native library comes from. For dependencies, DEP-2 limits their number, and `cargo deny` checks advisories, licences, and sources (Section 4.8).

These attacks are out of scope. A malicious or compromised native Slang library or Vulkan SDK runs with the full privileges of the application, so no Rust rule can constrain it. Defects in GPU drivers lie below every crate here. Attacks on the repository itself, such as a malicious change, are a matter of access control, not of coding rules.

The residual risk has three parts. First, the soundness of unsafe code rests on judgement rules and on the contracts that the Slang documentation states, which no tool verifies, and Miri cannot run the native library. Second, until the configuration of Section 4 exists and the check runs the tools of Section 4.8, the rules they would enforce depend on review. Third, shader source from an attacker can make the native compiler use unbounded time or memory unless the application limits it, and no rule here prevents that.

## 21. References

Every `[IR-<n>]` and `[S-<n>]` identifier in this document resolves to its entry in Section 3 of the reference list, `docs/rust-references.md`, which owns the details of each of those sources, such as its author, year, and address. Sections 21.1 and 21.2 list only the cited sources that the reference list does not hold.

### 21.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 21.2. Informative References

- [SLANG-RS-26] "Objects must outlive Session?", issue 26 of the FloatyMonkey/slang-rs repository on GitHub, <https://github.com/FloatyMonkey/slang-rs/issues/26>.

## Appendix A. Reviewer's Reading List

This appendix lists every judgement rule with a one-line summary. A review agent loads this table, checks each change against the rules that apply, cites the identifier in each finding, and reads the full rule when a finding needs it.

| Rule | Summary |
| --- | --- |
| LINT-1 | Each lint exception sits on the narrowest scope and gives a checkable reason. |
| LINT-2 | The lint tables change only together with Sections 4.5 to 4.7 and the rule they enforce. |
| API-1 | Conversion methods are named `as_`, `to_`, or `into_` by cost and ownership, and getters have no `get_` prefix. |
| API-2 | Public functions return named iterator types that implement the iterator traits that hold. |
| API-3 | One name per concept, one word order, and Slang's names for Slang concepts. |
| API-4 | `From` only for lossless, infallible, non-panicking conversions, and `TryFrom` otherwise. |
| API-5 | Constructors follow the standard names, and `Default` exists only when the default is valid. |
| API-6 | A builder for four or more options, with infallible setters and a validating `build`. |
| API-7 | Public types implement the common traits that apply, `Copy` never on resource owners. |
| API-8 | Traits that users must not implement are sealed and documented as sealed. |
| API-9 | `#[non_exhaustive]` on types that mirror Slang, describe errors, or hold options, and a lint exception on closed types and on structs that other crates construct with a literal. |
| API-10 | Private fields, one path per item, no glob re-exports or preludes. |
| API-11 | Each trait's object safety is decided, documented, and tested when `dyn` is intended. |
| API-12 | Generic bounds over stored trait objects, no derivable bounds on structs, least restrictive closure bounds. |
| API-13 | Generic path and string parameters only at the public edge, over a non-generic inner function. |
| API-14 | Results are returned as values, not through out-parameters. |
| API-15 | `Deref` only on smart pointers. |
| API-16 | `Drop` never fails or blocks, with explicit `close` and wait methods. |
| TYPE-1 | Validation happens once, in the constructor of a newtype with a private field. |
| TYPE-2 | Enums replace meaningful `bool` values and option strings. |
| TYPE-3 | Interdependent fields are one enum, bounded values use bounded types, derived data is not stored. |
| TYPE-4 | Typestate for lifecycles where misuse is likely and the states are few. |
| TYPE-5 | `From` and `TryFrom` for integers, `as` only with a lint exception, `try_from` between `usize` and fixed-width integers, checked arithmetic on outside sizes. |
| TYPE-6 | Integers and enums convert through an explicit `match`, never `as` or `transmute`. |
| TYPE-7 | No `_` arm on enums of the same crate, and full destructuring in manual trait implementations. |
| TYPE-8 | Newtypes separate units, enums express single choices, and `bitflags` expresses combinable flags. |
| TYPE-9 | Indexes and lengths from outside the crate are read with `get`, slice patterns, or iterators. |
| ERR-1 | One opaque `Error` struct per library crate, with a `#[non_exhaustive]` `ErrorKind`, the result code, and the diagnostic text. |
| ERR-2 | No type-erased or foreign error types in a library's public API. |
| ERR-3 | Each fallible call site attaches its operation, and `From` exists only when it loses nothing. |
| ERR-4 | Lowercase, short `Display` text without the source's text, followed by the diagnostic text. |
| ERR-5 | Errors for everything outside the crate's control, panics only for the crate's own bugs, and discards only as `let _ =` under a lint exception. |
| ERR-6 | Every `unwrap`-like site is a stated invariant with an `internal error:` message, and no unfinished error handling is committed. |
| OWN-1 | Borrow what is read, take by value what is stored, never clone a borrowed parameter to store it. |
| OWN-2 | Return owned new data, borrowed parts, and `Cow` for sometimes-changed input. |
| OWN-3 | Elide lifetimes, no lifetime on stored public types, descriptive names for several borrows. |
| OWN-4 | Destruction-order rules of the native API are expressed in lifetimes or counted parent references. |
| OWN-5 | `Arc` for shared heavyweight objects, arenas with index identifiers for graphs. |
| OWN-6 | No `.clone()` to satisfy the borrow checker. |
| OWN-7 | Iterator adapters where they read well, loops where they read better, `Result` collected into `Result`. |
| UNS-1 | `unsafe` only for FFI, new abstractions, or measured performance. |
| UNS-2 | Unsafe code sits in the smallest module, with each invariant in one private helper. |
| UNS-3 | Safe functions are sound for every input, and `unsafe` marks only the risk of undefined behaviour. |
| UNS-4 | Soundness never depends on a destructor running. |
| UNS-5 | Soundness never depends on generic trait implementations or callbacks behaving well. |
| UNS-6 | `# Safety` sections and safety comments follow the writing guide, and `Send` or `Sync` impls and unsafe traits state their promises. |
| UNS-7 | Raw pointers become references only when non-null, aligned, valid, and unaliased. |
| FFI-1 | The `-sys` crate holds generated declarations only, and the safe crate re-exports none of them except through the raw-pointer methods of COM-11. |
| FFI-2 | `links = "slang-compiler"`, link directives from the build script, no `#[link]`. |
| FFI-3 | `unsafe extern "C"` declarations, `#[repr(C)]` types, C types, and `Option` for nullable pointers. |
| FFI-4 | `&CStr` tied to its owner or copied, `CString` bound to a variable, NUL bytes as errors. |
| FFI-5 | Slices from Slang are built after a null check, with element counts and an owner's lifetime. |
| FFI-6 | Every pointer from Slang is checked for null before use. |
| FFI-7 | No panic crosses the boundary, and callbacks catch panics only to return a failure code. |
| FFI-8 | Callback contexts outlive the registration, and callbacks from other threads are synchronized. |
| FFI-9 | Foreign handles are neither `Send` nor `Sync` until the Slang documentation proves otherwise. |
| FFI-10 | Open-ended C values are integer newtypes with constants, never Rust enums. |
| FFI-11 | Memory is freed by the library that allocated it. |
| SYS-1 | A rerun instruction for every input, the `cargo::` form, and declared custom `cfg` names. |
| SYS-2 | Build scripts read environment variables when they run, never with `env!` or `option_env!`. |
| SYS-3 | The target platform comes from `CARGO_CFG_TARGET_*`, never `cfg!`. |
| SYS-4 | Every spawned process has its exit status checked, and a failure is returned as an error from `main`. |
| SYS-5 | Writes only inside `OUT_DIR`, no downloads, no installs. |
| SYS-6 | The found Slang matches the pinned SDK version and Slang commit. |
| SYS-7 | Slang comes from an installed Vulkan SDK by default or a static source build by opt-in, nothing else. |
| COM-1 | One owning interface pointer type pairs every `AddRef` with one `Release`. |
| COM-2 | Owned and borrowed interface pointers are distinguished for every Slang function. |
| COM-3 | `QueryInterface` is a checked cast, never a pointer cast. |
| COM-4 | UUIDs come from generated constants, never typed by hand. |
| COM-5 | Children hold their parent, and the child's pointer field comes first. |
| COM-6 | Blob contents are slices borrowed from the blob. |
| COM-7 | Result codes map to `Error` in exactly one function. |
| COM-8 | Diagnostics blobs are always collected, into the error or as returned warnings. |
| COM-9 | Generated vtables, UUIDs, and thunks are never edited by hand. |
| COM-10 | Interface wrapper methods take `&self`. |
| COM-11 | Every interface wrapper has `from_raw`, `into_raw`, and `as_raw`. |
| COM-12 | The safe API follows Rust types, not the interface hierarchy. |
| COM-13 | Each interface wrapper reaches its vtable through one helper, so each vtable call is one unsafe operation. |
| CONC-1 | Each public type's `Send` and `Sync` status is decided, documented, and asserted in a test. |
| CONC-2 | Lock guards live in their own short-lived binding, never in a `match` or `while let` scrutinee. |
| CONC-3 | Poisoned locks are recovered or turned into errors on purpose. |
| CONC-4 | No hidden threads, no library statics, no changes to the process environment. |
| CONC-5 | Atomic orderings match their purpose, and `SeqCst` prompts a review. |
| CONC-6 | A blocking core, async only for external waits, no `Send + 'static` bounds unless spawning. |
| CONC-7 | Every public `async fn` documents its cancel safety and holds nothing across `.await`. |
| TEST-1 | Standard locations for unit tests, integration tests, and examples, and descriptive test names. |
| TEST-2 | Snapshot tests with `insta` for text and structured output, with deterministic output. |
| TEST-3 | Tests that need the native library or a GPU are tagged so they can be filtered. |
| TEST-4 | Every bug fix comes with a regression test. |
| TEST-5 | Examples are complete programs that build in every check and run where their tags allow. |
| TEST-6 | Failure paths are tested and return the documented error kinds. |
| TEST-7 | Unsafe code is tested under Miri against a fake implementation in Rust. |
| TEST-8 | A regenerate-and-compare test and a link test cover the bindings. |
| TEST-9 | No tests that restate the implementation, and property tests for round trips. |
| TEST-10 | One integration-test binary per crate, `tests/<suite>/main.rs`, with sibling modules and a `#[cfg(test)] mod common;`. |
| DEP-1 | Cargo features are additive and named after what they enable. |
| DEP-2 | Dependencies are few, preferably standard, and declared once in the workspace. |
| DEP-3 | wgpu types stay out of the compiler crates, and wgpu upgrades are breaking releases of the crates that expose them. |
| DEP-4 | The MSRV is declared and respected, and nightly serves checkers only. |
| SIMP-1 | Concrete types first, and abstractions only for two implementations that exist now. |
| SIMP-2 | No backend abstraction before a second backend exists, with wgpu code contained. |
| SIMP-3 | Examples of a new API are written first and the API is reworked until they are clean. |
| SIMP-4 | Plain code over macros, and optimization only after measurement. |

## Change Log

- 2026-11-10: Marked the tools of Section 4.8 as installed, added their verified commands with `--no-tests=warn` and the nextest form of Miri, corrected the `deny.toml` rule on path dependencies, and recorded the arguments of the `coverage` recipe.
- 2026-11-10: Initial version.
