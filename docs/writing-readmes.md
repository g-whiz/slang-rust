# Writing READMEs

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-19 |
| Applies to | The root `README.md` and the `README.md` of every library crate in this repository |

## Abstract

This guide defines what the READMEs of this repository contain, in which order, and how they link and render on GitHub and on crates.io. The root README helps a visitor on GitHub decide whether to use the project and which crate to pick. A crate README helps a Rust developer on crates.io add one crate and take a first step. Each fact that users need has one home, and the READMEs point to it instead of copying it. The guide marks each rule as checked by a tool or by a reviewer.

## 1. Introduction

A README is the first page that a reader of this project sees, on GitHub for the repository and on crates.io for a crate. A reader decides from it within minutes whether the project or crate fits, so a README that buries its purpose or its first step loses the reader.

The published copy of a crate README cannot be corrected without a new release, because Cargo transfers the file to the registry when the crate is published [CARGO-MANIFEST]. Facts that change with the code therefore live in the API documentation, which each release rebuilds, and a README links to them.

This guide governs the content, structure, examples, and rendering of every README. `docs/writing-documentation.md` governs their language and punctuation, as Section 8 of that guide defines. The examples package `slang-rust-examples` is not published and has no README.

## 2. Requirements Language

### 2.1. Key Words

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.2. Terminology

- root README: The file `README.md` at the root of the repository, which GitHub shows as the project page.
- crate README: The file `README.md` in the package directory of a library crate, such as `crates/slango/README.md`, which Cargo packages and crates.io renders as the page of the crate.
- README: A root README or a crate README.
- home: The one place where a fact is stated in full.
- pointer: One sentence that names a fact and links to its home.
- alert: A block quote that starts with a marker such as `> [!WARNING]`, which GitHub and crates.io render as a highlighted box.
- status notice: The alert that states the release status of the project or a crate.
- hosted API documentation: The rustdoc site at `https://g-whiz.github.io/slang-rust/`, which the documentation workflow builds from each release tag. The page of a crate is `https://g-whiz.github.io/slang-rust/<crate_name>/`, with underscores in the crate name.

## 3. Readers and Purpose

A README is a landing page [YEGOR256]. It says what the project or crate is, why a reader would use it, how to take the first step, and where to go next, which are the questions that common guidance on READMEs asks [OSG] [GITHUB-README]. A classification of the content of GitHub READMEs lists the purpose of the project among the common kinds of content [PRANA].

RDM-1. Each README MUST answer, in this order, what its subject is, why a reader would use it, how to take the first step, and where to go next, for the reader of its kind. A README that answers these questions out of order, or for the other kind of reader, sends the reader to the wrong crate or loses the reader before the first step.

Reference material, tutorials, and explanation are distinct kinds of documentation, and Diataxis advises against mixing them [DIATAXIS]. The README therefore links to the API documentation for reference material and to example programs for longer walkthroughs [MAKE-README] [18F].

Checked by: judgement.

## 4. Where Each Fact Lives

### 4.1. Homes

RDM-2. Each fact in Table 1 MUST be stated in full in its home only. Two statements of one fact diverge, and the copy in a published crate README cannot be fixed until the next release.

Table 1. Where each fact lives.

