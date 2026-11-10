# Rust References

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-10 |
| Applies to | Agents that write, review, or extend Rust code in this repository, and the standards document `docs/rust-coding-standards.md` |

## Abstract

This document lists the external sources behind the Rust coding standards of this repository and organizes them for lookup by topic. It holds a topic index, a catalogue of 105 sources with the tier, currency, and relevance of each and the rules derived from it, the topics that no source covers well, the sources that could not be fully read, and the procedure for maintaining the list. When a change introduces a design pattern or raises a design question, an agent uses it to find what the sources say and to check whether the standards already cover the case.

## 1. Introduction

The standards document `docs/rust-coding-standards.md` draws its judgement rules from a curated list of Rust writing and from supplementary sources. Its rules cover the topics that the project needed when it was written. A later change can use a pattern that no rule covers, such as a new kind of builder, a graph of foreign objects, or an async entry point. This list lets an agent find the sources for that pattern, judge how far to trust each one, and see which rules each source already supports. The agent then finds and fills a gap in the standard before code depends on it.

This document states no requirement on code. Section 6 states how to maintain the list.

### 1.1. How to Use This List

An agent that is about to use a design pattern, or that faces a design question, follows these steps:

1. Find the topic in Section 2. When no topic fits, search the "Consult when" fields of the catalogue in Section 3.
2. Read the listed sources, starting with the highest tier (Section 1.3). Before relying on a source, read its catalogue entry for its read status and for what in it is outdated.
3. Read the rules that the topic lists in `docs/rust-coding-standards.md`, together with Sections 17 and 18 of that document, which record resolved conflicts and outdated advice.
4. If no rule covers the question, follow Section 19 of `docs/rust-coding-standards.md`, "Extending the Standard". If no source covers the topic either, Section 4 of this document says what kind of source would.

### 1.2. Identifiers

Every source has one identifier, used in this document and in `docs/rust-coding-standards.md` alike:

- `[IR-<n>]`: the entry at zero-based index n of the file `resources.json` in the curated list at <https://github.com/mre/idiomatic-rust>, as of the snapshot that Section 6.2 records.
- `[S-<n>]`: supplementary source n, which is not part of the curated list. Sources `[S-1]` to `[S-10]` were chosen mainly for the foreign function interface (FFI), `-sys` crates, and unsafe code, which the curated list covers weakly. Some of them, such as `[S-10]`, also cover other topics.

An identifier is never reused or renumbered (Section 6). The catalogue in Section 3 holds the entry for every identifier that this document cites, so it takes the place of a References section. Section 21 of `docs/rust-coding-standards.md` points to the catalogue in the same way.

### 1.3. Precedence Tiers

Each catalogue entry gives the tier of its source under the precedence rule in Section 1.3 of `docs/rust-coding-standards.md`, which also says how to settle a disagreement within a tier. This document writes the tiers as 1, 2, and 3, in the order that section lists them. Where that section does not name a source, the entry assigns the tier by the same criteria and says that the tier is assigned here.

### 1.4. Entry Fields

Each catalogue entry in Section 3 has these fields, in this order. The field Chapters appears only when the notes record pages. The notes are the reading notes on which the catalogue is based.

- Author: the person or organization that the curated list or the notes name, or "not recorded".
- Year: the year in the curated list, with any correction that the notes made. For a supplementary source, the year the notes give.
- URL: the address in the curated list. When a different address was the one actually read, such as an archive copy, a rendered book, or slides, the field gives that address too.
- Kind: article, book, talk, workshop, project, forum, or documentation.
- Tier: 1, 2, or 3, as Section 1.3 describes.
- Read status: full, partial, or unreachable, followed by what was read or tried. "Full for" and a scope, such as "full for the crates page", limits the status to that scope.
- Current: yes, mostly, partly, no, or unknown, with what in the source is outdated.
- Relevance: high, medium, or low for this project, or a range such as "low to medium", with the reason.
- Consult when: the situations in which an agent opens the source.
- Chapters: for a book, course, or large site, the pages that the notes record as read or worth reading.
- Cited by: the rule identifiers in `docs/rust-coding-standards.md` whose text cites the source, and any other sections of that document that cite it.

## 2. Topic Index

Each subsection lists the sources for one topic, most useful first, with what each contributes, and then the rules in `docs/rust-coding-standards.md` that cover the topic. A rule identifier names a judgement rule, and its prefix names the section of the standards document that holds it, as Section 3 of that document defines. A source can appear under several topics.

### 2.1. API Design and Naming

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | The canonical checklist: naming, common traits, predictability, flexibility, future proofing, and debuggability. |
| [S-10] | 2 | A 2026 organizational standard: one path per item, no glob re-exports, escape hatches, and API shape for AI agents. |
| [IR-2] | 2 | Naming, import, and ordering discipline, with a reason for each rule. |
| [IR-17] | 2 | Field visibility, accessors without `get_`, and named public iterator types. |
| [IR-63] | 3 | Nine rules for library APIs, from a port of a Python library to Rust. |
| [IR-48] | 3 | Conversion-friendly parameters, extension traits, and lazy APIs. |
| [IR-68] | 3 | A condensed checklist for a new library crate. |
| [IR-3] | 3 | The API stability table of breaking changes. |
| [IR-73] | 3 | Which standard traits to implement, and naming. |
| [IR-38] | 3 | Return values instead of out-parameters. |
| [IR-66] | 3 | Ergonomics heuristics: explicitness, consistency, and convention over configuration. |
| [IR-64] | 3 | Static typing with selective type erasure, from a related post. |
| [IR-52] | 3 | The `as_`, `to_`, and `into_` naming table. |
| [IR-36] | 3 | Small checklist items from a live code review. |

Rules: API-1, API-3, API-7, API-10, API-14, API-15, and the rest of Section 5 of the standards document.

### 2.2. Builders and Constructors

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | C-BUILDER, and constructors as static inherent methods. |
| [S-10] | 2 | Builders for four or more options. |
| [IR-2] | 2 | `builder()` returning a builder type, and constructor discipline. |
| [IR-10] | 2 | Standard method naming conventions. Its naming sub-pages were not read. |
| [IR-5] | 3 | The builder, constructor, and `Default` idioms. |
| [IR-63] | 3 | Builders, or a plain configuration struct with `Default`. |
| [IR-44] | 3 | Builder steps whose types limit the next call, and deferred errors. |
| [IR-73] | 3 | Choosing between consuming and non-consuming builders. |
| [IR-68] | 3 | The case for consuming builders. |
| [IR-51] | 3 | Builders and one-shot types in ripgrep. |

Rules: API-5, API-6. Section 17 of the standards document records the builder conflicts.

### 2.3. Conversions

These sources cover `From`, `Into`, `TryFrom`, `AsRef`, `Borrow`, and `FromStr`.

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | Implementing `From`, `TryFrom`, and `AsRef` rather than `Into`, and placing a conversion on the more specific type. |
| [IR-53] | 3 | Which conversion trait to implement, and mapping enums to and from integers. |
| [IR-82] | 3 | Conversions on newtypes, `AsRef`, `Deref`, and `Borrow`. |
| [IR-90] | 3 | `TryFrom` instead of a `From` that panics. |
| [IR-73] | 3 | `From` and `TryFrom`, never `Into`, and `FromStr`. |
| [IR-48] | 3 | `Into` and `AsRef` parameters with a non-generic inner function. |
| [IR-55] | 3 | A worked example of `Into<String>` parameters and `From` shorthands. |
| [IR-44] | 3 | Not treating fallible conversions as `From`. |

Rules: API-1, API-4, TYPE-6.

### 2.4. Newtypes and Type-Driven Design

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-10] | 2 | The newtype pattern, parse-don't-validate, and borrow checker invariants. |
| [IR-82] | 3 | The most complete newtype guide: validating constructors, traits, and unchecked construction. |
| [IR-28] | 3 | Invariants in types with private fields, and non-empty collections. |
| [IR-90] | 3 | Defensive patterns that replace "never happens" comments with types. |
| [IR-92] | 3 | Bounded types and mutually dependent fields as an enum. |
| [IR-45] | 3 | Units of measure with zero-sized markers. |
| [IR-5] | 3 | The newtype pattern. |
| [IR-41] | 3 | Enums instead of booleans. |
| [IR-29] | 3 | Not storing derived data. |
| [IR-33] | 3 | Validated newtypes with `TryFrom` in a service. |

Rules: TYPE-1, TYPE-2, TYPE-3, TYPE-8.

### 2.5. Typestate and State Machines

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-10] | 2 | The typestate pattern with generics, and an embedded example. |
| [IR-50] | 3 | State as a type parameter, consuming transitions, and an enum wrapper for dynamic state. |
| [IR-42] | 3 | Typestate in a layered hardware abstraction. |
| [IR-48] | 3 | Session types. |
| [IR-68] | 3 | Session types to enforce call order. |
| [IR-73] | 3 | Typestate with `PhantomData`. |
| [IR-23] | 3 | A short typestate sketch. |
| [IR-44] | 3 | Builder chains whose types offer only the valid next calls. |

Rules: TYPE-4.

### 2.6. Enums and Pattern Matching

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-2] | 2 | Pattern-matching discipline and exhaustive destructuring. |
| [IR-94] | 3 | `match`, `if let`, `matches!`, `let`-`else`, guards, and bindings. |
| [IR-90] | 3 | No wildcard arms on the crate's own enums, and full destructuring. |
| [IR-41] | 3 | Two-variant enums instead of `bool`. |
| [IR-7] | 3 | Choosing between an enum and a trait object. |
| [IR-6] | 3 | Matching on a full tuple to remove nesting. |
| [IR-47] | 3 | `Option` combinators instead of nested `match`. |
| [IR-31] | 3 | One puzzle solved with match guards. |

Rules: TYPE-2, TYPE-7, API-9, FFI-10.

### 2.7. Error Handling in Libraries

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-10] | 2 | Canonical error structs. |
| [IR-2] | 2 | Error message wording, error type design, and converting foreign errors early. |
| [IR-10] | 2 | `thiserror` for libraries, `anyhow` for applications, and errors at the C++ boundary. |
| [IR-86] | 3 | An opaque error struct with an `ErrorKind`, and keeping third-party errors out of the API. |
| [IR-34] | 3 | One variant per fallible call site and no blanket `From`. |
| [IR-56] | 3 | The classic library error design, with outdated tooling. |
| [IR-32] | 3 | An error enum that wraps lower-level sources. |
| [IR-63] | 3 | `thiserror` enums and boxing large errors. |
| [IR-75] | 3 | A short refresher on `thiserror` and `anyhow`. |
| [IR-43] | 3 | Collecting fallible results into one `Result`. |
| [IR-24] | 3 | Error-handling recipes. |

Rules: ERR-1, ERR-2, ERR-3, ERR-4, COM-7, COM-8. Section 17 of the standards document records the error-type conflicts.

### 2.8. Panics and Unwrap

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-10] | 2 | Panic means stop. |
| [IR-2] | 2 | When to panic, and `expect` over `unwrap`. |
| [IR-13] | 2 | `assert!`, `debug_assert!`, and panics in `Drop`. |
| [IR-87] | 3 | Panics for local bugs only, and libraries that return `Result`. |
| [IR-76] | 3 | Three kinds of `unwrap`, and which of them to ship. |
| [IR-89] | 3 | `unwrap` and `todo!` as removable markers in prototypes. |
| [IR-92] | 3 | No `unwrap` or `expect` outside tests. |
| [IR-56] | 3 | Panics for bugs only. |
| [IR-23] | 3 | `unwrap` abuse and panics in libraries as anti-patterns. |

Rules: ERR-5, ERR-6. Section 2.22 covers panics at the FFI boundary.

### 2.9. Strings and String Parameters

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | C-CALLER-CONTROL and C-GENERIC for parameter types. |
| [IR-79] | 3 | `String` or `&str` for parameters, returns, and fields. |
| [IR-40] | 3 | `&str` for reading, `String` by value for storing, and `impl AsRef<str>`. |
| [IR-57] | 3 | `Into<String>` for stored strings. |
| [IR-58] | 3 | Returning `Cow` when input is only sometimes changed. |
| [IR-63] | 3 | Accepting every kind of string at the public edge. |
| [IR-48] | 3 | `Cow` and `AsRef` with a non-generic inner function. |
| [IR-85] | 3 | Pragmatic string parameters for applications. |

C strings at the FFI boundary appear in Section 2.20. Rules: API-13, OWN-1, OWN-2, FFI-4.

### 2.10. Paths

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-13] | 2 | `AsRef<Path>`-style generic parameters. |
| [IR-69] | 3 | `AsRef<Path>` only on public methods, over a concrete inner function. |
| [IR-63] | 3 | `AsRef<Path>` parameters, with the opposite view on how far to generalize. |
| [IR-7] | 3 | The non-generic inner function that limits monomorphization. |
| [IR-40] | 3 | A footnote on `AsRef<Path>`. |
| [IR-92] | 3 | Checking absolute paths before `Path::join`. |

Rules: API-13. Section 17 of the standards document records the resolution.

### 2.11. Ownership and Borrowing

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-10] | 2 | No `Rc`, `Arc`, `Box`, or `RefCell` in public signatures. |
| [IR-80] | 3 | Avoiding lifetimes in public types, and `Rc` or `Arc` as alternatives. |
| [IR-46] | 3 | C destroy-order rules expressed in Rust ownership. |
| [IR-5] | 3 | `mem::take` and `mem::replace`, and cloning to satisfy the borrow checker as an anti-pattern. |
| [IR-79] | 3 | Owned strings in structs. |
| [IR-60] | 3 | Strategies for "cannot move out of" errors, partly obsolete since non-lexical lifetimes (NLL). |
| [IR-29] | 3 | Immutable bindings and moving ownership. |
| [IR-85] | 3 | Cloning in non-hot application code. |
| [IR-91] | 3 | Borrow checker errors as design feedback. |

Rules: OWN-1, OWN-2, OWN-4, OWN-5, OWN-6.

### 2.12. Lifetimes

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-1] | 1 | `PhantomData`, variance, subtyping, and drop checking. |
| [S-7] | 1 | Lifetime capture in `-> impl Trait` returns in edition 2024. |
| [IR-2] | 2 | Eliding lifetimes and naming them. |
| [IR-10] | 2 | `PhantomData` lifetimes and borrow checker invariants. |
| [IR-80] | 3 | When not to add a lifetime parameter. |
| [IR-30] | 3 | Naming lifetimes after their borrow source. |
| [IR-46] | 3 | Child lifetimes tied to a parent object. |
| [IR-48] | 3 | Descriptive lifetime names. |

Rules: OWN-3, OWN-4, COM-5.

### 2.13. Iterators

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | `iter`, `iter_mut`, and `into_iter`, and naming iterator types. |
| [IR-17] | 2 | Named public iterator types with extra traits. |
| [IR-83] | 3 | `impl Iterator` against `Box<dyn Iterator>`, readability, laziness, and performance. |
| [IR-43] | 3 | Collecting into `Result`, `filter_map`, and `partition`. |
| [IR-59] | 3 | Choosing `iter`, `iter_mut`, or `into_iter`. |
| [IR-93] | 3 | Returning a concrete `Vec` when every element is computed anyway. |
| [IR-70] | 3 | Iterator chains against loops, as a style discussion. |
| [IR-71] | 3 | Accepting `impl IntoIterator`. |
| [IR-23] | 3 | Iterator adapters over index loops. |

Rules: API-2, OWN-7.