| Fact | Home | Elsewhere |
| --- | --- | --- |
| Alpha status, stability of the API, and how a dependent names the full pre-release version | Section `Status` of the root README | The status notice of each crate README (Section 6.3) and the version in its `Installation` snippet, which is the version of its package. The version in `Getting Started` of the root README, which is the latest published version |
| The two ways to get Slang: an installed Vulkan SDK, or the Cargo feature `vendored` that builds Slang from source | Crate-level documentation of `slang-compiler-sys` | A pointer in `Requirements` of the root README and of each crate README whose crate links Slang |
| What a vendored build needs and costs, and the shared folder in the Cargo target directory | Crate-level documentation of `slang-src` | A pointer from the home of the two ways to get Slang |
| Which crate takes the feature `vendored`, including the need to enable it on both `slango-macros` and `slango` | `# Features` of the crate-level documentation of each crate | A pointer in `Requirements` of the crate README |
| libclang, which every build needs to generate the bindings | Crate-level documentation of `slang-compiler-sys` | A pointer in `Requirements` of the root README and of each crate README whose crate links Slang |
| Minimum supported Rust version (MSRV) | `Requirements` of the root README, matching `rust-version` in the workspace manifest | None |
| Supported operating systems and architectures | `Requirements` of the root README | `# Platform support` of a crate states only what differs for that crate |
| Licence of one crate | `Licence` of its crate README, matching its `license` field | `Licence` of the root README states the licences by kind of crate and links to each crate README |
| The licence consequence of `slang-slint` depending on Slint under the GPL | `Licence` of `crates/slang-slint/README.md` | A pointer in `Licence` of the root README and in the crate-level documentation of `slang-slint` |
| Cargo features, the API, errors and their messages, and environment variables | Crate-level and item documentation of the crate | None |
| How the crates are built and why | Design documents under `docs/design/` | None |
| Changes between releases | The GitHub release of each tag | None |

### 4.2. Pointers

RDM-3. Outside its home, a document MAY state a fact of Table 1 only in the form that the Elsewhere column gives. Where that form is a pointer, the document MUST NOT restate details of the fact, such as a list of tools, a version, a command, a size, a duration, or a condition. A pointer keeps the reader one click from the full fact, while a partial copy reads as complete and goes stale without notice.

This illustration shows a pointer in the `Requirements` section of a crate README:

```markdown
Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes.
```

### 4.3. What Moves Out of a README

RDM-4. A README MUST NOT hold the list of Cargo features, API reference material, error messages or troubleshooting steps, environment variables, or build internals. These belong in the homes of Table 1, where rustdoc checks links and examples and each release rebuilds the page. A copy in a README is checked by nothing [YEGOR256].

Checked by: judgement, for Sections 4.1 to 4.3.

## 5. The Root README

### 5.1. Sections and Order

RDM-5. The root README MUST have the sections of Table 2, in that order, with exactly these headings, and MUST NOT add other sections. Crate READMEs link to these headings by their anchors, and a reader who knows one README of the project finds the same content in the same place in the next one.

Table 2. Sections of the root README.

| Order | Heading | Presence | Content |
| --- | --- | --- | --- |
| 1 | `# slang-rust` | Required | The title, optionally followed by the badge of Section 8.6 |
| 2 | None | Required | The opening paragraph of Section 5.2 |
| 3 | `## Status` | Required | The status notice of Section 5.3 |
| 4 | `## Crates` | Required | The table of Section 5.4 |
| 5 | `## Requirements` | Required | The MSRV, the supported platforms, and pointers to Slang and libclang |
| 6 | `## Getting Started` | Required | The first step of Section 5.5 |
| 7 | `## Examples` | Optional | One lead-in sentence that says to run each command from the repository root, then one `sh` block with a comment line and a command for each example program, as RDM-23 allows |
| 8 | `## Documentation` | Required | Links to the hosted API documentation and, for contributors, to `AGENTS.md` |
| 9 | `## Getting Help` | Required | A link to `https://github.com/g-whiz/slang-rust/issues` |
| 10 | `## Licence` | Required | The licences by kind of crate, with links |

The licence comes last, as Standard Readme requires [STANDARD-README]. A Contributing section is absent while the project has no contribution policy. GitHub builds a table of contents from the headings [GITHUB-README], so the README has no table of contents of its own.

### 5.2. Opening

RDM-6. The first sentence of the root README MUST say what the project is by its category, in at most 120 characters, and the second sentence MUST say what it covers. A visitor reads the first sentence to decide whether to read on, and Standard Readme asks for a short description first [STANDARD-README].

This illustration shows the form of the opening:

```markdown
slang-rust is a set of Rust crates for the Slang shader language. They compile and reflect Slang shaders, run compute kernels and render pipelines on the GPU through wgpu or slang-rhi, and show the result in a Slint user interface.
```

### 5.3. Status

RDM-7. The section `Status` MUST hold one alert of type `WARNING` that states that the crates are alpha releases, that the API can change in any release, and that a dependent names the full pre-release version, such as `"0.1.0-alpha.1"`. Cargo matches a pre-release only from a requirement that names a pre-release, so a dependent who writes `"0.1"` gets a resolution error.

### 5.4. Crates

RDM-8. The section `Crates` MUST hold one table with one row for each library crate and the columns Crate, Use it to, and API documentation. The Crate column links the crate name to its crate README on GitHub, the Use it to column has one sentence, and the API documentation column links to the page of the crate on the hosted API documentation, or, for a crate with `publish = false`, to its source directory on GitHub.

The rows start with the crates that a user depends on directly, which are `slango`, `slango-macros`, `slang-compiler`, `slang-slint`, and `slang-rhi`, followed by the crates they build on, which are `slango-codegen`, `slang-compiler-sys`, `slang-rhi-sys`, `slang-glslang-sys`, `slang-src`, and `slang-bindgen`. A visitor scans the table top down for the crate that fits. The tracing README uses such a table [TRACING].

### 5.5. Getting Started

RDM-9. The section `Getting Started` MUST show, as a command in a `sh` block, how to add the first crate with its full version, and MUST link to the crate README that holds the first example. It MAY show the command that runs one example program from a clone. Guidance on documentation asks that a new user reach a first success quickly [KAPLAN-MOSS] [WTD].

This illustration shows the command:

```sh
cargo add slango@0.1.0-alpha.1
```

Checked by: judgement, for Sections 5.1 to 5.5. A tool could check the headings and their order, but no configured tool does.

## 6. Crate READMEs

### 6.1. Sections and Order

RDM-10. A crate README MUST have the sections of Table 3, in that order, with exactly these headings, and MUST NOT add other sections. A reader who moves between the crates of this project finds the same content under the same heading on each page.

Table 3. Sections of a crate README.

| Order | Heading | Presence | Content |
| --- | --- | --- | --- |
| 1 | `# <crate-name>` | Required | The package name, exactly as in the manifest |
| 2 | None | Required | The opening paragraph of Section 6.2 |
| 3 | None | Required | The status notice of Section 6.3 |
| 4 | `## Installation` | Required | The dependency snippet of Section 6.4, or for `slang-src` the sentences that RDM-13 gives instead |
| 5 | `## Example` | Required, except for `slang-compiler-sys`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-src` | The example of Section 7 |
| 6 | `## Requirements` | Required when the crate needs more than Cargo | Pointers to the native requirements of the crate |
| 7 | `## Documentation` | Required | Links to the page of the crate on the hosted API documentation and to the root README |
| 8 | `## Licence` | Required | The licence of the crate and links to its licence texts |

### 6.2. Opening

RDM-11. The first sentence of a crate README MUST say what the crate is, with the meaning of the `description` field of its manifest, and the second sentence MUST say where the crate fits and which sibling crate a reader uses instead for a different need. A reader who reaches a support crate from a search, such as a `-sys` crate, otherwise depends on the wrong crate.

### 6.3. Status Notice

RDM-12. Directly under the opening, a crate README MUST hold one alert of type `WARNING` of at most two sentences, which says that the crate is an alpha release and links to `https://github.com/g-whiz/slang-rust#status`. While the manifest of the crate sets `publish = false`, the alert also says that the crate is not published on crates.io. A reader on crates.io does not see the root README, so without the notice the reader expects a stable API.

### 6.4. Installation