### 2.14. Collections, Trees, and Graphs

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-54] | 3 | A `Vec` arena with index identifiers instead of `Rc<RefCell<..>>`. |
| [IR-80] | 3 | `Rc` and `Arc` for shared ownership. |
| [IR-6] | 3 | `Borrow` for using a set as a map. |
| [IR-24] | 3 | Recipes for data structures. |

Rules: OWN-5.

### 2.15. Traits and Generics

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | C-GENERIC, sealed traits, and no derivable bounds on structs. |
| [IR-10] | 2 | Sealed traits, extension traits, and monomorphization. |
| [IR-81] | 3 | Generic bounds instead of stored trait objects. |
| [IR-7] | 3 | Conditional implementations and non-generic inner functions. |
| [IR-3] | 3 | Writing generic code that stays sound against hostile trait implementations. |
| [IR-93] | 3 | Generics only for implementations that exist now. |
| [IR-5] | 3 | A helper trait for repeated bounds, and the strategy pattern. |
| [IR-62] | 3 | Extractor-style traits, content not read. |
| [IR-64] | 3 | Associated types, from a related post. |
| [IR-55] | 3 | Traits for developer-friendly libraries. |
| [IR-26] | 3 | Chapter 11, Traits and Generics, not read. |

Rules: API-8, API-11, API-12, UNS-5, SIMP-1.

### 2.16. Object Safety and Trait Objects

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | C-OBJECT: deciding between trait objects and generics. |
| [IR-10] | 2 | Dyn compatibility, dyn against generics, and enums against dyn. |
| [IR-81] | 3 | Not designing for object safety unless `dyn` is needed. |
| [IR-7] | 3 | Three things to try when a trait cannot be a trait object. |
| [IR-64] | 3 | Type erasure only at escape hatches. |
| [IR-0] | 3 | An index that lists essays on object safety. |

Rules: API-11, API-12. Section 17 of the standards document records the object-safety conflict.

### 2.17. Callbacks and Closures

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-1] | 1 | Callback trampolines, user-data pointers, and no callbacks after destruction. |
| [IR-13] | 2 | `extern "C"` callback functions. |
| [IR-3] | 3 | Closure bounds in the order `FnOnce`, `FnMut`, `Fn`. |
| [IR-61] | 3 | `Send` and `Sync` bounds on closures as the basis of safe parallelism. |
| [IR-62] | 3 | Extractor-style registration APIs, content not read. |

Rules: API-12, FFI-7, FFI-8.

### 2.18. Macros

Declarative macros are covered by [IR-8], [IR-44], and [IR-28]. Procedural macros are covered by [IR-15] and the unread [IR-65].

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | The macro chapter: input syntax that resembles the output, attributes, scope, and visibility. |
| [S-10] | 2 | A chapter on macros. |
| [IR-15] | 3 | Procedural macros: `syn`, `quote`, `trybuild` compile-fail tests, and error spans. |
| [IR-44] | 3 | Crate-private declarative macros, and renaming a macro during refactoring. |
| [IR-28] | 3 | `compile_error!` in a macro, combined with a type that carries the invariant. |
| [IR-72] | 3 | No speculative macros. |
| [IR-64] | 3 | Plain Rust before macros. |
| [IR-25] | 3 | Chapter 7 on macros, not read. |
| [IR-26] | 3 | Chapter 21, Macros, not read. |
| [IR-65] | 3 | A talk on macro idioms, content not read. |

Rules: SIMP-4.

### 2.19. Unsafe Code and Soundness

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-1] | 1 | Why the unsafe rules exist: opaque types, `Send` and `Sync`, `PhantomData`, leaking, and privacy as the unsafe boundary. |
| [S-6] | 1 | `# Safety` and `// SAFETY:` conventions, and the vocabulary of soundness and validity. |
| [S-5] | 1 | The exact list of undefined behaviour, and unsafe operations. |
| [S-7] | 1 | Edition 2024 unsafe changes: `unsafe_op_in_unsafe_fn`, unsafe attributes, and `static mut`. |
| [S-10] | 2 | Unsafe only for FFI, novel abstractions, or performance, and never an unsound safe API. |
| [IR-10] | 2 | The unsafe chapter and its documentation conventions. |
| [IR-13] | 2 | Safe interfaces over unsafe code, with preconditions asserted first. |
| [IR-27] | 3 | Centralizing an unsafe invariant in one helper, and testing with Miri. |
| [IR-5] | 3 | Containing unsafe code in the smallest module. |
| [IR-3] | 3 | Unsafe, unsound, and undefined, and adversarial code. |
| [IR-46] | 3 | Never relying on a guard's `Drop` for soundness. |
| [IR-22] | 3 | `forbid(unsafe_code)` on safe crates, and undefined behaviour as a whole-program property. |
| [IR-0] | 3 | Essays on unsafe abstractions, listed in its catalogue entry. |
| [IR-35] | 3 | Replacing pointer arithmetic with slices. |
| [IR-25] | 3 | Chapter on unsafe code, not read. |
| [IR-26] | 3 | Chapter 22, Unsafe Code, not read. |

Rules: UNS-1 to UNS-7.

### 2.20. FFI Declarations and Safe Wrappers

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-1] | 1 | `unsafe extern` declarations, opaque types, nullable pointers, and `CString` and `CStr`. |
| [S-5] | 1 | Extern blocks, `safe fn` items, ABI strings, and `MaybeUninit` for out-parameters. |
| [S-6] | 1 | `CStr::from_ptr` lifetimes and `NonNull`. |
| [S-7] | 1 | `unsafe extern` and `#[unsafe(no_mangle)]` in edition 2024. |
| [IR-13] | 2 | C types, opaque types, null checks, and the `-sys` and safe crate split. |
| [IR-10] | 2 | Calling C, `bindgen`, and child handles with `PhantomData`. |
| [S-10] | 2 | No leaking of `-sys` types, and `from_native` and `into_native` escape hatches. |
| [IR-7] | 3 | Inbound and outbound FFI with a `-sys` crate and a safe wrapper. |
| [IR-5] | 3 | FFI idioms for strings and errors, and object-based wrappers. Written for outbound FFI. |
| [IR-46] | 3 | Soundness, cost, and usability in C bindings. |
| [IR-42] | 3 | Layering a raw layer, generated bindings, and safe abstractions. |
| [IR-35] | 3 | Pointer-and-length pairs as slices. |
| [IR-38] | 3 | Converting C out-pointers into return values. |
| [IR-9] | 1 | A minimal safe wrapper over an extern block, with outdated syntax. |
| [IR-26] | 3 | Chapter 23, Foreign Functions, not read. |

Rules: FFI-1 to FFI-11, API-14.

### 2.21. Bindings, Sys Crates, Build Scripts, and Linking

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-3] | 1 | `links`, `DEP_*` metadata, rerun rules, `OUT_DIR`, and checked-in generated files. |
| [S-2] | 1 | The `bindgen` builder: allowlists, opaque types, C++ limits, and layout tests. |
| [S-4] | 3 | A practical recipe for a `-sys` crate: features, environment overrides, static and dynamic linking, and versioning. |
| [S-10] | 2 | M-SYS-CRATES and naming of `-sys` crates. |
| [IR-13] | 2 | `bindgen` from a build script, and crate types. |
| [IR-7] | 3 | The `-sys` crate layering. |
| [IR-24] | 3 | Compiling C and C++ from a build script with `cc`. |
| [S-9] | 3 | Profile settings, which apply only at the root of a binary. |

Rules: SYS-1 to SYS-7, FFI-1, FFI-2, TEST-8, DEP-1. Section 17 of the standards document records the conflict over checked-in bindings.

### 2.22. Panics at the FFI Boundary

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-5] | 1 | The unwinding table for each ABI. |
| [S-1] | 1 | No panic across `extern "C"`, and `catch_unwind` only to translate. Its unwinding page is outdated. |
| [S-10] | 2 | `catch_unwind` as a last resort. |
| [IR-13] | 2 | `catch_unwind` at the boundary. Outdated on undefined behaviour. |
| [IR-10] | 2 | Never unwinding a panic across FFI, and errors across the C++ boundary. |
| [IR-87] | 3 | Unwinding, the FFI ABI, and `panic=abort`. |
| [S-9] | 3 | `panic=abort` makes `catch_unwind` inert. |

Rules: FFI-7. Section 4.4 describes the gap in edition 2024 forms.

### 2.23. COM-Style and Reference-Counted Foreign Objects

No source in this list covers COM-style interfaces, `AddRef` and `Release` pairing, `QueryInterface` with universally unique identifiers (UUIDs), or the generation of vtables. The rules in Section 12 of `docs/rust-coding-standards.md`, COM-1 to COM-13, cite no source from this list. They are project decisions. Section 4.1 and Section 4.2 describe the gap. These sources cover neighbouring ideas:

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-1] | 1 | Owning handles over foreign objects, `Send` and `Sync`, and `PhantomData` for child objects. |
| [IR-10] | 2 | RAII handles, drop bombs, and `PhantomData<&'a mut Parent>` for child handles. |
| [IR-27] | 3 | Building a reference-counted pointer: orderings for the count, and overflow handling. |
| [IR-46] | 3 | Parent and child handles with a native destroy order. |
| [IR-5] | 3 | Object-based APIs and RAII guards over foreign handles. |

Rules: COM-1 to COM-13, OWN-4, FFI-9.

### 2.24. C++ Interoperability

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-10] | 2 | CXX bridges, C++ exceptions, the limits of CXX, and error handling in Chromium. |
| [S-2] | 1 | What `bindgen` cannot translate from C++: templates, inline functions, inheritance, and exceptions. |
| [IR-0] | 3 | An index that lists essays on C++ integration, not evaluated. |

No rule cites a source on C++ interoperability. The safe crate reaches C++ only through the COM-style interface of Section 12 of the standards document.

### 2.25. Drop, RAII, and Resource Cleanup

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | Destructors that never fail, and an alternative to destructors that block. |
| [S-1] | 1 | Leaking, and soundness that never depends on a destructor. |
| [IR-10] | 2 | RAII, drop guards, drop bombs, and scope guards. |
| [IR-13] | 2 | No panic in `Drop`, and a consuming `close()` for fallible teardown. |
| [IR-5] | 3 | RAII guards and finalization in destructors. |
| [IR-46] | 3 | A drop guard for native state, withdrawn by its own author. |
| [IR-88] | 3 | No async cleanup in `Drop`. |
| [IR-27] | 3 | Not assuming that destructors run. |

Rules: API-16, UNS-4, COM-1.

### 2.26. Concurrency

These sources cover threads, atomics, locks, `Send`, and `Sync`.

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-1] | 1 | `Send` and `Sync`, and `unsafe impl` with a justification. |
| [S-7] | 1 | The `if let` temporary scope change, and `set_var` becoming unsafe. |
| [S-10] | 2 | Public types are `Send` unless documented, and no statics in libraries. |
| [IR-13] | 2 | `Send` and `Sync` through `unsafe impl`. |
| [IR-27] | 3 | Memory orderings, atomics, reference counts, poisoning, and scoped threads. |
| [IR-84] | 3 | Lock guards in `if let` and `match` scrutinees. |
| [IR-3] | 3 | No hidden threads and no global state. |
| [IR-61] | 3 | Data parallelism with `join`, and sequential fallback. |
| [IR-24] | 3 | Recipes for threads, channels, and atomics. |

Rules: CONC-1 to CONC-5, FFI-9. Section 17 of the standards document records the conflict over `Send` handles.

### 2.27. Async and Cancellation

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-17] | 2 | Native `async fn` in traits. |
| [IR-88] | 3 | Cancel safety, guards across `.await`, `select!`, and async cleanup. |
| [IR-78] | 3 | No `Send + 'static` bounds unless the library spawns. |
| [IR-77] | 3 | Async for composability rather than speed. |
| [IR-39] | 3 | `.await` and `?` instead of futures combinators. |
| [IR-26] | 3 | Chapter 20, Asynchronous Programming, not read. |

Rules: CONC-6, CONC-7, API-16.

### 2.28. Numeric Conversions and Casts

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-74] | 3 | `From` and `TryFrom` instead of `as`. |
| [IR-92] | 3 | Checked arithmetic, `NonZero`, bounded types, and no `as`. |
| [IR-53] | 3 | Enums to integers through `match`, integers to enums through `TryFrom`. |
| [IR-45] | 3 | Integer base units and checked arithmetic. |
| [IR-3] | 3 | `From` and `TryFrom` instead of `as` casts. |
| [IR-22] | 3 | Overflow wraps in release builds. |

Rules: TYPE-5, TYPE-6, TYPE-9. Section 17 of the standards document records the conflict over `as`.

### 2.29. Defensive Programming and Untrusted Input

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-90] | 3 | Slice patterns, no length check followed by indexing, and controlled construction. |
| [IR-92] | 3 | Size limits on external input, time-of-check to time-of-use, and path checks. |
| [IR-87] | 3 | No panic on caller input. |
| [IR-22] | 3 | Test vectors and differential fuzzing. |
| [IR-35] | 3 | Fuzzing and dynamic checks when refactoring unsafe code. |

Rules: TYPE-9, ERR-5, TYPE-5.

### 2.30. Edition 2024 Changes

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-7] | 1 | Every edition 2024 change that affects this project, chapter by chapter. |
| [S-5] | 1 | The current form of extern blocks. |
| [IR-84] | 3 | The `if let` deadlock that edition 2024 fixes, and the `match` case it does not. |

Rules: FFI-3, CONC-2, CONC-4, DEP-4. Section 18 of the standards document lists the outdated forms.

### 2.31. Documentation and Rustdoc

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | The documentation chapter: crate-level docs, examples with `?`, and `# Errors`, `# Panics`, and `# Safety`. |
| [S-6] | 1 | `# Safety` sections and `// SAFETY:` comments. |
| [S-10] | 2 | Canonical documentation sections and a short first sentence. |
| [IR-10] | 2 | The anatomy of a doc comment, and library against application docs. |
| [IR-2] | 2 | Comment discipline and the first sentence of a doc comment. |
| [IR-37] | 3 | Writing crate documentation, by a rustdoc maintainer. |
| [IR-49] | 3 | Guides against reference documentation. |
| [IR-63] | 3 | Testing the README as crate documentation. |
| [IR-9] | 1 | Doctest syntax reminders. |

Rules: TEST-5. Section 7 of `docs/writing-documentation.md` governs API documentation.

### 2.32. Testing

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-8] | 1 | Miri: what it detects, what it cannot run, and how to set it up in CI. |
| [S-3] | 1 | A test that verifies checked-in generated files. |
| [IR-17] | 2 | Test layout, naming, and snapshot tests with `insta`. |
| [IR-13] | 2 | Property tests with `proptest`. |
| [IR-10] | 2 | Unit, integration, and doc tests, and lints. |
| [S-10] | 2 | Mockable system calls and no tautological tests. |
| [IR-63] | 3 | API tests in `tests/`. |
| [IR-27] | 3 | Drop counters, stress tests, and Miri for unsafe code. |
| [IR-15] | 3 | Compile-fail tests with `trybuild`. |
| [IR-22] | 3 | Fuzzing and assurance tools. |
| [IR-36] | 3 | Tests for malformed input, and fuzzing. |
| [S-4] | 3 | A test that references C symbols, to check linking. |
| [IR-84] | 3 | Testing each locking branch. |
| [IR-9] | 1 | Syntax for unit tests and doctests. |

Unit tests, doctests, snapshot tests, fuzzing, and Miri are each covered above. Rules: TEST-1 to TEST-10.

### 2.33. Lints and Tooling