RDM-13. The section `Installation` MUST hold a `toml` block with the dependency table that the crate is meant for and one line that names the crate with the full version of its manifest. It MAY add a second line that enables the feature `vendored`, with a pointer to its home. A requirement without the full pre-release version fails to resolve, as Section 5.3 explains.

The one exception is `slang-src`, whose section `Installation` holds no snippet. It says, in one or two sentences, that a program never depends on `slang-src` directly and enables the feature `vendored` of the crate that it uses instead, with a link to the home of that feature in Table 1. A snippet would invite a dependency that does nothing, because only the build scripts of other crates read what `slang-src` provides.

### 6.5. Content by Kind of Crate

RDM-14. A crate README MUST meet the row of Table 4 for its kind. Each kind has a different first step and a different sibling that most readers want instead. Native wrappers such as rusqlite also explain how the native library is found [RUSQLITE].

Table 4. Content of a crate README by kind of crate.

| Kind | Crates | The opening says | The example | `Requirements` also points to |
| --- | --- | --- | --- | --- |
| `-sys` crate | `slang-compiler-sys`, `slang-glslang-sys`, `slang-rhi-sys` | That the crate holds raw unsafe bindings or a native build, and that most users depend on `slang-compiler`, or on `slang-rhi` for `slang-rhi-sys`, instead | None | How the crate gets the native library, and the licence of the native code that it builds or links |
| Source crate | `slang-src` | That the crate provides the Slang source tree to the build scripts of other crates, and that a user never depends on it directly | None, because the crate has no API | That its build script clones from the network on the first build in a target directory |
| Build tool | `slang-bindgen` | That the crate generates the bindings from the Slang headers with libclang, and that `slang-compiler-sys` runs it in every build | One call of `generate` | libclang |
| Safe wrapper | `slang-compiler` | That the crate compiles and reflects Slang shaders without a GPU, and that `slango` runs them | Compiling one small shader and printing a result | Slang and libclang |
| High-level library | `slango` | That the crate runs compute kernels and draws with render pipelines on the GPU through wgpu by default or through slang-rhi with its feature `rhi`, with one API | Loading a module and running one kernel | Slang, libclang, a GPU adapter, and the build from source and the platforms of the feature `rhi` |
| Procedural macro | `slango-macros` | That the macro compiles a Slang file while the crate builds and generates typed bindings to its kernels | One call of `shader!` and one call of a generated kernel | Slang at build time, the feature `vendored` on both `slango-macros` and `slango`, and the exact version that ties it to `slango` |
| Generator | `slango-codegen` | That the crate is the generator behind `slango-macros`, and when to use it instead of the macro | One call of the generator | Slang at build time, and the exact version that ties it to `slango` |
| Safe wrapper of slang-rhi | `slang-rhi` | That the crate runs compute kernels and draws with render pipelines on the GPU through slang-rhi, and that most programs use `slango`, which does both through wgpu or, with its feature `rhi`, through this crate | Opening a device and printing a result, with a link to a whole compute dispatch in the crate documentation | The build of Slang and slang-rhi from source, the supported platforms, and what a program needs at run time |
| Slint integration | `slang-slint` | That the crate shows what `slango` draws in a Slint user interface | The smallest wiring of a Slint window to a `slango` render pipeline | What a Linux build of Slint needs, and the licence consequence in its own `Licence` section |

### 6.6. Manifest

RDM-15. The manifest of each library crate MUST set `readme = "README.md"` to its own crate README and MUST NOT inherit the `readme` field of the workspace. Cargo packages the file that the field names and crates.io renders it as the page of the crate [CARGO-MANIFEST], so an inherited root README shows every crate the same project page.

Checked by: `check-package` checks that each packaged crate holds a `README.md` as a regular file. Whether that file is the crate's own README, and the rest of Sections 6.1 to 6.6, are judgement rules.

## 7. Examples

### 7.1. Form

RDM-16. The example of a crate README MUST be one complete program of 5 to 30 lines, counting its `use` declarations and its `main`, that performs the first task a reader of that crate needs. It MUST use `?` and MUST NOT use `unwrap` or `expect`, because readers copy the example into their programs [MAKE-README]. The serde and reqwest READMEs each show a short example near the top [SERDE] [REQWEST].

RDM-17. A README example MUST NOT contain hidden lines, which start with `# `, and MUST show its `use` declarations and a `fn main() -> Result<(), Box<dyn std::error::Error>>` instead. GitHub renders a hidden line as written, so the reader sees `# ` lines that rustdoc would hide. This replaces, for README examples only, the hidden `Ok` line of Section 7.5 of `docs/writing-documentation.md`.

RDM-18. In every other respect, a README example MUST follow Section 7.5 of `docs/writing-documentation.md`, including `no_run` with its reason in the lead-in, never `ignore`, a language tag on every code block, and the `text` block for an example that cannot compile as a doctest. A reader cannot tell a broken example from a correct one, so these rules keep the compiler checking it.

RDM-19. When the example prints, the README SHOULD show what it prints in a `text` block after it, with a lead-in that says so, and the output MUST be what the example prints [MAKE-README]. A reader compares the output with the result on their own machine, so a wrong output reads as a failure.

### 7.2. Doctests

RDM-20. Each crate whose README holds Rust code MUST compile that code as doctests by including the README in `src/lib.rs` in this form only:

```rust
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
```

A README example that no build compiles goes stale with the next change to the API. Rustdoc compiles every Rust block of the included file as a doctest [RUSTDOC] [RUST-1.54], and the item exists only while rustdoc collects doctests.

RDM-21. A crate MUST NOT use its README as its crate-level documentation, such as with `#![doc = include_str!("../README.md")]`. The two pages serve different readers with different link forms: a README needs absolute URLs and visible example lines, while crate-level documentation needs intra-doc links and the sections of Section 7.4 of `docs/writing-documentation.md`. The checkers of that guide also do not read documentation written as a `#[doc]` attribute, so their enforced rules would become judgement rules.

RDM-22. The root README MUST NOT hold a Rust code block, because no crate includes it, so nothing compiles that code. It holds `toml` and `sh` blocks only.

### 7.3. Commands

RDM-23. Every command in a README MUST be exact, in a `sh` block, with a lead-in that says what it does and the directory it runs from. A block of several commands, such as the section `Examples` of the root README, MAY instead have one lead-in that names the directory, with a comment line above each command that says what it does. An author MUST run every command that a change adds or alters before the change is done, because no tool runs README commands and a reader runs them as written.

This illustration shows the form of the section `Examples` of the root README:

````markdown
Run each command from the root of the repository:

```sh
# Compile a Slang file and print its entry points and parameters.
cargo run -p slang-rust-examples --bin compile-and-reflect -- examples/shaders/scale.slang
# Run a compute kernel and check its results.
cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
```
````

Checked by: `cargo test --doc` compiles the Rust code of each included README, and `check-doctest` runs it. `check-doctest-attributes` reports a code block marked `ignore` in every `README.md`. The presence of the include, hidden lines, example length and style, printed output, and commands are judgement rules.

## 8. Links and Rendering

### 8.1. Absolute Links

RDM-24. Every link and every image in a README MUST use an absolute `https` URL, except an in-page anchor that RDM-26 allows. crates.io resolves relative links only in limited cases, and a relative image in a README at the root of a workspace can fail to resolve [CRATESIO-MD] [CARGO-16865]. A link to a file in the repository uses the form `https://github.com/g-whiz/slang-rust/blob/main/<path>`.

A link to `main` shows the current code, which can differ from the released version. A fact that must match a release therefore lives in the hosted API documentation, which each release tag rebuilds, as Section 4.1 states.

### 8.2. API Documentation Links