These sources cover Clippy, rustfmt, `cargo deny`, and the other tools.

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-4] | 1 | Lint groups, `[workspace.lints]`, `clippy.toml`, `#[expect]`, and CI. |
| [S-10] | 2 | `#[expect]` with a reason, and gating commits on lints and tools. |
| [IR-2] | 2 | rustfmt and Clippy settings. |
| [IR-17] | 2 | Lint settings and CI targets. Its `#![warn]` in each `lib.rs` is outdated. |
| [IR-7] | 3 | Allow-by-default rustc lints. |
| [IR-90] | 3 | Lints for indexing, wildcard matches, and `#[must_use]`. |
| [IR-92] | 3 | A lint list, part of which no longer exists. |
| [IR-1] | 3 | Tools such as `cargo-deny`, `cargo-semver-checks`, and `cargo-nextest`. |
| [IR-22] | 3 | `cargo audit`, Kani, and siderophile. |
| [IR-5] | 3 | `#![deny(warnings)]` in source as an anti-pattern. |
| [IR-91] | 3 | Enabling pedantic Clippy early. |
| [IR-24] | 3 | Development tool recipes. |

Rules: LINT-1, LINT-2, and Section 4 of the standards document.

### 2.34. Cargo Features and Dependencies

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-8] | 1 | Direct, additive feature names, and C-STABLE on stable public dependencies. |
| [S-10] | 2 | Additive features and a single workspace manifest. |
| [S-4] | 3 | Features against environment variables, and static against dynamic linking. |
| [IR-1] | 3 | Crate choices by category. |
| [IR-3] | 3 | The semver breaking-change table. |
| [IR-5] | 3 | Small crates and duplicate versions. |

Rules: DEP-1 to DEP-4, SYS-7.

### 2.35. Project and Crate Layout

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-10] | 2 | Core and glue crates, and a flat workspace. |
| [IR-2] | 2 | Structural discipline and module files. |
| [IR-51] | 3 | Single-purpose crates in a workspace. |
| [IR-7] | 3 | A `-sys` crate with a safe wrapper crate. |
| [IR-42] | 3 | Four layers of abstraction. |
| [IR-36] | 3 | `foo.rs` with `foo/`, and a library with a binary in one crate. |

Rules: FFI-1, API-10. Section 17 of the standards document records the conflict over `mod.rs`.

### 2.36. Architecture and Abstraction Boundaries

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-81] | 3 | Concrete types first, and traits extracted later. |
| [IR-93] | 3 | No speculative generalization. |
| [IR-72] | 3 | Excessive abstraction as an anti-pattern. |
| [IR-33] | 3 | Hiding a backend behind a trait to test without it. |
| [IR-42] | 3 | Layered abstractions over hardware. |
| [IR-64] | 3 | Retained and declarative APIs over GPU or UI state. |
| [IR-5] | 3 | The strategy pattern for backends. |

Rules: SIMP-1, SIMP-2, DEP-3, API-12.

### 2.37. Performance

| Source | Tier | What it contributes |
| --- | --- | --- |
| [S-9] | 3 | Build configuration for binaries and benchmarks. |
| [IR-83] | 3 | Measuring before rewriting iterators as loops. |
| [IR-72] | 3 | Premature optimization and profiling. |
| [IR-93] | 3 | Measuring instead of avoiding allocations. |
| [IR-3] | 3 | Performance tips. |
| [IR-51] | 3 | Buffer reuse, and no `inline(always)` without profiling. |
| [IR-58] | 3 | `String::with_capacity`. |
| [IR-7] | 3 | Limiting monomorphization. |
| [IR-61] | 3 | When to parallelize. |

Rules: SIMP-4.

### 2.38. Simplicity and Prototyping

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-93] | 3 | Simplicity as a design rule. |
| [IR-89] | 3 | Prototyping, and which markers to remove before committing. |
| [IR-72] | 3 | Four kinds of bad code. |
| [IR-63] | 3 | Writing examples before the API. |
| [IR-44] | 3 | Designing the API from the caller's side. |
| [IR-66] | 3 | Simplicity for the caller over simplicity of the implementation. |
| [IR-85] | 3 | Pragmatic shortcuts for applications. |
| [IR-91] | 3 | Modelling with types first. |

Rules: SIMP-1 to SIMP-4.

### 2.39. Code Review

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-51] | 3 | A review of ripgrep's design. |
| [IR-36] | 3 | A live review of a small crate. |
| [IR-72] | 3 | A review checklist of anti-patterns. |
| [IR-23] | 3 | Short anti-pattern examples. |
| [IR-92] | 3 | Pitfalls that pass the compiler. |
| [IR-90] | 3 | Defensive patterns to look for. |

Rules: Appendix A of the standards document lists every judgement rule for review.

### 2.40. Command-Line Tools

The binding generator is a command-line tool, and these sources apply to it.

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-19] | 3 | Error reporting, exit codes, and testing for command-line programs, not read. |
| [IR-36] | 3 | A library and a binary in one crate. |
| [IR-20] | 3 | Clones of Unix tools, with code only. |

Rules: Section 1.1 of the standards document lets applications use `Box<dyn std::error::Error>`.

### 2.41. Learning Material

These sources teach Rust and yield few rules. An agent consults them for background, not for decisions.

| Source | Tier | What it contributes |
| --- | --- | --- |
| [IR-10] | 2 | A maintained course with advanced tracks. |
| [IR-25] | 3 | A book for experienced Rust programmers, not read. |
| [IR-26] | 3 | A comprehensive book, table of contents only. |
| [IR-9] | 1 | Examples of language features. |
| [IR-0] | 3 | An index of essays. |
| [IR-91] | 3 | How to learn Rust. |
| [IR-16] | 3 | Beginner exercises. |
| [IR-18] | 3 | A university course. |
| [IR-11] | 3 | A text editor tutorial. |
| [IR-12] | 3 | Test-driven koans. |
| [IR-14] | 3 | A distributed systems course. |
| [IR-21] | 3 | An embedded course. |

## 3. Catalogue

The catalogue holds 105 entries: one for each of the 95 entries of the curated list, `[IR-0]` to `[IR-94]`, and one for each supplementary source, `[S-1]` to `[S-10]`. Section 1.4 defines the fields. An entry whose source yielded no rules stays in the catalogue, so that a later agent knows the source was considered.

### 3.1. Curated List Entries

**[IR-0] Rust Anthology**

- Author: not recorded. The notes name essay authors, such as Niko Matsakis, Ralf Jung, BurntSushi, and nrc, but no single author.
- Year: 2018.
- URL: <https://github.com/brson/rust-anthology>. Read: <https://github.com/brson/rust-anthology/blob/master/master-list.md>.
- Kind: project, an index of about 150 essays.
- Tier: 3.
- Read status: partial. The README and the master list were read. The essays were indexed but not opened.
- Current: mostly. The essays date from 2015 to 2018, and anything on futures 0.1, mio, error-chain, `try!`, Travis, pre-2018 modules, pre-NLL borrow checking, macros 1.1, or `extern crate` is outdated.
- Relevance: medium. It is an index, not a rule source. Its index lists essays on unsafe code, traits, API idioms, and FFI, which were not opened.
- Consult when: designing unsafe abstractions, trait objects, conversion APIs, builders, state machines, or interior mutability, reasoning about `Send` and `Sync`, or asking whether someone has written an essay on a topic.
- Chapters: the notes record the essays on unsafe code as the only ones worth a later read:
  - <http://smallcultfollowing.com/babysteps/blog/2016/05/23/unsafe-abstractions/>
  - <http://smallcultfollowing.com/babysteps/blog/2016/05/27/the-tootsie-pop-model-for-unsafe-code/>
  - <http://smallcultfollowing.com/babysteps/blog/2016/08/18/tootsie-pop-followup/>
  - <http://smallcultfollowing.com/babysteps/blog/2016/09/12/thoughts-on-trusting-types-and-unsafe-code/>
  - <http://smallcultfollowing.com/babysteps/blog/2016/10/02/observational-equivalence-and-unsafe-code/>
  - <http://smallcultfollowing.com/babysteps/blog/2017/01/22/assigning-blame-to-unsafe-code/>
  - <http://smallcultfollowing.com/babysteps/blog/2017/02/01/unsafe-code-and-shared-references/>
  - <http://smallcultfollowing.com/babysteps/blog/2015/04/29/on-reference-counting-and-leaks/>
  - <https://www.ralfj.de/blog/2016/01/09/the-scope-of-unsafe.html>
  - <https://www.ralfj.de/blog/2017/06/09/mutexguard-sync.html>
  - <http://cglab.ca/~abeinges/blah/rust-unsafe-intro/>
  - <http://cglab.ca/~abeinges/blah/everyone-poops/>
  - <https://huonw.github.io/blog/2014/07/what-does-rusts-unsafe-mean/>
  - <https://huonw.github.io/blog/2016/04/memory-leaks-are-memory-safe/>
  - <https://gankro.github.io/blah/only-in-rust/>
- Cited by: no rule. Section 18.

**[IR-1] blessed.rs**

- Author: not recorded.
- Year: 2019. It is a living page, read in 2026.
- URL: <https://blessed.rs/crates>.
- Kind: project.
- Tier: 3.
- Read status: full for the crates page. The sections on Rust versions, learning, and other lists were not read.
- Current: yes. It carries current notes, such as `LazyLock` in the standard library.
- Relevance: high. It guides dependency choices for an FFI, graphics, and Slint workspace. It does not list wgpu, ash, vulkano, libloading, rstest, proptest, or mockall.
- Consult when: picking a crate for errors, FFI generation, logging, tests and benchmarks, graphics, windowing, bit flags, byte casting, or CI tooling.
- Cited by: DEP-2, TEST-2. Section 18.

**[IR-2] Canonical's Rust Best Practices**

- Author: Canonical.
- Year: 2025.
- URL: <https://canonical.github.io/rust-best-practices/>.
- Kind: project, written as a book.
- Tier: 2, named in Section 1.3 of the standards document.
- Read status: full. All 13 content chapters were read. The contributing page was skipped.
- Current: mostly. A few rules are house style that conflicts with other sources or with Clippy lints, such as `foo/mod.rs` layout, single-letter lifetime names, `impl Trait` in parameters, a type named `Error`, and discarding with `.ok()`.
- Relevance: high. It is an organization-wide standard of short imperative rules, each with a reason.
- Consult when: writing or reviewing naming, import layout, struct and impl ordering, builders, error message wording, error types, panic policy, unsafe hygiene, or the first sentence of a doc comment.
- Chapters: `naming-discipline`, `import-discipline`, `pattern-matching-discipline`, `code-discipline`, `error-and-panic-discipline`, `function-discipline`, `ordering-discipline`, `unsafe-discipline`, `structural-discipline`, and `comment-discipline`.
- Cited by: API-3, API-6, CONC-3, ERR-2, ERR-3, ERR-4, ERR-5, ERR-6, OWN-3, TYPE-7, UNS-1. Sections 1.3, 17, and 18.

**[IR-3] cheats.rs - Idiomatic Rust tips**

- Author: not recorded.
- Year: 2020. The page is updated regularly.
- URL: <https://cheats.rs>.
- Kind: project, a single cheat-sheet page.
- Tier: 3.
- Read status: partial. The sections Idiomatic Rust, Performance Tips, Closures in APIs, Unsafe Unsound Undefined, Adversarial Code, API Stability, Type Conversions, and Foreign Types and Traits were read. The language reference tables and the rest were not read in detail.
- Current: yes, for the sections read.
- Relevance: medium to high. Its API stability table, adversarial code table, and unsafe summary map directly onto library rules.
- Consult when: deciding whether a change breaks semver, writing generic code that must stay sound against hostile trait implementations, choosing `Fn`, `FnMut`, or `FnOnce` bounds for callbacks, or reviewing unsafe soundness.
- Chapters: the anchors `#idiomatic-rust`, `#api-stability`, `#adversarial-code`, `#unsafe-unsound-undefined`, `#closures-in-apis`, `#type-conversions`, and `#performance-tips`.
- Cited by: API-9, API-12, CONC-4, DEP-2, TYPE-5, UNS-3, UNS-5. Section 17.

**[IR-4] clippy**

- Author: not recorded.
- Year: 2015, maintained since.
- URL: <https://github.com/rust-lang/rust-clippy>. Read: the Clippy book at <https://doc.rust-lang.org/clippy/>.
- Kind: project, read through its book.
- Tier: 1, named in Section 1.3 of the standards document for Clippy.
- Read status: full for the pages listed below. The GitHub Actions page and the development chapters were not read.
- Current: yes.
- Relevance: high. It defines how every deterministic style and correctness check is configured and run.
- Consult when: choosing lint groups, writing the `[workspace.lints]` table, adding `clippy.toml`, suppressing a lint, or running Clippy in CI.
- Chapters: `lints.html`, `configuration.html`, `usage.html`, `continuous_integration/index.html`, and `lint_configuration.html`.
- Cited by: DEP-4. Sections 1.3 and 17.

**[IR-5] Patterns**

- Author: not recorded.
- Year: 2016, with content updated into 2026.
- URL: <https://rust-unofficial.github.io/patterns/>. Read: the Markdown source at <https://github.com/rust-unofficial/patterns>.
- Kind: project, written as a book.
- Tier: 3.
- Read status: full. Every idiom, pattern, and anti-pattern page was read, and both FFI sections in full. The Command, Interpreter, Visitor, and Fold pages and the functional pages were read for their description and discussion only.
- Current: mostly. The FFI samples use spellings from before edition 2024, such as `#[no_mangle]`, `extern "C" {}`, and `libc::c_char`. The page on `deny(warnings)` lists lints as of rustc 1.48, and the lint `temporary_cstring_as_ptr` it names no longer exists.
- Relevance: high. Its FFI idioms, unsafe containment, RAII guards, newtype, builder, `#[non_exhaustive]`, and the `Deref` anti-pattern apply directly. Its FFI pages address outbound FFI, while this project binds inbound.
- Consult when: designing a safe wrapper around a raw handle, passing strings across the C boundary, converting errors, designing a builder or newtype, deciding whether to implement `Deref`, or extending a public type without a breaking change.
- Chapters: under `src/` in the source repository: `idioms/ffi/accepting-strings.md`, `idioms/ffi/passing-strings.md`, `idioms/ffi/errors.md`, `patterns/ffi/wrappers.md`, `patterns/ffi/export.md`, `patterns/structural/unsafe-mods.md`, `patterns/behavioural/RAII.md`, `idioms/dtor-finally.md`, `patterns/behavioural/newtype.md`, `patterns/creational/builder.md`, `idioms/ctor.md`, `idioms/default.md`, `idioms/priv-extend.md`, `idioms/mem-replace.md`, `anti_patterns/borrow_clone.md`, `anti_patterns/deref.md`, `anti_patterns/deny-warnings.md`, `patterns/structural/compose-structs.md`, and `patterns/behavioural/strategy.md`.
- Cited by: API-5, API-9, API-15, DEP-2, FFI-4, FFI-11, OWN-4, OWN-6, UNS-2. Sections 17 and 18.

**[IR-6] Elements of Rust**

- Author: Ferrous Systems.
- Year: 2019.
- URL: <https://github.com/ferrous-systems/elements-of-rust>.
- Kind: project, a single README.
- Tier: 3, assigned here, because it is a collection of techniques and not an organizational standard.
- Read status: full.
- Current: mostly. The `Box<FnOnce>` workaround is obsolete since Rust 1.35, one example uses `failure`, and the empty-enum "never" rule is partly obsolete.
- Relevance: low to medium. It offers small control-flow and iterator techniques, immutable finalized configuration, and `Borrow` for set lookup.
- Consult when: removing nesting from `match` or combinator chains, collecting iterators of `Result`, capturing `Arc` values in closures, or modelling finalized configuration.
- Cited by: no rule. Sections 17 and 18.