RDM-25. A README MUST link to the API documentation of a crate on the hosted API documentation and MUST NOT link to docs.rs. docs.rs cannot build these crates, which need libclang and either a Vulkan SDK or network access to clone Slang. A README MAY link to the crates.io page and the hosted API documentation of a crate before the first release publishes them, because the release makes the links resolve without another edit.

### 8.3. In-Page Anchors

RDM-26. A README MAY link to a heading of the same page with `#<anchor>` and to a heading of the root README with `https://github.com/g-whiz/slang-rust#<anchor>`. crates.io rewrites in-page links to match its own heading identifiers, so an in-page anchor works on both sites [CRATESIO-MD]. Tables 2 and 3 fix the headings, so the anchors stay stable.

### 8.4. Alerts

RDM-27. A README MUST use an alert for its status notice and MAY use one more alert, only for a fact whose omission harms the reader, such as the licence consequence in `crates/slang-slint/README.md`. GitHub and crates.io both render alerts [CRATESIO-MD], and a page with many alerts teaches the reader to skip them.

### 8.5. HTML

RDM-28. A README MUST NOT contain raw HTML. crates.io sanitizes HTML and keeps only a short list of elements [CRATESIO-MD], so an element that GitHub shows can disappear from the page of the crate.

### 8.6. Images and Badges

RDM-29. Only the root README MAY hold an image, at most one, which shows the output of an example program, with alternative text that describes what it shows. An image that shows nothing about use adds length [YEGOR256].

RDM-30. Only the root README MAY hold a badge, at most one, which is the status of the CI workflow at `https://github.com/g-whiz/slang-rust/actions/workflows/ci.yml/badge.svg`, directly under the title. A README MUST NOT load a badge from a third-party service. A badge signals project quality to a visitor [TROCKMAN], but crates.io already shows the version and licence of a crate, and a third-party image tells another host who reads the page.

Checked by: judgement, for Sections 8.1 to 8.6. A tool could check absolute URLs, raw HTML, images, badges, and that each link resolves, but no configured tool does.

## 9. What a README Never Contains

RDM-31. A README MUST NOT contain any of these items, because each goes stale with the next change, and a published copy stays stale until the next release:

- Progress records: milestones, what works and what does not as a status, what was tried, completion dates, dated measurements, identifiers of CI runs, and counts of tests.
- A changelog, which belongs in the GitHub release of each tag.
- API reference material, which belongs in the crate documentation.
- Marketing adjectives, such as `blazing`, `powerful`, `seamless`, `simple`, `easy`, `modern`, `robust`, and `production-ready`. A README states a property as a fact the reader can check.
- The path, section number, or rule identifier of a document under `docs/`, which a reader on crates.io cannot open.
- A path from a local machine, a `TODO`, a `FIXME`, or an attribution line.

Checked by: judgement. `check-doc-references` does not read READMEs.

## 10. Language and Maintenance

RDM-32. A README MUST follow Section 8 of `docs/writing-documentation.md` and `docs/avoiding-claudelish.md`, so that every page of the project uses one spelling and ASCII punctuation and none uses the listed phrases.

RDM-33. A change that alters a fact MUST update its home, and every pointer that the change makes false, in the same change, as Hard Rule 1 of `AGENTS.md` requires. A crate README ships inside its package, so each example in it that names a version of the crates MUST name the version of its package, as the API documentation does, and the change that raises that version raises the example. The root README is not packaged, so each example in it MUST name the latest published version, and the change that follows a publication raises it, so that a visitor can install what the README shows.

Checked by: `check-prose` for ASCII punctuation, semicolons, the listed spellings, and the phrase check of Section 6 of `docs/avoiding-claudelish.md` in every Markdown file, and `check-typos` for misspellings. Whether each README names the version that RDM-33 requires is a judgement rule, because no configured tool compares them.

## 11. Security Considerations

This guide defines no component. Its first security consideration is that a README tells readers which commands to run and which dependencies to add, so a wrong command or a dependency line that names the wrong crate leads a reader to run or build code they did not intend. RDM-13, RDM-23, and RDM-24 keep commands exact and links absolute and checkable.