**[IR-7] Possible Rust**

- Author: not recorded.
- Year: 2020, with posts from 2020 to 2026.
- URL: <https://www.possiblerust.com/>.
- Kind: project, a site of posts and guides.
- Tier: 3.
- Read status: partial. Eight of about ten posts were read. The FFI and lint posts were read in detail, the others from their openings and headings only.
- Current: mostly. The lints post lists some historical lints.
- Relevance: high for the inbound and outbound FFI guide, which describes the `-sys` crate and safe wrapper layering of this project, and medium for the rest.
- Consult when: structuring a `-sys` crate against a safe crate, choosing between an enum and a trait object, making a trait usable as a trait object, reducing monomorphization in generic wrappers, or choosing allow-by-default rustc lints.
- Chapters: `/guide/inbound-outbound-ffi`, `/pattern/non-generic-inner-functions`, `/guide/enum-or-trait-object`, `/pattern/3-things-to-try-when-you-cant-make-a-trait-object`, `/pattern/conditional-impls`, `/pattern/naming-your-lifetimes`, `/pattern/rust-lints-you-may-not-know`, and `/guide/what-can-coerce-and-where-in-rust`.
- Cited by: API-11, API-13, FFI-1.

**[IR-8] Rust API Guidelines**

- Author: not recorded.
- Year: 2017. It is a maintained living document.
- URL: <https://rust-lang.github.io/api-guidelines/>.
- Kind: project.
- Tier: 1, named in Section 1.3 of the standards document.
- Read status: full. The checklist and all 11 chapters were read, and 54 guideline identifiers were recorded.
- Current: mostly. It still mentions `Error::description`, uses `try!` in C-QUESTION-MARK, carries a caveat from before `impl Trait` in C-NEWTYPE-HIDE, and uses `lazy_static`-era examples. C-STABLE cannot be met with the `0.x` GPU stack this project uses.
- Relevance: high. It is the canonical checklist for the safe crate, the high-level crate, and the Slint integration crate.
- Consult when: naming a public item, deciding which traits a public type implements, designing a builder, error type, feature, or sealed trait, writing rustdoc, or preparing a release.
- Chapters: `checklist.html`, `naming.html`, `interoperability.html`, `macros.html`, `documentation.html`, `predictability.html`, `flexibility.html`, `type-safety.html`, `dependability.html`, `debuggability.html`, `future-proofing.html`, and `necessities.html`.
- Cited by: API-1, API-2, API-3, API-4, API-5, API-6, API-7, API-8, API-10, API-11, API-12, API-14, API-15, API-16, CONC-1, DEP-1, DEP-3, ERR-1, ERR-4, OWN-1, TYPE-8. Sections 1.3, 17, and 18.

**[IR-9] Rust by Example**

- Author: not recorded.
- Year: 2015.
- URL: <https://doc.rust-lang.org/rust-by-example/>.
- Kind: project, an online book.
- Tier: 1, assigned here as a book the Rust project publishes, for the language features it demonstrates.
- Read status: partial. The index and the pages below were read. Later error pages, traits, macros, threads, and channels were not read.
- Current: mostly. The FFI chapter uses `extern {}` without `unsafe` and `#[link]`, which edition 2024 replaces with `unsafe extern "C" {}`.
- Relevance: low. It is introductory.
- Consult when: needing a syntax reminder for doctest attributes, `#[should_panic(expected = "...")]`, `#[ignore]`, tests that return `Result`, or a minimal safe wrapper over an extern block.
- Chapters: `unsafe.html`, `std_misc/ffi.html`, `testing/unit_testing.html`, `testing/doc_testing.html`, `meta/doc.html`, and `error/multiple_error_types.html`.
- Cited by: no rule. Section 18.

**[IR-10] Comprehensive Rust**

- Author: not recorded. The notes record only that Google hosts it.
- Year: 2021, maintained through 2025 and 2026.
- URL: <https://github.com/google/comprehensive-rust>. Read: <https://google.github.io/comprehensive-rust/>.
- Kind: workshop, a course.
- Tier: 2, named in Section 1.3 of the standards document.
- Read status: partial. The full table of contents and about 30 chapters were read: Unsafe Rust except Unions, the C and C++ interoperability pages, Chromium's pages on CXX and error handling, the Idiomatic Rust chapters, `thiserror`, `anyhow`, and testing. Concurrency, bare metal, and the naming sub-pages were not read.
- Current: yes. It covers edition 2024, `unsafe extern`, and `safe fn` in extern blocks.
- Relevance: high. Its Idiomatic Rust track, C++ interoperability chapter, and Unsafe chapter apply directly.
- Consult when: writing or reviewing unsafe code or extern blocks, designing RAII handles, typestate builders, newtypes, sealed traits, or extension traits, structuring doc comments, designing error types, or deciding how errors and panics cross a C++ boundary.
- Chapters: under the read URL: `unsafe-rust/unsafe-functions/extern-c.html`, `unsafe-rust/unsafe-traits.html`, `android/interoperability/with-c/bindgen.html`, `android/interoperability/cpp.html`, `chromium/interoperability-with-cpp/limitations-of-cxx.html`, `chromium/interoperability-with-cpp/error-handling.html`, `idiomatic/leveraging-the-type-system/newtype-pattern.html`, `idiomatic/leveraging-the-type-system/raii.html`, `idiomatic/leveraging-the-type-system/typestate-pattern.html`, `idiomatic/leveraging-the-type-system/borrow-checker-invariants.html`, `idiomatic/polymorphism/generics/sealed-traits.html`, `idiomatic/polymorphism/dyn.html`, `idiomatic/foundations-api-design/meaningful-doc-comments.html`, and `error-handling/thiserror.html`.
- Cited by: API-8, FFI-3, FFI-5, FFI-11, OWN-4, TEST-1, TYPE-1, TYPE-4, TYPE-8, UNS-1, UNS-6. Section 1.3.

**[IR-11] Hecto: Build your own text editor in Rust**

- Author: not recorded.
- Year: 2019.
- URL: <https://philippflenker.com/hecto/>.
- Kind: workshop.
- Tier: 3.
- Read status: partial. The landing page and chapter list only.
- Current: mostly. The early version uses an older crossterm and manual raw mode.
- Relevance: low. It is an application tutorial with no API design, FFI, or unsafe content.
- Consult when: never for this project.
- Cited by: no rule.

**[IR-12] Build your own JIRA with Rust**

- Author: not recorded.
- Year: 2020.
- URL: <https://github.com/LukeMathWalker/build-your-own-jira-with-rust/>.
- Kind: workshop.
- Tier: 3.
- Read status: partial. The README only.
- Current: mostly. The notes give no reason.
- Relevance: low. It is a test-driven workshop of koans for newcomers.
- Consult when: only if onboarding material needs a test-driven exercise format.
- Cited by: no rule.

**[IR-13] Ferrous Systems Teaching Material**

- Author: Ferrous Systems.
- Year: 2019, maintained since.
- URL: <https://ferrous-systems.github.io/teaching-material/index.html>.
- Kind: workshop, slide modules.
- Tier: 2, named in Section 1.3 of the standards document.
- Read status: partial. The index and the ten modules below were read.
- Current: mostly. The FFI slides use `#[no_mangle]` and bare extern blocks, which edition 2024 replaces. The slides say that unwinding across FFI is undefined behaviour, but since Rust 1.81 it aborts.
- Relevance: high for FFI, unsafe code, panics, and `Drop`, and medium for testing and design patterns.
- Consult when: writing the `-sys` crate and its build script, deciding how panics are handled at the FFI boundary, or writing unsafe abstractions.
- Chapters: `ffi.html`, `unsafe.html`, `drop-panic-abort.html`, `send-and-sync.html`, `dynamic-and-static-libs.html`, `error-handling.html`, `testing.html`, `proptest.html`, `documentation.html`, and `design-patterns.html`.
- Cited by: API-16, CONC-1, FFI-1, FFI-3, FFI-6, FFI-7, FFI-9, FFI-11, TEST-9. Sections 1.3 and 18.

**[IR-14] PingCAP talent plan**

- Author: PingCAP.
- Year: 2018.
- URL: <https://github.com/pingcap/talent-plan>.
- Kind: workshop, a project-based course.
- Tier: 3.
- Read status: partial. The README only.
- Current: mostly. Its one Rust course targets async-era Rust only in part.
- Relevance: low. It is a distributed systems course.
- Consult when: building a concurrent or networked service. Not for FFI or API design.
- Cited by: no rule.

**[IR-15] Procedural Macros Workshop**

- Author: not recorded.
- Year: 2019.
- URL: <https://github.com/dtolnay/proc-macro-workshop>.
- Kind: workshop.
- Tier: 3.
- Read status: partial. The README only. The five project specifications and their test cases were not read.
- Current: mostly. `syn`, `quote`, and `trybuild` are still the standard toolkit, but the README predates `syn` 2, so check current `syn` documentation for API names.
- Relevance: medium. The project may add derive or attribute macros, and the README gives the testing method for macros.
- Consult when: writing or testing a procedural macro, or deciding how a macro reports errors.
- Cited by: no rule.

**[IR-16] rust-lang/rustlings**

- Author: rust-lang.
- Year: 2018.
- URL: <https://github.com/rust-lang/rustlings>.
- Kind: workshop, beginner exercises.
- Tier: 3.
- Read status: partial. The README only.
- Current: yes. It is maintained.
- Relevance: low. It teaches beginner syntax.
- Consult when: never for this project.
- Cited by: no rule.

**[IR-17] Rust Development at Sentry**

- Author: Sentry.
- Year: 2020. The content has been updated since and mentions native async functions in traits.
- URL: <https://develop.sentry.dev/engineering-practices/rust/>.
- Kind: workshop, a single page of guidelines.
- Tier: 2, named in Section 1.3 of the standards document.
- Read status: full.
- Current: mostly. Its advice not to enable `unsafe_code` is house style, and its `#![warn(...)]` in each `lib.rs` and `-D warnings` through flags are replaced by `[workspace.lints]`.
- Relevance: medium. It is short and aimed at published SDK crates, so its rules on visibility and iterator types transfer directly.
- Consult when: deciding field visibility and accessors, public iterator types, async functions in traits, CI targets, snapshot testing, or versioning before 1.0.
- Cited by: API-1, API-2, CONC-6, TEST-1, TEST-2. Sections 1.3, 17, and 18.

**[IR-18] Rust 101**

- Author: Trifecta Tech Foundation. The course originated at tweede golf.
- Year: 2019.
- URL: <https://teach-rs.trifectatech.org/>.
- Kind: workshop, a university course.
- Tier: 3.
- Read status: partial. The landing page only. The fetch exposed no module list.
- Current: mostly. The notes give no reason.
- Relevance: low. The notes give no reason.
- Consult when: never for this project, unless a specialized track is needed.
- Cited by: no rule.

**[IR-19] Command Line Applications in Rust**

- Author: not recorded.
- Year: 2024.
- URL: <https://rust-cli.github.io/book/>.
- Kind: book.
- Tier: 3.
- Read status: partial. The landing page only.
- Current: yes.
- Relevance: low. It is about command-line binaries, which this project builds only as tools, such as the binding generator.
- Consult when: building a command-line tool, or for error reporting, exit codes, and testing of a command-line binary.
- Chapters: the notes name the chapters Nicer error reporting, Output for humans and machines, Testing, and Packaging and distribution, without addresses.
- Cited by: no rule.

**[IR-20] Command-Line Rust**

- Author: not recorded.
- Year: 2022.
- URL: <https://github.com/kyclark/command-line-rust>.
- Kind: book. The repository holds code only, and the book text is paid.
- Tier: 3.
- Read status: partial. The README only.
- Current: mostly. The main branch uses clap 4.
- Relevance: low. The notes give no reason.
- Consult when: never for this project.
- Cited by: no rule.

**[IR-21] Discover the world of microcontrollers through Rust!**

- Author: not recorded.
- Year: 2023.
- URL: <https://docs.rust-embedded.org/discovery/>.
- Kind: book.
- Tier: 3.
- Read status: partial. The landing page only.
- Current: yes.
- Relevance: low. It is an embedded hardware course. For embedded typestate, [IR-10] has a page.
- Consult when: never for this project.
- Cited by: no rule.

**[IR-22] High Assurance Rust**

- Author: not recorded.
- Year: 2022, still in progress.
- URL: <https://highassurance.rs/>.
- Kind: book.
- Tier: 3.
- Read status: partial. The single-page print version was indexed, and the sections on undefined behaviour, recommended tooling, and dynamic validation, the Tools of the Trade appendix, and the discussion of `forbid(unsafe_code)` were read. The chapters on unsafe code, FFI, and fuzzing are unfinished stubs, and the book has no Miri chapter.
- Current: mostly. Its tool inventory does not mention sanitizers or `cargo-careful`.
- Relevance: low to medium. It offers a tool inventory and general framing of undefined behaviour, not library design.
- Consult when: choosing assurance tools such as Miri, `cargo-fuzz`, Kani, `cargo-audit`, or siderophile, or explaining why `forbid(unsafe_code)` on safe crates is a useful audit signal.
- Chapters: <https://highassurance.rs/print.html> and <https://highassurance.rs/chp16_appendix/tools.html>.
- Cited by: TYPE-5. Section 17.

**[IR-23] Idiomatic Rust Snippets**

- Author: not recorded.
- Year: 2025.
- URL: <https://idiomatic-rust-snippets.org>.
- Kind: book.
- Tier: 3.
- Read status: partial. About 25 of roughly 120 pages were read: the anti-pattern pages on ownership, error handling, code structure, and the type system, several design pattern pages, and the packaging pages.
- Current: mostly. It is beginner level, and its `Result<_, String>` library error example conflicts with [IR-8].
- Relevance: low to medium. It mostly restates [IR-5] and Clippy defaults, but it lists anti-patterns that agents commit.
- Consult when: looking for a short worked example of one anti-pattern, such as excessive cloning, `&str` against `String`, panics in libraries, or stringly typed enums, or for a typestate or extension trait sketch.
- Cited by: OWN-7, TYPE-4.

**[IR-24] Rust Cookbook**

- Author: rust-lang-nursery.
- Year: 2021, updated since.
- URL: <https://github.com/rust-lang-nursery/rust-cookbook>. Read: <https://rust-lang-nursery.github.io/rust-cookbook/>.
- Kind: book, a collection of recipes.
- Tier: 3.
- Read status: partial. The index and the pages below were read.
- Current: mostly. The notes give no reason.
- Relevance: medium. It has no FFI section, but covers `cc` in a build script, `thiserror` and `anyhow`, concurrency primitives, and `tracing`.
- Consult when: choosing a crate for a common task, such as error types, threads and channels, logging, `semver`, or compiling C or C++ from a build script with `cc`.
- Chapters: `errors/handle.html`, `concurrency.html`, `safety_critical.html`, and `development_tools.html`.
- Cited by: no rule.

**[IR-25] Rust for Rustaceans**

- Author: Jon Gjengset, with David Tolnay as technical reviewer.
- Year: 2021.
- URL: <https://nostarch.com/rust-rustaceans>.
- Kind: book, paid.
- Tier: 3.
- Read status: partial. The table of contents and the free sample listing only. The sample chapter was not read.
- Current: mostly. It predates guidance on `let`-`else`, generic associated types, async functions in traits, `unsafe extern`, and the edition 2024 unsafe changes.
- Relevance: high. Its chapters on interfaces, errors, project structure, testing, macros, async, unsafe code, concurrency, and FFI map directly onto this project.
- Consult when: the free sources do not settle a question on API design, unsafe code, FFI, or macros.
- Chapters: the free sample at <https://nostarch.com/download/samples/RustforRustaceans_Ch1.pdf>.
- Cited by: no rule.

**[IR-26] Programming Rust: Fast, Safe Systems Development**

- Author: not recorded.
- Year: 2021, second edition.
- URL: <https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586/>. Read: <https://web.archive.org/web/2id_/https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586/>.
- Kind: book, paid.
- Tier: 3.
- Read status: partial. The publisher's page returned HTTP 403, and an archive copy gave the table of contents only.
- Current: mostly. It was written for Rust 2018 and predates `let`-`else`, generic associated types, async functions in traits, and edition 2024.
- Relevance: medium. The notes give no reason.
- Consult when: needing a book-length treatment of unsafe code, foreign functions, traits and generics, or async.
- Chapters: chapter 22, Unsafe Code, and chapter 23, Foreign Functions, for the FFI crates. Also chapters 7 (Error Handling), 11 (Traits and Generics), 19 (Concurrency), 20 (Asynchronous Programming), and 21 (Macros). No per-chapter addresses are recorded.
- Cited by: no rule.

**[IR-27] Rust Atomics and Locks**

- Author: not recorded.
- Year: 2023.
- URL: <https://marabos.nl/atomics/>, which redirects. Read: <http://mara.nl/atomics/>.
- Kind: book.
- Tier: 3.
- Read status: partial. The table of contents and chapters 1, 2, 3, and 6 were read. Chapters 4, 5, and 7 to 10 were not.
- Current: yes. Later additions such as `LazyLock`, `OnceLock`, and a `const` `Mutex::new` simplify parts of its lazy initialization advice.
- Relevance: high. The crates implement `Send` and `Sync` for foreign handles, may share compiler and device objects, and must reason about thread safety in a COM-style API.
- Consult when: writing `unsafe impl Send` or `Sync`, choosing an `Ordering`, building a reference-counted handle, or reviewing a hand-written lock.
- Chapters: `basics.html`, `atomics.html`, `memory-ordering.html`, and `building-arc.html`.
- Cited by: CONC-2, CONC-3, CONC-4, CONC-5, FFI-9, TEST-7, UNS-2, UNS-4, UNS-6. Section 17.

**[IR-28] Compile-Time Invariants in Rust**

- Author: not recorded.
- Year: 2023.
- URL: <https://corrode.dev/blog/compile-time-invariants/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. Non-empty and validated collection types suit safe wrappers, such as a non-empty list of entry points.
- Consult when: designing a type that must never be empty, or deciding whether a macro check or a type carries an invariant.
- Cited by: API-15, TYPE-1, TYPE-3.

**[IR-29] Aim For Immutability in Rust**

- Author: not recorded.
- Year: 2023.
- URL: <https://corrode.dev/blog/immutability/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: low. Mostly basic, with a few useful points on derived state.
- Consult when: reviewing a struct that stores cached or derived values, or code with heavy use of `mut`.
- Cited by: TYPE-3.

**[IR-30] Naming Your Lifetimes**

- Author: not recorded.
- Year: 2021.
- URL: <https://www.possiblerust.com/pattern/naming-your-lifetimes>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. Wrappers around COM-style objects have borrow relationships, such as session, module, and entry point, where descriptive names help.
- Consult when: a type or function has two or more borrow sources, or a long-lived owner hands out borrows.
- Cited by: OWN-3. Section 17.

**[IR-31] Aiming for idiomatic Rust**

- Author: Shane Osbourne.
- Year: 2021.
- URL: <https://web.archive.org/web/20221203043933/https://shane-o.dev/blog/aiming-for-idiomatic-rust>. The curated list gives this archive address.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes, for its narrow topic.
- Relevance: low. It solves one puzzle with match guards and has no library, API, FFI, or unsafe content.
- Consult when: never for this project, except in a discussion of match guards or recursion against an explicit stack.
- Cited by: no rule.

**[IR-32] Wrapping errors in Rust**

- Author: not recorded.
- Year: 2021.
- URL: <https://edgl.dev/blog/wrapping-errors-in-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. It is a short worked example of an error enum that wraps its sources.
- Consult when: adding an error variant that wraps a lower-level error, or explaining why a library does not return `anyhow::Error`.
- Cited by: ERR-2. Section 17.

**[IR-33] Hexagonal architecture in Rust**

- Author: Alexis Lozano.
- Year: 2021.
- URL: <https://alexis-lozano.com/blog/hexagonal-architecture-in-rust-1/>.
- Kind: article, the first of a seven-part series.
- Tier: 3.
- Read status: partial. Parts 1, 2, and 4 were read. Parts 3, 5, 6, and 7 were not.
- Current: mostly. Its error handling uses `()` errors and ad hoc enums, and its repository uses `&mut dyn Repository` as a stated simplification.
- Relevance: low. It describes application service architecture. Only the newtype validation and the trait that decouples a backend transfer.
- Consult when: hiding a backend, such as a GPU device, behind a trait so that higher layers can be tested without it, or writing validated newtypes.
- Cited by: no rule.

**[IR-34] Context-preserving error handling**

- Author: not recorded.
- Year: 2020.
- URL: <https://kazlauskas.me/entries/errors>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: high. One error variant per fallible call site, without a blanket `From`, suits an FFI layer where the same result code arises from many calls.
- Consult when: deciding whether error variants follow call sites or causes, and whether to derive `From`.
- Cited by: ERR-3, ERR-4. Section 17.

**[IR-35] Refactoring Rust Transpiled from C**

- Author: not recorded.
- Year: 2020.
- URL: <https://immunant.com/blog/2020/09/transpiled_c_safety/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. It predates `&raw`, later improvements to `MaybeUninit`, and `unsafe extern`.
- Relevance: low. The project writes new bindings, but its rules on replacing pointer-and-length pairs with slices carry over.
- Consult when: wrapping a C API that hands out pointer-and-length pairs or views into an owned buffer, or reviewing unsafe code that imitates C pointer arithmetic.
- Cited by: FFI-5.

**[IR-36] Learning Rust through open source and live code reviews**

- Author: loige.
- Year: 2020.
- URL: <https://loige.co/learning-rust-through-open-source-and-live-code-reviews/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. The notes give no reason.
- Relevance: low to medium. It reviews one small crate at beginner level, and a few items are useful as checklist reminders.
- Consult when: choosing module layout, exposing a library and a binary from one crate, or implementing `FromStr`.
- Cited by: API-10, TEST-6. Section 17.

**[IR-37] Guide on how to write documentation for a Rust crate**

- Author: Guillaume Gomez, a rustdoc maintainer.
- Year: 2020.
- URL: <https://blog.guillaume-gomez.fr/articles/2020-03-12+Guide+on+how+to+write+documentation+for+a+Rust+crate>.
- Kind: article.
- Tier: 3.
- Read status: full. It was cross-checked against the rustdoc book pages on writing documentation, documentation tests, and lints.
- Current: mostly. Its claim that `missing_doc_code_examples` can be enabled with `#![warn(...)]` is stale, because the lint is now `rustdoc::missing_doc_code_examples` and available only on nightly.
- Relevance: high. Its rules map onto rustdoc lints.
- Consult when: writing crate-level documentation, documentation of public items, or doctests, or choosing documentation lints.
- Cited by: no rule. Section 18.

**[IR-38] Are out parameters idiomatic in Rust?**

- Author: not recorded.
- Year: 2020.
- URL: <https://steveklabnik.com/writing/are-out-parameters-idiomatic-in-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes. Its claims about return value optimization are hedged and depend on the ABI.
- Relevance: medium. The FFI layer is full of C out-parameters, and this article states the rule for the safe layer.
- Consult when: wrapping a C function that fills an out-pointer, or deciding whether a safe API takes a `&mut` buffer.
- Cited by: API-14.

**[IR-39] Await a minute**

- Author: dtolnay.
- Year: 2019.
- URL: <https://docs.rs/dtolnay/0.0.3/dtolnay/macro._01__await_a_minute.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Async and await are stable since Rust 1.39, and its comparison with futures 0.1 combinators is historical.
- Relevance: low. It motivates `.await` and `?` over combinators and is not about library error design.
- Consult when: someone proposes combinator chains on futures, or code in the style of futures 0.1.
- Cited by: no rule. Section 18.

**[IR-40] Taking string arguments in Rust**

- Author: not recorded.
- Year: 2019.
- URL: <http://xion.io/post/code/rust-string-args.html>.
- Kind: article.
- Tier: 3.
- Read status: full. Read on a retry. The page as served ends after the section "Own it" and a footnote on `AsRef<Path>`. A follow-on section on `impl Into<String>` was not served, so that common practice is not backed by this source.
- Current: mostly. `impl AsRef<str>` is now the common spelling, and the example code has typos.
- Relevance: medium. It governs how the safe wrappers take strings and paths for Slang names, entry points, and file paths.
- Consult when: designing a public function that takes a string or path, or choosing between `&str`, `String`, `impl AsRef<str>`, and `impl Into<String>`.
- Cited by: API-13, OWN-1. Section 17.

**[IR-41] Rust Patterns: Enums Instead Of Booleans**

- Author: not recorded.
- Year: 2019.
- URL: <https://blakesmith.me/2019/05/07/rust-patterns-enums-instead-of-booleans.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. Builder and descriptor APIs have many flag-like parameters.
- Consult when: adding a `bool` parameter or field to a public API.
- Cited by: TYPE-2, TYPE-7.

**[IR-42] Programming an ARM microcontroller in Rust at four different levels of abstraction**

- Author: not recorded.
- Year: 2018.
- URL: <https://pramode.in/2018/02/20/programming-a-microcontroller-in-rust-at-four-levels-of-abstraction/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. The specific crate `stm32f30x-hal` is long outdated, but the layering it shows is still the model.
- Relevance: medium. It layers an unsafe raw layer, a generated API, a typestate layer, and a board crate, which matches the shape of a `-sys` crate, a safe crate, and a high-level crate.
- Consult when: justifying the crate layering, or designing typestate for GPU resource states.
- Cited by: TYPE-4.

**[IR-43] Iteration patterns for Result & Option**

- Author: not recorded.
- Year: 2017.
- URL: <http://xion.io/post/code/rust-iter-patterns.html>.
- Kind: article.
- Tier: 3.
- Read status: full. Read on a retry.
- Current: mostly. All three idioms are valid. Clippy now flags the `Result::ok` filter caveat, and `partition_map` is in itertools, not the standard library.
- Relevance: medium. It covers collecting many fallible FFI results into one `Result`.
- Consult when: turning many `Result` or `Option` values into one, or splitting successes from failures.
- Cited by: OWN-7.

**[IR-44] Lessons learned redesigning and refactoring a Rust Library**

- Author: not recorded.
- Year: 2017.
- URL: <https://web.archive.org/web/20220126172949/https://blog.mgattozzi.dev/refactor-rust/>. Read: <https://web.archive.org/web/20220126172949id_/https://blog.mgattozzi.dev/refactor-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full. Read on a retry, through the raw archive copy.
- Current: partly. Its builder, macro hygiene, and caller-first design advice stand. Its use of error-chain, `tokio-core`, an `Rc<RefCell<..>>` client, and the claim that `TryFrom` is nightly-only are outdated.
- Relevance: medium. Builder chains whose types limit the next call, and hidden internals, apply to the high-level crate.
- Consult when: designing a chained builder where each step limits the next calls, or deciding where a builder reports errors.
- Cited by: API-6, SIMP-3. Sections 17 and 18.

**[IR-45] Math with distances in Rust: safety and correctness across units**

- Author: not recorded.
- Year: 2017.
- URL: <https://www.ferrisellis.com/content/rust-implementing-units-for-types/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. It was written for Rust 1.15, its code does not compile as printed, and associated constants are stable since 1.20.
- Relevance: medium. It applies to typed GPU quantities such as byte offsets, sizes, and extents.
- Consult when: introducing a typed quantity, such as a byte size against an element count, or a handle typed with `PhantomData`.
- Cited by: TYPE-8.

**[IR-46] The balance between cost, useability and soundness in C bindings, and Rust-SDL2's release**

- Author: not recorded.
- Year: 2017.
- URL: <https://web.archive.org/web/20190509123207/https://cobrand.github.io/rust/sdl2/2017/05/07/the-balance-between-soundness-cost-useability.html>. The original address is <https://cobrand.github.io/rust/sdl2/2017/05/07/the-balance-between-soundness-cost-useability.html>.
- Kind: article.
- Tier: 3.
- Read status: full. Read through the archive copy.
- Current: mostly. The design lessons hold. It predates `std::thread::scope`, `&raw`, and the edition 2024 unsafe rules. Its `Rc` flag technique is to be avoided, and its first guard-based design was withdrawn by its own update.
- Relevance: high. It addresses the same problem as Slang: a native object graph with destroy-order constraints, wrapped soundly and without runtime cost.
- Consult when: designing parent and child handles, choosing between `Rc` or `Arc` liveness tracking and lifetimes, splitting a large wrapper object, or being tempted to rely on a guard's `Drop` for soundness.
- Cited by: OWN-4, UNS-4. Sections 17 and 18.

**[IR-47] Russian Dolls and clean Rust code**

- Author: mgattozzi, as the notes record it.
- Year: 2016.
- URL: <https://web.archive.org/web/20220126183049/https://blog.mgattozzi.dev/russian-dolls/>. Read: <https://web.archive.org/web/20220126183049id_/https://blog.mgattozzi.dev/russian-dolls/>.
- Kind: article.
- Tier: 3.
- Read status: full. Read on a retry, through the raw archive copy.
- Current: mostly. The combinator advice is current, but the example uses 2016 syntax, and `let`-`else` and `?` on `Option` now cover many cases.
- Relevance: low. It gives combinator advice for `Option`.
- Consult when: a nested `match` on `Option` goes three levels deep.
- Cited by: no rule.

**[IR-48] Elegant Library APIs in Rust**

- Author: Pascal Hertleif.
- Year: 2016, updated in 2017 and 2020.
- URL: <https://deterministic.space/elegant-apis-in-rust.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. `TryFrom` and `impl Trait` are now stable, `lazy_static!` is replaced by `LazyLock` and `OnceLock`, `Box<Error>` is now `Box<dyn Error>`, and its crate suggestions from the `failure` era are gone.
- Relevance: medium. [IR-8] now covers most of it, but its sections on session types and extension traits help the high-level crate.
- Consult when: designing conversion-friendly parameters, a typestate builder, extension traits, or lazy APIs based on iterators.
- Cited by: API-13, TYPE-2. Sections 17 and 18.

**[IR-49] Teaching libraries through good documentation**

- Author: Pascal Hertleif.
- Year: 2016.
- URL: <https://deterministic.space/teaching-libraries.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Rustdoc has since gained intra-doc links and `cfg(doctest)`, and most of its tooling wishlist is done.
- Relevance: medium. It separates guides from reference documentation, which a large API needs.
- Consult when: planning a user guide, the `examples/` directory, or integration-style documentation for a workspace.
- Cited by: TEST-5.

**[IR-50] Pretty State Machine Patterns in Rust**