Its second security consideration is that the build script of `slang-src` reaches the network on the first vendored build. Table 4 requires the README of `slang-src` to point to this, so that a reader who builds in an isolated environment knows before the build fails. RDM-30 keeps third-party images, which report each page view to their host, out of every README.

The threat model is an author who writes a wrong or stale instruction. An attacker, and a compromise of GitHub, crates.io, or the hosted documentation, are out of scope, because this guide governs writing and not hosting. The residual risk is a command or a pointer that a reviewer accepts without running or following it, because no configured tool checks links or commands.

## 12. References

### 12.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 12.2. Informative References

- [18F] 18F, "Making READMEs Readable", archived 2018-07-12, <https://github.com/18F/open-source-guide/blob/18f-pages/pages/making-readmes-readable.md>.
- [CARGO-16865] The Cargo Team, "Issue 16865", rust-lang/cargo, opened 2026-04-09, <https://github.com/rust-lang/cargo/issues/16865>.
- [CARGO-MANIFEST] The Cargo Team, "The Manifest Format", The Cargo Book, <https://doc.rust-lang.org/cargo/reference/manifest.html>.
- [CRATESIO-MD] The crates.io Team, "crates_io_markdown", source code of the crates.io Markdown renderer, <https://raw.githubusercontent.com/rust-lang/crates.io/main/crates/crates_io_markdown/src/lib.rs>.
- [DIATAXIS] Procida, D., "Diataxis", <https://diataxis.fr/>.
- [GITHUB-README] GitHub, "About READMEs", GitHub Docs, accessed 2026-11-17, <https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes>.
- [KAPLAN-MOSS] Kaplan-Moss, J., "Writing great documentation", 2009-11-10, <https://jacobian.org/2009/nov/10/what-to-write/>.
- [MAKE-README] "Make a README", <https://www.makeareadme.com/>.
- [OSG] Open Source Guides, "Starting an Open Source Project", <https://opensource.guide/starting-a-project/>.
- [PRANA] Prana et al., "Categorizing the Content of GitHub README Files", 2018, <https://arxiv.org/abs/1802.06997>.
- [REQWEST] McArthur, S., "reqwest", README, <https://github.com/seanmonstar/reqwest>.
- [RUSQLITE] The rusqlite Developers, "rusqlite", README, <https://github.com/rusqlite/rusqlite>.
- [RUST-1.54] The Rust Release Team, "Announcing Rust 1.54.0", 2021-07-29, <https://blog.rust-lang.org/2021/07/29/Rust-1.54.0/>.
- [RUSTDOC] The Rust Project, "The rustdoc book", <https://doc.rust-lang.org/rustdoc/>.
- [SERDE] The Serde Developers, "serde", README, <https://github.com/serde-rs/serde>.
- [STANDARD-README] Littauer, R., "Standard Readme", <https://github.com/RichardLitt/standard-readme>.
- [TRACING] The Tokio Developers, "tracing", README, <https://github.com/tokio-rs/tracing>.
- [TROCKMAN] Trockman, A., et al., "Adding Sparkle to Social Coding", ICSE 2018, <https://cmustrudel.github.io/projects/badges>.
- [WTD] Write the Docs, "A beginner's guide to writing documentation", <https://www.writethedocs.org/guide/writing/beginners-guide-to-docs/>.
- [YEGOR256] Bugayenko, Y., "Elegant README", 2019-04-23, <https://www.yegor256.com/2019/04/23/elegant-readme.html>.

## Appendix A. Requirement Summary