- Author: not recorded.
- Year: 2016.
- URL: <https://hoverbear.org/2016/10/12/rust-state-machine-pattern/>, which returns only a redirect notice. Read: <https://hoverbear.org/blog/rust-state-machine-pattern/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Its code is in the 2015 edition, and transitions through `From` cannot return `Result` or take arguments, so consuming methods are preferred.
- Relevance: high. Typestate is a core tool for GPU resource lifecycles.
- Consult when: designing a lifecycle API where operations are valid only in some states.
- Cited by: TYPE-4.

**[IR-51] Ripgrep Code Review**

- Author: mbrt.
- Year: 2016.
- URL: <https://blog.mbrt.dev/posts/ripgrep/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: partly. Ripgrep has since been split into `grep-*` crates, and the `lazy_static`, `quick-error`, and `crossbeam` queue details are obsolete.
- Relevance: medium. Its lessons on splitting a workspace and shaping APIs transfer.
- Consult when: deciding crate boundaries, designing builders and one-shot types, returning partial failures, or reusing buffers.
- Cited by: SIMP-4. Section 18.

**[IR-52] Rustic Bits**

- Author: not recorded.
- Year: 2016.
- URL: <https://llogiq.github.io/2016/02/11/rustic.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Its note on compiler plugins is outdated, one snippet is not valid syntax today, and its `Result<(), ()>` advice conflicts with Clippy.
- Relevance: low. Clippy or [IR-8] now covers almost all of its tips.
- Consult when: needing the `as_`, `to_`, and `into_` naming table, or a reminder of the `Entry` API.
- Cited by: no rule. Section 18.

**[IR-53] Convenient and idiomatic conversions in Rust**

- Author: not recorded.
- Year: 2016.
- URL: <https://ricardomartins.cc/2016/08/03/convenient_and_idiomatic_conversions_in_rust>, which redirects. Read: <https://ricardomartins.dev/2016/08/03/convenient_and_idiomatic_conversions_in_rust>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. `TryFrom` was unstable then and is stable since Rust 1.34, and its associated type is now `Error`.
- Relevance: high. Conversion traits appear throughout wrapper types and FFI enum mapping.
- Consult when: deciding which conversion trait to implement, or mapping enums to and from C integers.
- Cited by: API-4, TYPE-6. Section 18.

**[IR-54] Idiomatic tree and graph like structures in Rust**

- Author: not recorded.
- Year: 2016.
- URL: <https://rust-leipzig.github.io/architecture/2016/12/20/idiomatic-trees-in-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Its arena never frees or reuses slots, and a plain `usize` index has no protection against stale identifiers. The notes add generational index crates from outside the source, to be confirmed before use.
- Relevance: high. Scene graphs, reflection trees, module graphs, and resource dependency graphs all need a plan that avoids `Rc<RefCell<..>>` and lifetimes.
- Consult when: modelling a tree or graph with parent, sibling, or cyclic links, or choosing between `Rc<RefCell<..>>`, indexes, and an arena crate.
- Cited by: OWN-5. Section 17.

**[IR-55] Rust traits for developer friendly libraries**

- Author: not recorded.
- Year: 2015.
- URL: <https://benashford.github.io/blog/2015/05/24/rust-traits-for-developer-friendly-libraries/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. The principle holds. The Elasticsearch details and the custom macro are obsolete.
- Relevance: low. [IR-48], [IR-8], and [IR-68] state the same ideas better.
- Consult when: wanting a worked example of `Into<String>` parameters and `From` shorthands for a typed query language.
- Cited by: no rule. Section 17.

**[IR-56] Error Handling in Rust**

- Author: BurntSushi, as the notes record it.
- Year: 2015, updated in 2020.
- URL: <https://burntsushi.net/rust-error-handling/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Its examples use the Rust 1.0 beta, `try!`, `Box<Error>` without `dyn`, and `description` and `cause`.
- Relevance: high. Its advice for library writers is the classic statement of library error design.
- Consult when: designing the top-level error type, choosing between `Box<dyn Error>` and a custom type, or deciding when `unwrap` is acceptable.
- Cited by: ERR-1, ERR-2, ERR-3, ERR-5. Sections 17 and 18.

**[IR-57] Creating a Rust function that accepts String or &str**

- Author: not recorded.
- Year: 2015.
- URL: <https://hermanradtke.com/2015/05/06/creating-a-rust-function-that-accepts-string-or-str.html/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Calling `as_ref()` on a `String` can be ambiguous today, so `&name` or `as_str()` is preferred. It does not mention a thin generic wrapper over a non-generic inner function.
- Relevance: medium. Constructors in safe wrappers often store an owned `String`.
- Consult when: designing a constructor or setter that stores a string, or choosing between `&str`, `String`, `impl Into<String>`, and `impl AsRef<str>`.
- Cited by: OWN-1. Section 17.

**[IR-58] Creating a Rust function that returns a &str or String**

- Author: not recorded.
- Year: 2015.
- URL: <https://hermanradtke.com/2015/05/29/creating-a-rust-function-that-returns-string-or-str.html/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. Helpers that sanitize or escape names, paths, and shader source often return their input unchanged.
- Consult when: writing a function that sometimes transforms its input and sometimes returns it as is, or building a string of known size.
- Cited by: OWN-2.

**[IR-59] Effectively Using Iterators In Rust**

- Author: not recorded.
- Year: 2015.
- URL: <https://hermanradtke.com/2015/06/22/effectively-using-iterators-in-rust.html/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Arrays have implemented `IntoIterator` by value since Rust 1.53, which changes its array examples.
- Relevance: medium. The notes give no reason.
- Consult when: choosing between `iter()`, `iter_mut()`, and `into_iter()`, or removing a `clone()` that only feeds an iterator.
- Cited by: no rule.

**[IR-60] Strategies for solving 'cannot move out of' borrowing errors in Rust**

- Author: not recorded.
- Year: 2015.
- URL: <https://hermanradtke.com/2015/06/09/strategies-for-solving-cannot-move-out-of-borrowing-errors-in-rust.html/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Non-lexical lifetimes make its opening example and the strategy of wrapping code in braces largely obsolete.
- Relevance: low. The notes give no reason.
- Consult when: a borrow really outlives its last use, such as a borrow stored in a struct, a guard, or a closure capture.
- Cited by: OWN-6. Section 18.

**[IR-61] Rayon: data parallelism in Rust**

- Author: not recorded.
- Year: 2015.
- URL: <https://smallcultfollowing.com/babysteps/blog/2015/12/18/rayon-data-parallelism-in-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. The design reasoning holds. The code is from before Rust 1.0, Rayon's internals have changed, and `crossbeam` scoped threads are replaced by `std::thread::scope`.
- Relevance: low to medium. It helps decide when to parallelize shader compilation, and why `Send` and `Sync` bounds on closures make that safe.
- Consult when: adding data parallelism, or designing a closure-taking API whose `Send` and `Sync` bounds carry safety.
- Cited by: UNS-2. Section 18.

**[IR-62] Tricks of the Trait: Enabling Ergonomic Extractors**

- Author: Rob Ede, the speaker.
- Year: 2023.
- URL: <https://www.youtube.com/watch?v=7DOYtnCXucw>.
- Kind: talk.
- Tier: 3.
- Read status: unreachable. The video page gave only navigation, and no slides or transcript were found.
- Current: unknown. The talk was not read.
- Relevance: medium. Extractor-style traits fit callback and pipeline registration APIs in the high-level crate.
- Consult when: designing a registration API where function arguments are derived from types. Until the talk is read, read the `FromRequest` documentation of actix-web and axum instead.
- Cited by: no rule.

**[IR-63] Nine Rules for Elegant Rust Library APIs**

- Author: Carl M. Kadie.
- Year: 2022, with an update in 2023.
- URL: <https://www.youtube.com/watch?v=6-8-9ZV-2WQ>. Read: the companion article at <https://web.archive.org/web/2id_/https://medium.com/data-science/nine-rules-for-elegant-rust-library-apis-9b986a465247>.
- Kind: talk, with a companion article.
- Tier: 3.
- Read status: full for the companion article, read on a retry. The video was not watched.
- Current: mostly. `derive_builder` and `thiserror` remain popular. The text numbers its rules inconsistently.
- Relevance: high. It is a direct checklist for the public API of a library with a high-level crate on top.
- Consult when: designing a public type, builder, or error type, writing examples and API tests, or choosing between a builder and a struct of options.
- Cited by: API-6, API-13, SIMP-3, TEST-1. Sections 17 and 18.

**[IR-64] Ergonomic APIs for hard problems**

- Author: Raph Levien.
- Year: 2022.
- URL: <https://www.youtube.com/watch?v=Phk0C-kLlho>. Read: the abstract at <https://rustlab.it/talks/keynote-ergonomic-apis-for-hard-problems> and the related post <https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html>.
- Kind: talk.
- Tier: 3.
- Read status: partial. Only the abstract of the talk was read. Its five techniques were not seen, and the notes draw on the related post, which is not a record of the talk.
- Current: unknown. Only the abstract and a related post were read.
- Relevance: medium. Static typing with associated types and selective type erasure applies to the high-level crate and the Slint integration.
- Consult when: designing a retained or declarative API over GPU or UI state, or choosing between static types and `dyn` erasure.
- Cited by: no rule.

**[IR-65] Macros for a More Productive Rust**

- Author: jam1garner, the speaker.
- Year: 2020.
- URL: <https://www.youtube.com/watch?v=dZiWkbnaQe8>. Read: the abstract at <https://av.tib.eu/media/52202>.
- Kind: talk.
- Tier: 3.
- Read status: unreachable. Only the abstract was readable, and no slides, transcript, or write-up were found.
- Current: unknown.
- Relevance: low. No rules can be drawn from the abstract.
- Consult when: never as a primary source. Use [IR-15] and the macros chapter of [IR-25] instead.
- Cited by: no rule.

**[IR-66] Making Rust Delightful**

- Author: Nick Cameron.
- Year: 2019.
- URL: <https://www.youtube.com/watch?v=YSEx8wtlPWc>. Read: the slides at <https://raw.githubusercontent.com/nrc/talks/master/rustcon-asia-19/making-rust-delightful.pdf>.
- Kind: talk.
- Tier: 3.
- Read status: partial. Slide titles, headings, and code only, with no narration, so the reasoning behind each slide is inferred.
- Current: mostly. Its examples, `?`, non-lexical lifetimes, and `.await`, have all shipped.
- Relevance: low to medium. It is a philosophy of language ergonomics whose heuristics fit API design.
- Consult when: weighing whether a public API is ergonomic, such as defaults against explicitness.
- Cited by: no rule.

**[IR-67] Idiomatic Rust - Writing Concise and Elegant Rust Code**

- Author: Matthias Endler.
- Year: 2018.
- URL: <https://www.youtube.com/watch?v=P2mooqNMxMs>. Read: the slides at <https://fosdem.org/2018/schedule/event/rust_idiomatic/attachments/slides/2555/export/events/attachments/rust_idiomatic/slides/2555/Idiomatic_Rust___Matthias_Endler___FOSDEM_2018.pdf>.
- Kind: talk.
- Tier: 3.
- Read status: partial. About 20 text slides were readable. Some slides are images, and no transcript was available.
- Current: partly. Its tooling advice is outdated because it recommends the `failure` crate, while its general direction holds.
- Relevance: low. It is an introduction by the author of the curated list.
- Consult when: never for rules. Only as history of the curated list.
- Cited by: no rule. Section 18.

**[IR-68] Idiomatic Rust Libraries**

- Author: Pascal Hertleif.
- Year: 2017.
- URL: <https://www.youtube.com/watch?v=0zOg8_B71gE>. Read: the slides at <https://killercup.github.io/rustfest-idiomatic-libs/index.html>.
- Kind: talk.
- Tier: 3.
- Read status: full for the slides. The video was not viewed.
- Current: mostly. `TryFrom` is now stable, and its advice to use `#![deny(warnings)]` is now discouraged.
- Relevance: medium. It condenses [IR-48] and [IR-8].
- Consult when: needing a quick checklist for a new library crate.
- Cited by: TYPE-2. Sections 17 and 18.

**[IR-69] Preferred way of passing `Path`-like types around?**

- Author: not recorded.
- Year: 2020 in the curated list. The notes date the post to 2019.
- URL: <https://www.reddit.com/r/rust/comments/cekeq9/preferred_way_of_passing_pathlike_types_around/>. Read: <https://web.archive.org/web/2020/https://www.reddit.com/r/rust/comments/cekeq9/preferred_way_of_passing_pathlike_types_around/>.
- Kind: forum.
- Tier: 3.
- Read status: partial. Read on a retry. The original post and five replies in the archive copy were read, of 12 comments.
- Current: yes.
- Relevance: medium. It settles how FFI wrappers take file paths.
- Consult when: writing a public function that takes a path, or deciding whether a generic is worth the monomorphization cost.
- Cited by: API-13. Section 17.

**[IR-70] Which is more idiomatic? Functional, imperative or a mix?**

- Author: not recorded.
- Year: 2017.
- URL: <https://users.rust-lang.org/t/which-is-more-idiomatic-functional-imperative-or-a-mix/11278>.
- Kind: forum.
- Tier: 3.
- Read status: full.
- Current: mostly. The notes give no reason.
- Relevance: low. It is a style discussion with no firm convention, and its claims about speed conflict.
- Consult when: a reviewer or agent argues about iterator chains against loops.
- Cited by: no rule.

**[IR-71] An idiomatic way to sum up values in a multidimensional Array**

- Author: not recorded.
- Year: 2017.
- URL: <https://users.rust-lang.org/t/an-idiomatic-way-to-sum-up-values-in-a-multidimensional-array/9485>.
- Kind: forum.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: low. It holds one idea.
- Consult when: writing a function that is generic over any nested collection.
- Cited by: no rule.

**[IR-72] The Four Horsemen of Bad Rust Code**

- Author: not recorded.
- Year: 2024.
- URL: <https://github.com/corrode/four-horsemen-talk>, which links only to the slides and video. Read: <https://speakerdeck.com/mre/the-4-horsemen-of-bad-rust-code>.
- Kind: talk.
- Tier: 3.
- Read status: partial. The slide text as summarized. No video or transcript.
- Current: yes.
- Relevance: medium. It is a checklist of anti-patterns for review.
- Consult when: reviewing agent-written code for excessive abstraction, premature optimization, or missing tests and documentation.
- Cited by: SIMP-1, SIMP-4.

**[IR-73] Idiomatic Rust Workshop**

- Author: the owner of fettblog.eu and oida.dev, under the handle @ddprrt. The notes give no other name.
- Year: 2021.
- URL: <https://fettblog.eu/slides/idiomatic-rust/>. Read: the slides at <https://oida.dev/wp-content/slides/idiomatic-rust_compressed.pdf>.
- Kind: workshop.
- Tier: 3.
- Read status: full. About 80 slides, with some code slides truncated in extraction.
- Current: mostly. The notes give no reason.
- Relevance: medium. It is a clear checklist on derives, conversions, error handling, extension traits, builders, and typestate.
- Consult when: deciding which standard traits to implement, `From` against `Into`, consuming against non-consuming builders, or sketching typestate.
- Cited by: no rule. Section 17.

**[IR-74] Rust Number Conversion: Don't Follow the Book...**

- Author: not recorded.
- Year: 2021.
- URL: <https://blog.notmet.net/2021/12/rust-number-conversion-dont-follow-the-book.../>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Importing `TryInto` is unnecessary since edition 2021.
- Relevance: high. FFI code converts between `usize`, `u32`, `i32`, and C types constantly.
- Consult when: converting integers across the FFI boundary or between `usize` and fixed-width types.
- Cited by: TYPE-5. Sections 17 and 18.

**[IR-75] Practical guide to Error Handling in Rust**

- Author: not recorded.
- Year: 2024.
- URL: <https://dev-state.com/posts/error_handling/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: low. It is a general overview with no depth specific to libraries.
- Consult when: needing a quick refresher on `thiserror` against `anyhow`, or on `ok_or_else`.
- Cited by: no rule.

**[IR-76] Three Kinds Of Unwrap**

- Author: not recorded.
- Year: 2024.
- URL: <https://zkrising.com/writing/three-unwraps/>. Read: <https://web.archive.org/web/2id_/https://zkrising.com/writing/three-unwraps/>.
- Kind: article.
- Tier: 3.
- Read status: full. Read on a retry, through the archive copy. The original returned almost nothing.
- Current: yes. The `.todo()` and `.unreachable()` methods it proposes are only a request for comments, not part of the standard library.
- Relevance: medium. It gives a vocabulary for where `unwrap` and `expect` are acceptable in library code.
- Consult when: writing or reviewing an `unwrap()` or `expect()`, or setting the panic policy of the workspace.
- Cited by: ERR-6.

**[IR-77] Why choose async/await over threads?**

- Author: not recorded.
- Year: 2024.
- URL: <https://notgull.net/why-not-threads/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. It argues for async from the viewpoint of servers, not libraries.
- Relevance: medium. Async matters here for shader compilation jobs, readback, and Slint integration rather than for I/O.
- Consult when: deciding whether a public API is async, or whether to offer a blocking API and let callers choose.
- Cited by: CONC-6. Section 17.