| Label | Section | Requirement | Checked by |
| --- | --- | --- | --- |
| RDM-1 | 3 | What, why, first step, and where next, in order, for one reader | Judgement |
| RDM-2 | 4.1 | Each fact of Table 1 in full in its home only | Judgement |
| RDM-3 | 4.2 | Elsewhere, only the form that Table 1 gives, and a pointer restates no details | Judgement |
| RDM-4 | 4.3 | No feature list, API reference, errors, environment variables, or build internals | Judgement |
| RDM-5 | 5.1 | Root README sections, order, and headings of Table 2 | Judgement |
| RDM-6 | 5.2 | Root opening: category in at most 120 characters, then scope | Judgement |
| RDM-7 | 5.3 | Status alert with the full pre-release version | Judgement |
| RDM-8 | 5.4 | Table of crates with links, in the stated order | Judgement |
| RDM-9 | 5.5 | First step as a command, with a link to the first example | Judgement |
| RDM-10 | 6.1 | Crate README sections, order, and headings of Table 3 | Judgement |
| RDM-11 | 6.2 | Crate opening: what it is, then where it fits | Judgement |
| RDM-12 | 6.3 | Status notice with a link to the root Status | Judgement |
| RDM-13 | 6.4 | Dependency snippet with the full version, except `slang-src`, which says to enable `vendored` on another crate | Judgement |
| RDM-14 | 6.5 | The row of Table 4 for the kind of crate | Judgement |
| RDM-15 | 6.6 | Own `readme` field, not inherited | `check-package` for a packaged `README.md`, judgement for the rest |
| RDM-16 | 7.1 | One complete program of 5 to 30 lines, with `?` and no `unwrap` or `expect` | Judgement |
| RDM-17 | 7.1 | No hidden lines, a visible `fn main` that returns `Result` | Judgement |
| RDM-18 | 7.1 | Section 7.5 of the writing guide otherwise | `cargo test --doc`, `check-doctest-attributes`, judgement for the lead-in |
| RDM-19 | 7.1 | Printed output shown and correct | Judgement |
| RDM-20 | 7.2 | README included as doctests through `ReadmeDoctests` | `cargo test --doc` for the code, judgement for the presence of the include |
| RDM-21 | 7.2 | README not used as crate-level documentation | Judgement |
| RDM-22 | 7.2 | No Rust code block in the root README | Judgement |
| RDM-23 | 7.3 | Exact commands with a lead-in, or one lead-in and a comment line per command, run before the change is done | Judgement |
| RDM-24 | 8.1 | Absolute `https` URLs for links and images, except in-page anchors | Judgement |
| RDM-25 | 8.2 | Hosted API documentation, never docs.rs | Judgement |
| RDM-26 | 8.3 | In-page anchors only to the fixed headings | Judgement |
| RDM-27 | 8.4 | Status alert and at most one more alert | Judgement |
| RDM-28 | 8.5 | No raw HTML | Judgement |
| RDM-29 | 8.6 | At most one image, in the root README | Judgement |
| RDM-30 | 8.6 | At most one badge, the CI status, in the root README | Judgement |
| RDM-31 | 9 | No progress records, changelog, API reference, marketing, document citations, local paths, or attribution | Judgement |
| RDM-32 | 10 | Language of Section 8 of the writing guide and the phrases of `docs/avoiding-claudelish.md` | `check-prose` with its phrase check, `check-typos`, judgement for the rest |
| RDM-33 | 10 | Homes and pointers updated in the same change, and the version that each README names | Judgement |

## Appendix B. Crate README Skeleton

This skeleton shows the form of a crate README with placeholders in angle brackets. It is an illustration, and the placeholders name no real item:

````markdown
# <crate-name>

<What the crate is, in one sentence.> <Where it fits, and which sibling crate to use instead.>

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
<crate-name> = "<full version>"
```

## Example

<One lead-in sentence that says what the example does, and why it is `no_run` when it is.>

```rust,no_run
use <crate_name>::<Item>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    <the steps of the first task>
    Ok(())
}
```

## Requirements

<One pointer for each native requirement.>

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/<crate_name>/) describes every item. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

<The licence of the crate, with links to its licence texts.>
````