**[IR-78] Async Rust can be a pleasure to work with (without Send + Sync + 'static)**

- Author: not recorded.
- Year: 2024.
- URL: <https://emschwartz.me/async-rust-can-be-a-pleasure-to-work-with-without-send-sync-static/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. It treats async closures as future work, and they are now stable. Check the state of return type notation before relying on its limits.
- Relevance: medium. It bears on whether wrapper types and library futures are `Send`.
- Consult when: an async public API fails with "future is not Send", or deciding whether a trait requires `Send + 'static`.
- Cited by: CONC-6. Section 17.

**[IR-79] When should I use String vs &str?**

- Author: not recorded.
- Year: 2024.
- URL: <https://steveklabnik.com/writing/when-should-i-use-string-vs-str/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. It sets the baseline for string types in public signatures.
- Consult when: choosing parameter, return, and field types for strings, or deciding whether a struct holds a borrowed string.
- Cited by: OWN-1, OWN-2.

**[IR-80] Don't Worry About Lifetimes**

- Author: not recorded.
- Year: 2024.
- URL: <https://corrode.dev/blog/lifetimes/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: high. It is the best short statement of when not to add lifetimes to public types.
- Consult when: about to add a lifetime parameter to a public struct, or when the compiler suggests lifetime annotations that spread through the API.
- Cited by: OWN-3, OWN-5. Section 17.

**[IR-81] How to Build Abstractions in Rust Applications**

- Author: not recorded.
- Year: 2024 in the curated list. The notes and the address date it to 2023.
- URL: <https://orodu.net/2023/12/27/rust-abstractions.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. It argues for generics over trait objects, which bears on how the high-level crate abstracts over backends and test doubles.
- Consult when: choosing between generic bounds and `Box<dyn Trait>`, or designing a seam for tests.
- Cited by: API-11, API-12, SIMP-1, SIMP-2. Section 17.

**[IR-82] The Ultimate Guide to Rust Newtypes**

- Author: not recorded.
- Year: 2024.
- URL: <https://www.howtocodeit.com/articles/ultimate-guide-rust-newtypes>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: high. Newtypes for handles, formats, sizes, and validated names are central to the safe wrapper layer.
- Consult when: wrapping a raw FFI value or primitive, choosing the traits of a newtype, or deciding on `Deref` or `Borrow`.
- Cited by: API-5, API-7, TYPE-1, TYPE-8.

**[IR-83] Rust Iterators Beyond the Basics**

- Author: JetBrains.
- Year: 2024.
- URL: <https://blog.jetbrains.com/rust/2024/03/12/rust-iterators-beyond-the-basics-part-i-building-blocks/>. Parts II and III have the same address with the endings `-part-ii-key-aspects/` and `-part-iii-tips-and-tricks/` in place of `-part-i-building-blocks/`.
- Kind: article, in three parts.
- Tier: 3.
- Read status: full. Read on a retry, all three parts.
- Current: yes.
- Relevance: medium. It covers APIs that return iterators and the choice between `impl Iterator` and `Box<dyn Iterator>`.
- Consult when: writing a function that returns an iterator, choosing between a loop and a long chain, or hunting a bug caused by laziness.
- Cited by: API-2, OWN-7. Section 17.

**[IR-84] Rust's Sneaky Deadlock With if let Blocks**

- Author: not recorded.
- Year: 2024.
- URL: <https://brooksblog.bearblog.dev/rusts-sneaky-deadlock-with-if-let-blocks/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Edition 2024 fixes the `if let ... else` case, but the same trap remains for `match` and `while let`. The article does not mention editions.
- Relevance: medium. The lesson on guard lifetimes still applies to `match`.
- Consult when: holding a `Mutex` or `RwLock` guard in a condition, or reviewing code that reads and then writes the same lock.
- Cited by: CONC-2, TEST-6. Section 18.

**[IR-85] The Mediocre Programmer's Guide to Rust**

- Author: not recorded.
- Year: 2024.
- URL: <https://www.hezmatt.org/~mpalmer/blog/2024/05/01/the-mediocre-programmers-guide-to-rust.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. It describes a pragmatic application style, not a library style.
- Relevance: low. It trades API quality for speed of writing.
- Consult when: debating whether to clone or borrow in application code that is not hot.
- Cited by: no rule.

**[IR-86] Designing Error Types in Rust Libraries**

- Author: not recorded.
- Year: 2025.
- URL: <https://d34dl0ck.me/rust-bites-designing-error-types-in-rust-libraries/index.html>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: high. It is the most direct treatment of library error types and the pattern of `io::Error` with a kind.
- Consult when: choosing between an error enum and an opaque struct with `kind()`, or deciding whether a third-party error may appear in a public variant.
- Cited by: DEP-3, ERR-1, ERR-2. Section 17.

**[IR-87] To panic or not to panic**

- Author: not recorded.
- Year: 2025.
- URL: <https://www.ncameron.org/blog/to-panic-or-not-to-panic/>.
- Kind: article.
- Tier: 3.
- Read status: full. One rule about libraries was inferred by the summarizing tool.
- Current: yes.
- Relevance: high. It covers unwinding across `extern "C"`, `panic=abort`, and library code that must not panic on caller input.
- Consult when: deciding whether a precondition is a panic or an `Err`, writing callbacks invoked from C++, or reviewing `unwrap` or indexing in library code.
- Cited by: ERR-5, FFI-7, TYPE-9.

**[IR-88] Cancelling async Rust**

- Author: not recorded.
- Year: 2025.
- URL: <https://sunshowers.io/posts/cancelling-async-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. It applies when the library exposes async APIs, and it is the main source on documenting cancel safety.
- Consult when: designing a public `async fn`, using `select!` or `try_join!`, or holding a guard across `.await`.
- Cited by: API-16, CONC-7. Section 17.

**[IR-89] Prototyping in Rust**

- Author: not recorded.
- Year: 2025.
- URL: <https://corrode.dev/blog/prototyping/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: medium. Much of it is for throwaway code, but its rules on placeholders and assertions map onto lint policy.
- Consult when: deciding which lints are off in prototypes and on in committed library code, or using `todo!`, `dbg!`, or `unwrap`.
- Cited by: ERR-6.

**[IR-90] Patterns for Defensive Programming in Rust**

- Author: not recorded.
- Year: 2025.
- URL: <https://corrode.dev/blog/defensive-programming/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: high. It feeds the lint configuration directly.
- Consult when: setting lints, writing trait implementations by hand, adding struct fields, or designing constructors or public structs.
- Cited by: API-4, API-9, TYPE-2, TYPE-7, TYPE-9. Section 17.

**[IR-91] Flattening Rust's Learning Curve**

- Author: not recorded.
- Year: 2025.
- URL: <https://corrode.dev/blog/flattening-rusts-learning-curve/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: low. It is aimed at learners. Its one transferable point is enabling pedantic Clippy early.
- Consult when: writing guidance on how to respond to borrow checker errors.
- Cited by: no rule.

**[IR-92] Pitfalls of Safe Rust**

- Author: not recorded.
- Year: 2025.
- URL: <https://corrode.dev/blog/pitfalls-of-safe-rust/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Its lint list contains names that no longer exist or are mismatched: `unchecked_duration_subtraction` is now `unchecked_time_subtraction`, and `transmute_int_to_char` and `transmute_int_to_float` do not exist in clippy 0.1.99.
- Relevance: high. Arithmetic, casts, indexing, `Default`, redacted `Debug`, and untrusted input all apply to FFI-adjacent code.
- Consult when: writing numeric code, parsing external data, or deriving `Debug`, `Default`, or `Serialize` on configuration or secret types.
- Cited by: API-5, TYPE-3, TYPE-5, TYPE-9. Sections 17 and 18.

**[IR-93] Be Simple**

- Author: not recorded.
- Year: 2025.
- URL: <https://corrode.dev/blog/simple/>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes.
- Relevance: high. It counterweighs agents that over-generalize a high-level API.
- Consult when: about to add a generic, a trait, or a layer of abstraction, or designing the common path of the convenience crate.
- Cited by: API-2, SIMP-1, SIMP-2, SIMP-3, SIMP-4, TYPE-3. Section 17.

**[IR-94] Level Up your Rust pattern matching**

- Author: not recorded.
- Year: 2025.
- URL: <https://blog.cuongle.dev/p/level-up-your-rust-pattern-matching>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: yes. It does not cover `if let` chains, stable in edition 2024 since Rust 1.88.
- Relevance: medium. General idioms, of which enums and exhaustiveness matter for FFI result and flag enums.
- Consult when: writing complex matches, or choosing between `match`, `if let`, `matches!`, and `let`-`else`.
- Cited by: no rule.

### 3.2. Supplementary Sources

**[S-1] The Rustonomicon**

- Author: not recorded.
- Year: a living book. Its FFI page is updated for edition 2024.
- URL: <https://doc.rust-lang.org/nomicon/>.
- Kind: book.
- Tier: 1, named in Section 1.3 of the standards document.
- Read status: partial. The pages below were read in full, and `obrm.html` is only a stub. The pages on data representation, uninitialized memory, constructors and destructors, borrow splitting, and atomics were not read.
- Current: mostly. Its unwinding page is older than its FFI page and is outdated on panics, so trust the FFI page and [S-5]. Its statement that extern types are unstable dates from 2021 and needs a re-check.
- Relevance: high. It explains why the rules on opaque types, `Send` and `Sync`, `PhantomData`, variance, and panics at the boundary exist.
- Consult when: designing an owning handle over a foreign object, deciding `Send` and `Sync`, writing a callback trampoline, or using `PhantomData` for a child object tied to a lifetime.
- Chapters: `ffi.html`, `phantom-data.html`, `send-and-sync.html`, `leaking.html`, `working-with-unsafe.html`, `unwinding.html`, `subtyping.html`, `dropck.html`, and `obrm.html`.
- Cited by: FFI-3, FFI-4, FFI-6, FFI-7, FFI-8, FFI-9, OWN-4, UNS-2, UNS-3, UNS-4, UNS-7. Sections 1.3, 17, and 18.

**[S-2] The bindgen User Guide**

- Author: not recorded.
- Year: a living guide.
- URL: <https://rust-lang.github.io/rust-bindgen/>. The notes also use the `Builder` reference at <https://docs.rs/bindgen/latest/bindgen/struct.Builder.html>.
- Kind: book.
- Tier: 1, assigned here as a Rust project publication for `bindgen`.
- Read status: partial. The pages below were read. The pages on the command line, replacing types, unions, bitfields, flexible array members, `pkg-config`, and most tutorial steps were not.
- Current: yes. The guide has no page on vtables, virtual methods, enum styles, or the `links` key. Only the `Builder` reference covers `vtable_generation`, which it calls experimental, `default_enum_style`, and `layout_tests`.
- Relevance: high. The raw bindings are generated from the Slang headers.
- Consult when: configuring `bindgen`, deciding what to allowlist or make opaque, or meeting a C++ construct that `bindgen` cannot translate.
- Chapters: `library-usage.html`, `tutorial-3.html`, `tutorial-5.html`, `non-system-libraries.html`, `requirements.html`, `allowlisting.html`, `blocklisting.html`, `opaque.html`, `nocopy.html`, `must-use-types.html`, `visibility.html`, `cpp.html`, and `faq.html`.
- Cited by: FFI-10. Sections 17 and 18.

**[S-3] The Cargo Book: Build Scripts**

- Author: not recorded.
- Year: a living book, checked in 2026.
- URL: <https://doc.rust-lang.org/cargo/reference/build-scripts.html> and <https://doc.rust-lang.org/cargo/reference/build-script-examples.html>. The manifest page was read for the `links` key.
- Kind: book.
- Tier: 1, named in Section 1.3 of the standards document.
- Read status: full for both pages.
- Current: yes. It uses the `cargo::` instruction syntax, which needs Cargo 1.77. The older `cargo:` syntax that [S-2] and [S-4] show still works.
- Relevance: high. It defines `links`, `DEP_*` metadata, rerun rules, and the contract of a `-sys` crate.
- Consult when: writing or reviewing a build script, setting `links`, passing include paths to a dependent crate, or letting users override the native library.
- Cited by: FFI-1, FFI-2, SYS-1, SYS-3, SYS-5, TEST-8. Sections 1.3, 17, and 18.

**[S-4] Using C libraries in Rust: make a sys crate**

- Author: Kornel.
- Year: about 2018 to 2019. The page is undated.
- URL: <https://kornel.ski/rust-sys-crate>.
- Kind: article.
- Tier: 3.
- Read status: full.
- Current: mostly. Its static linking defaults and `cargo:` syntax are older, and [S-3] and [S-10] supersede some of it.
- Relevance: high. It is the practical recipe for a `-sys` crate: building, linking, features, and versioning.
- Consult when: designing the features and environment variables for static and dynamic Slang, deciding on vendoring, or handling version skew.
- Cited by: DEP-1, FFI-1, FFI-10, SYS-5, SYS-7, TEST-8, TYPE-6. Sections 17 and 18.

**[S-5] The Rust Reference**

- Author: not recorded.
- Year: a living document, read in 2026.
- URL: <https://doc.rust-lang.org/reference/>.
- Kind: book.
- Tier: 1, named in Section 1.3 of the standards document.
- Read status: full for the four pages below.
- Current: yes.
- Relevance: high. It states exactly which invalid values, aliasing, and ABI mismatches are undefined behaviour, and the edition 2024 form of extern blocks.
- Consult when: deciding whether a foreign pointer may become a reference, whether a result-code enum is valid, what `unsafe extern` and `safe fn` mean, or what happens when a panic or C++ exception crosses a boundary.
- Chapters: `behavior-considered-undefined.html`, `unsafety.html`, `items/external-blocks.html`, and `items/functions.html`, which holds the unwinding table.
- Cited by: FFI-3, FFI-5, FFI-7, UNS-7. Sections 1.3, 17, and 18.

**[S-6] Standard library documentation: keyword unsafe, and the Unsafe Code Guidelines glossary**

- Author: not recorded.
- Year: the keyword documentation is from 2026, and the glossary from 2019 onward.
- URL: <https://doc.rust-lang.org/std/keyword.unsafe.html> and <https://rust-lang.github.io/unsafe-code-guidelines/>.
- Kind: documentation.
- Tier: 1, named in Section 1.3 of the standards document.
- Read status: partial. The keyword page and the glossary were read in full, together with the documentation of `CStr` and `NonNull`. The data layout and validity chapters of the guidelines were not read.
- Current: yes. The guidelines describe themselves as largely abandoned and non-normative, but the glossary is maintained, and the layout chapters may be stale.
- Relevance: high. It defines how to document unsafe contracts and the vocabulary used in review: soundness, validity and safety invariants, and provenance.
- Consult when: writing a `# Safety` section or a safety comment, or deciding whether an API is sound or merely valid.
- Chapters: `glossary.html` of the guidelines.
- Cited by: FFI-4, FFI-6, UNS-3, UNS-6, UNS-7. Section 1.3.

**[S-7] The Edition Guide: Rust 2024**

- Author: not recorded.
- Year: 2025, with Rust 1.85.
- URL: <https://doc.rust-lang.org/edition-guide/rust-2024/index.html>.
- Kind: book.
- Tier: 1, named in Section 1.3 of the standards document.
- Read status: partial. Thirteen chapters were read. The chapters on boxed slices, `gen`, reserved syntax, rustdoc, rustfmt, and several Cargo and rustc changes were not, and the page on unsafe environment functions returned 404.
- Current: yes.
- Relevance: high. The workspace uses edition 2024, so every change applies by default.
- Consult when: writing extern blocks, `#[no_mangle]` or `export_name` items, `-> impl Trait` that returns borrows, `if let` over a lock guard, or `static mut`.
- Chapters: `unsafe-extern.html`, `unsafe-attributes.html`, `unsafe-op-in-unsafe-fn.html`, `static-mut-references.html`, `rpit-lifetime-capture.html`, `temporary-if-let-scope.html`, `temporary-tail-expr-scope.html`, `newly-unsafe-functions.html`, `never-type-fallback.html`, `cargo-resolver.html`, `prelude.html`, and `macro-fragment-specifiers.html`.
- Cited by: CONC-2, CONC-4, FFI-3. Sections 1.3 and 18.

**[S-8] Miri**

- Author: not recorded.
- Year: maintained, read in 2026.
- URL: <https://github.com/rust-lang/miri>.
- Kind: project.
- Tier: 1, assigned here as Rust project documentation for Miri.
- Read status: full for the README.
- Current: yes.
- Relevance: medium. It checks the safe wrapper's own unsafe code, but it cannot run the native Slang library.
- Consult when: deciding which tests run under Miri, setting up its CI job, or testing the wrapper without the native library.
- Chapters: the README sections on foreign functions, what Miri cannot do, the CI example, and nextest.
- Cited by: TEST-7.

**[S-9] The Rust Performance Book: Build Configuration**

- Author: not recorded.
- Year: a living book.
- URL: <https://nnethercote.github.io/perf-book/build-configuration.html>.
- Kind: book.
- Tier: 3.
- Read status: partial. The build configuration page only. The linting page was not read.
- Current: yes.
- Relevance: low. Profile settings apply only at the root of a binary, so a library cannot impose them on its users.
- Consult when: configuring the workspace profiles for an application crate or benchmarks.
- Chapters: the section on panic behaviour.
- Cited by: FFI-7.

**[S-10] Pragmatic Rust Guidelines**

- Author: Microsoft.
- Year: 2026, version 2026.6, generated on 2026-10-26.
- URL: <https://microsoft.github.io/rust-guidelines/>.
- Kind: documentation.
- Tier: 2, named in Section 1.3 of the standards document.
- Read status: partial. Every chapter below was read, and the fetch tool paraphrased the checklist. The FAQ, the change log, and the single-file agent version were not read.
- Current: yes.
- Relevance: high. It is a recent organizational standard with a chapter on AI, and it contains the guidelines M-SYS-CRATES, M-ESCAPE-HATCHES, M-UNSAFE, and M-FFI-NAMING.
- Consult when: deciding the crate split and names, the public API of the safe wrapper (builders, errors, `Clone`, `Send`), documentation sections, lint configuration, or API design for AI agents.
- Chapters: about, checklist, universal, `libs/interop`, `libs/ux`, `libs/resilience`, `libs/building`, macros, apps, ffi, correctness, performance, project, docs, ai, and agents.
- Cited by: API-4, API-6, API-7, API-10, CONC-1, CONC-4, DEP-1, DEP-2, DEP-3, DEP-4, ERR-1, ERR-2, ERR-5, FFI-1, FFI-7, FFI-9, OWN-5, SYS-7, TEST-6, TEST-7, TEST-9, TYPE-1, UNS-1, UNS-3. Sections 1.3 and 17.

## 4. Coverage Gaps

This section lists topics that the project needs and that the sources in Section 3 do not cover, or cover weakly. For each gap it states what kind of source would fill it. Until such a source is added (Section 6.1), the rules for the topic in `docs/rust-coding-standards.md` rest on project decisions and on the neighbouring sources named here.

### 4.1. COM Interfaces and QueryInterface

No source covers COM-style interfaces: owning interface pointers that pair `AddRef` with `Release`, borrowed interface pointers, `QueryInterface` with UUIDs, or `Send` and `Sync` for reference-counted foreign handles. The notes say that the Rustonomicon's chapter on ownership-based resource management [S-1] is only a stub, and that the `windows` and `com` crates would teach an owning COM pointer design. Section 2.23 lists the neighbouring sources. COM-5 of the standards document cites the issue `[SLANG-RS-26]` on a third-party binding as evidence of the parent-lifetime problem, and Section 21.2 of that document lists the issue. A source that fills this gap is the documentation and source of a maintained Rust crate that binds COM interfaces, together with Slang's own documentation of its interface conventions.

### 4.2. Vtable Generation

The `bindgen` guide [S-2] has no page on vtables or virtual methods. Only its `Builder` reference documents `vtable_generation`, and calls it experimental. No source describes how a generator emits `#[repr(C)]` vtable structs, UUID constants, and thunks from C++ headers, which COM-9 relies on. A source that fills this gap is the reference documentation of the header parser that the binding generator uses, or a published binding generator for a COM-style C++ API.

### 4.3. Mapping Result Codes to Errors

Only [IR-5] treats error codes at the FFI boundary, and its examples cover outbound FFI, from Rust to C. [S-10] touches the topic through canonical error structs. The mapping of Slang result codes, and of `HRESULT`-style codes in general, to a Rust error type is the notes' own inference. COM-7 and COM-8 rest on project decisions. A source that fills this gap is Slang's documentation of its result codes and diagnostics, or a maintained binding crate that maps such codes.

### 4.4. Panics Across FFI in Edition 2024 Forms

[S-5], [S-1], and [S-7] together cover panics at the boundary, but weakly. The unwinding page of [S-1] is outdated, [IR-13] still calls unwinding across `extern "C"` undefined behaviour, and [S-7] does not cover `extern "C-unwind"`. The notes state that since Rust 1.81 a panic that escapes `extern "C"` aborts, but no single source read in full describes the current behaviour with edition 2024 syntax. A source that fills this gap is official Rust documentation of the `C-unwind` ABI and of the abort behaviour.

### 4.5. wgpu-Specific Patterns

No source covers wgpu, and [IR-1] does not list it. DEP-3 and SIMP-2 rest on project decisions. A source that fills this gap is the wgpu project's own documentation and examples.

### 4.6. GPU Resource Lifetime Design

No source covers the lifetimes of GPU resources, such as devices, queues, buffers, and pipelines. [IR-46] shows parent and child handles with a native destroy order, [IR-50] and [IR-42] show typestate for resource states, and [IR-54] shows arenas for graphs. The notes point to vulkano references in the curated list, but the curated list contains none. A source that fills this gap is the design documentation of an established Rust GPU library, such as vulkano or wgpu.

### 4.7. Slint Integration

No source covers Slint, or rendering a custom pipeline inside a Slint view. The notes mention Slint only as a context for other topics. A source that fills this gap is Slint's own documentation on integrating custom rendering.

### 4.8. Slices From Foreign Memory

FFI-5 builds slices from pointers and lengths that Slang returns. [S-5] implies the rules through its list of invalid values, and [IR-35] treats pointer-and-length pairs, but the documentation of `std::slice::from_raw_parts` was not read. A source that fills this gap is that standard library documentation.

## 5. Sources That Could Not Be Fully Read

Most sources were read through a summarizing fetch tool, which returns a summary made by a small model rather than the page text. A read status of "full" therefore means that the whole page was read in summary form, not verbatim. The rules in `docs/rust-coding-standards.md` paraphrase their sources, and some lint names in the sources were found to be stale. An agent opens the source itself before relying on fine detail, such as an exact API name, a quoted rule, or a lint name.

This table lists every entry whose read status is partial or unreachable, with what was read or tried and the rules that cite it, so that an agent knows which rules rest on weak evidence.

| Source | Status | What was read or tried | Cited by |
| --- | --- | --- | --- |
| [IR-0] | Partial | README and master list. The essays were not opened. | No rule |
| [IR-3] | Partial | Eight sections of a single large page. | API-9, API-12, CONC-4, DEP-2, TYPE-5, UNS-3, UNS-5 |
| [IR-7] | Partial | Eight of about ten posts, two in detail. | API-11, API-13, FFI-1 |
| [IR-9] | Partial | The index and six chapter pages. | No rule |
| [IR-10] | Partial | Table of contents and about 30 chapters. Concurrency and bare metal not read. | 11 rules |
| [IR-11] | Partial | Landing page and chapter list. | No rule |
| [IR-12] | Partial | README only. | No rule |
| [IR-13] | Partial | Index and ten modules. | 9 rules |
| [IR-14] | Partial | README only. | No rule |
| [IR-15] | Partial | README only. The project specifications and tests were not read. | No rule |
| [IR-16] | Partial | README only. | No rule |
| [IR-18] | Partial | Landing page only. No module list was exposed. | No rule |
| [IR-19] | Partial | Landing page only. | No rule |
| [IR-20] | Partial | README only. The book text is paid. | No rule |
| [IR-21] | Partial | Landing page only. | No rule |
| [IR-22] | Partial | Print page index and four sections. Several chapters are unfinished stubs. | TYPE-5 |
| [IR-23] | Partial | About 25 of 120 pages. | OWN-7, TYPE-4 |
| [IR-24] | Partial | Index and four pages. | No rule |
| [IR-25] | Partial | Table of contents only. The book is paid. | No rule |
| [IR-26] | Partial | The publisher's page returned HTTP 403. An archive copy gave the table of contents only. | No rule |
| [IR-27] | Partial | Chapters 1, 2, 3, and 6 of 10. | 9 rules |
| [IR-33] | Partial | Parts 1, 2, and 4 of 7. | No rule |
| [IR-62] | Unreachable | The video page gave only navigation. No slides or transcript were found, and a conference summary gave one line. | No rule |
| [IR-64] | Partial | The abstract only, plus a related post by the same author. | No rule |
| [IR-65] | Unreachable | The video page gave no transcript. Only the abstract was readable. | No rule |
| [IR-66] | Partial | Slide text extracted from the PDF, with no narration. | No rule |
| [IR-67] | Partial | About 20 text slides. Image slides and the video were not read. | No rule |
| [IR-69] | Partial | An archive copy with 5 of 12 comments. Reddit blocks direct fetches. | API-13 |
| [IR-72] | Partial | Slide text as summarized. No video or transcript. | SIMP-1, SIMP-4 |
| [S-1] | Partial | Nine pages, one of them a stub. Several chapters were not read. | 11 rules |
| [S-2] | Partial | Thirteen pages and the `Builder` reference. Several pages were not read. | FFI-10 |
| [S-6] | Partial | The keyword page and the glossary. The layout and validity chapters were not read. | FFI-4, FFI-6, UNS-3, UNS-6, UNS-7 |
| [S-7] | Partial | Thirteen chapters. One chapter returned 404. | CONC-2, CONC-4, FFI-3 |
| [S-9] | Partial | The build configuration page only. | FFI-7 |
| [S-10] | Partial | Every chapter. The FAQ, the change log, and the agent version were not read. | 24 rules |

Section 3 gives the full list of rules that cite each source.

Some entries were first unreachable and were read on a later retry, through plain HTTP or a raw archive copy: [IR-40], [IR-43], [IR-44], [IR-47], [IR-63], [IR-76], and [IR-83] are now full, and [IR-26] and [IR-69] are partial. Some entries were read through a substitute for the address in the curated list: [IR-4] through its book, [IR-63] through its companion article, and [IR-66], [IR-67], [IR-68], [IR-72], and [IR-73] through their slides. A talk read only through its slides or article can contain points that the slides or article omit.

## 6. Maintaining the List

### 6.1. Adding a Source

An agent adds a source when it reads one to answer a design question or to support a new rule. It follows these steps:

1. Take the next unused `[S-<n>]` identifier, which is one more than the highest `[S-<n>]` in Section 3.2. Never reuse an identifier, including one of a withdrawn source, and never renumber, because `docs/rust-coding-standards.md`, commits, and reviews cite sources by identifier, and a reused or renumbered identifier makes an old citation point to a different source.
2. Add an entry to Section 3.2 with every field of Section 1.4. Write "not recorded" for a field the agent could not determine, and never fill a field from memory.
3. Record the read status as it was: full only when the whole source was read, partial with what was read, and unreachable with what was tried. A partial or unreachable source also gets a row in Section 5.
4. Add the source to the table of every topic in Section 2 that it covers, in order of usefulness, and add a topic when none fits.
5. When a rule in `docs/rust-coding-standards.md` cites the source, add the rule to the "Cited by" field. Section 21 of that document needs no change, because it resolves every `[S-<n>]` identifier in the catalogue.
6. If the source fills a gap in Section 4, update that gap.
7. Add an entry to the change log.

### 6.2. Refreshing From the Upstream List

The 95 `[IR-<n>]` entries were taken from `resources.json` at commit `0c4ecadfd772c57f2e671b850bb82cd6dc926a49` of the `master` branch of <https://github.com/mre/idiomatic-rust>, the snapshot used on 2026-11-10. An `[IR-<n>]` identifier always means index n in that snapshot, not in the current upstream file, because upstream indexes shift when entries are inserted or removed.

When the upstream list changes, an agent compares the current `resources.json` with this catalogue by URL, and then by title for an entry whose URL changed. It handles each difference this way:

1. An upstream entry that matches an `[IR-<n>]` entry keeps its identifier, whatever its new upstream index. The agent updates a changed URL or year in the entry and records the old value.
2. A new upstream entry gets the next unused `[S-<n>]` identifier, following Section 6.1, and its entry says that it comes from the curated list and from which upstream commit.
3. An upstream entry that was removed keeps its `[IR-<n>]` entry, with a note that upstream removed it and at which commit.
4. The agent records the new upstream commit in this section and in the change log.

This scheme is the simplest one that keeps every existing identifier stable. New identifiers never take the `IR` prefix, so the 95 `IR` identifiers stay a fixed set tied to one commit.

### 6.3. Refreshing an Entry

When an agent reads a source again, such as an entry in Section 5 that later becomes reachable, it updates the read status, the fields that the new reading changes, and the row in Section 5. When a rule in `docs/rust-coding-standards.md` gains, loses, or changes a citation, the agent updates the "Cited by" field of each source concerned and the topics that list the rule.

## Change Log

- 2026-11-10: Initial version.
