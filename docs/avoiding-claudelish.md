# Avoiding Claudelish

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-18 |
| Applies to | Every Markdown document and all API documentation in this repository |

## Abstract

This guide defines Claudelish, the recognizable prose habits of text written by Claude and other large language models, and sets the rules that keep it out of this repository. It catalogues each habit with an example and a plain rewrite, and it marks which rules a tool can check. It specifies the phrase check of `check-prose`, which rejects a list of fixed phrases and six patterns, and it gives a revision procedure for every draft.

## 1. Introduction

Claudelish is the prose of unedited large language model (LLM) output, as Section 2.2 defines. The name extends "Claudish", which two recent sources use for the habits of Claude in particular [LESSWRONG-ACTORS] [WATERLOO-CLAUDISH]. The wider term for unwanted generated text is "slop" [WILLISON-SLOP].

An agent writes nearly all the prose in this repository. A reader who meets the habits of unedited model output stops trusting the text, and then skims it or rechecks its facts. One article makes the same point about generated READMEs, which look finished before anyone has edited them [HACKERNOON-README]. A news report says that maintainers find the review of AI-assisted pull requests costlier than before [REGISTER-REVIEW].

This guide adds to the writing guide and covers the same text, which is Markdown documents and API documentation. Where the two disagree, the writing guide prevails, as Section 4 shows for headings in title case.

The guide judges the text and never its author. A single tell proves nothing about who wrote a text, and Wikipedia's editors say that text with these indicators is not always generated [WIKI-AI]. A reviewer reports the phrase and its rewrite and does not guess at authorship.

Each rule gives the form to write along with the form to avoid, because Anthropic's guidance advises telling Claude what to do instead of what not to do [ANTHROPIC-PROMPTING].

## 2. Requirements Language

### 2.1. Key Words

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.2. Terminology

This guide uses the terms of Section 2.2 of the writing guide, such as author and reviewer, and adds these:

- Claudelish: Prose whose recurring habits of vocabulary, sentence construction, structure, formatting, or tone mark it as unedited output of an LLM.
- tell: One such habit, such as a marker word or a list item that opens with a bold label.
- cluster: Two or more tells in one paragraph. A cluster is a stronger sign than a single tell [WIKI-AI].
- actor: The grammatical subject that performs the action of the verb of a sentence.
- phrase check: The check of fixed phrases and patterns that Section 6 specifies.
- writing guide: The document `docs/writing-documentation.md`.

## 3. Catalogue of Tells

Each subsection states its rules and then gives a table of tells. Every example is an illustration written about this project, and its rewrite shows the plain form. The crates' own documentation, and not a rewrite here, is the record of their behaviour.

The column "Checked by" names the phrase check of Section 6 or says "Judgement".

### 3.1. Vocabulary

CLD-1. An author MUST NOT use the marker words, promotional adjectives, or substitutes for "is" and "has" that the phrase list of Section 6.2 holds. They carry no fact, and a reader who meets them reads the sentence around them as filler. The author writes the concrete thing with a plain verb such as "is", "has", "runs", "links", or "returns".

CLD-2. An author MUST write out in plain words the relation that agent shorthand compresses, such as `gated on`, `load-bearing`, or `byte-identical`, and name both sides of it, because a reader who does not share the shorthand has to guess the relation. Orwell's rule applies: never use a jargon word "if you can think of an everyday English equivalent" [ORWELL].

| Tell | Example | Rewrite | Checked by |
| --- | --- | --- | --- |
| Marker words | `This section delves into the intricate landscape of Slang reflection.` | This section describes how `slang-compiler` reflects the parameters of a shader. | Phrase check |
| Promotional adjectives | `slango offers a powerful, seamless, and blazing fast way to run compute kernels.` | `slango` loads a Slang module and runs its compute kernels with their parameters set by name. | Phrase check for the listed words, judgement for others such as "rich" |
| A substitute for "is" or "has" | `slang-src serves as the single source of truth for the recorded Slang commit.` | `slang-src` records the Slang commit that a vendored build clones. | Phrase check |
| Claudish shorthand | `The merge is gated on CI.` | The agent runs `just merge` only after the CI run of the last commit of the branch concluded `success` with every job succeeded. | Phrase check |
| Claudish shorthand | `This check is load-bearing.` | Without this check, nothing reports a call into Slang that runs outside `abort_on_exception`. | Phrase check |
| Claudish shorthand | `The two snapshots are byte-identical.` | `cmp` reports no difference between the two snapshots. | Phrase check |
| Filler | `It is worth noting that the vendored build simply needs CMake.` | A vendored build needs CMake 3.22 or later. | Phrase check for the listed phrases, judgement for "just" and "easy" |

### 3.2. Sentence Constructions

CLD-3. An author MUST make the actor of each sentence a person, a program, or a component that performs the action, such as the user, the build script, Cargo, or Slang. An abstract thing that lies, promises, or refuses, such as a check that "will not lie to you", hides which component acts and what it does [LESSWRONG-ACTORS].

CLD-4. An author MUST NOT frame a claim as the denial of a claim that nobody made, as in `it is not X, it is Y` or `not only X but also Y`, because the denied claim plants an idea the reader did not hold, and the reader then has to work out which of the two the document asserts. A contrast is allowed where the reader is likely to hold the wrong view, such as the belief that the vendored route falls back to the Vulkan SDK.

CLD-5. An author MUST NOT append a participle phrase that comments on a sentence, such as `, highlighting its focus on safety`, because such a phrase asserts significance with no fact behind it, and a reader cannot check it [WIKI-AI]. The author states a consequence or a reason as a clause of its own, joined with "so" or "because".

CLD-6. An author MUST list exactly the items that exist, because a list padded to three items for rhythm adds a claim that nothing backs, and a list cut to three drops a real item.

CLD-7. An author MUST state what is untested or unknown plainly, and what is known with its source or the condition under which it holds. A hedge such as `based on available information` or `could potentially` leaves the reader unable to tell a tested fact from a guess.

| Tell | Example | Rewrite | Checked by |
| --- | --- | --- | --- |
| Abstract thing as actor | `The build script will not lie to you about which Slang it linked.` | The build script records the build tag of the Slang that it links in `BUILD_INFO`. | Judgement |
| Abstract thing as actor | `A compile error is honest about wgpu being the only backend.` | Without the feature `wgpu`, `slango` stops compiling with an error that names the feature. | Judgement |
| Denial pivot | `The vendored route is not a fallback, it's a separate build.` | The vendored route builds Slang from source. A build never falls back from one route to the other. | Phrase check for two forms, judgement for others |
| Trailing "-ing" analysis | `slango releases every Slang object before a load returns, highlighting its focus on safety.` | `slango` releases every Slang object before a load returns, so no Slang object outlives a load. | Phrase check for the listed participles, judgement for others |
| Reflexive rule of three | `The API is safe, fast, and ergonomic.` | The public types of `slango` that hold a GPU object are neither `Send` nor `Sync`. | Judgement |
| Hedge | `Based on available information, other Slang versions could potentially work.` | The crates are tested with the Slang of one Vulkan SDK version, and another Slang is best effort. | Phrase check for the listed hedges, judgement for others |

### 3.3. Paragraph and Document Structure

CLD-8. An author MUST open each document, section, and paragraph with its content and MUST end it when the content ends. An opening that announces what follows adds reading time and no fact, and a closing sentence that restates the paragraph is a second statement that can diverge from the first (Section 4.5 of the writing guide). Microsoft's style guide gives the same advice, which is to get to the point fast [MS-STYLE].

CLD-9. An author MUST NOT add a closing section of challenges or outlook, and MUST record limits and open questions where the document type puts them, such as the open questions of a design document. A generic closing section holds no requirement and no decision, so an agent cannot act on it [WIKI-AI].

| Tell | Example | Rewrite | Checked by |
| --- | --- | --- | --- |
| Signposting | `Let's dive into how the loader works.` | The loader of `slang-glslang-sys` answers each request for a library whose file name starts with `slang-glslang`. | Phrase check |
| Restating summary | A paragraph about the shared folder that ends `In short, the shared folder keeps vendored builds fast.` | Delete the last sentence. | Phrase check for an opening summary phrase, judgement for others |
| Formulaic closing section | `## Challenges and Future Outlook`, or `Despite these challenges, the crates are ready.` | An open question in the design document, with what decides it. | Phrase check |
| Overlong introduction | A README that opens with three paragraphs on why shaders matter | The README opens with what the project is, as RDM-6 of `docs/writing-readmes.md` requires. | Judgement |

### 3.4. Formatting

CLD-10. An author MUST NOT open an unordered list item with a label in bold. The item is a complete sentence or a `term: definition` entry, as Section 2.2 of the writing guide shows. A bold label repeats the first words of the item, and a reader who skims the labels skips the sentences that hold the facts.

CLD-11. An author SHOULD use bold only for a warning that the reader must not miss, and SHOULD add a heading only over content that a reader would cite on its own. Bold on many phrases hides the one that matters, and a heading over each short paragraph splits one point into fragments that lack context.

| Tell | Example | Rewrite | Checked by |
| --- | --- | --- | --- |
| Bold-label bullet | `- **Fast:** clones Slang once per target directory.` | The build script of `slang-src` clones Slang once per target directory. | Phrase check for unordered lists |
| Too much bold | `The **vendored** build needs **CMake** and **Ninja**.` | The vendored build needs CMake and Ninja. | Judgement |
| A heading on everything | A section with a level-three heading over each one-sentence paragraph | One section with its paragraphs in order | Judgement |
| Leftovers of a chat interface | `contentReference[oaicite:1]`, or a link that ends in `utm_source=chatgpt.com` | Delete the marker, and cite the source by its plain address. | Phrase check |

### 3.5. Tone

CLD-12. An author MUST address the reader of the document in the neutral register of a specification and MUST NOT write to the person who asked for the text. Praise, agreement, offers of more help, and apologies are leftovers of the chat in which the text was written, and they tell the reader that nobody edited it [WIKI-AI] [ANTHROPIC-OPUS4]. Sycophancy is a known behaviour of models trained with human feedback [ANTHROPIC-SYCOPHANCY], and the press has reported it in Claude Code [REGISTER-SYCOPHANCY].

CLD-13. An author MUST state the significance of a change or an item as what a user or an agent can do with it, because an inflated claim of importance gives the reader nothing to check and hides the fact that matters.

| Tell | Example | Rewrite | Checked by |
| --- | --- | --- | --- |
| Sycophantic opener | `Great question! The loader is installed on every global session.` | With its feature `vendored`, `slang-compiler` installs the loader of `slang-glslang-sys` on every global session that it creates. | Phrase check |
| Chat leftover | `I hope this helps! Let me know if you would like more examples.` | Delete the sentences. | Phrase check |
| Apology | `Apologies for the confusion in the earlier section.` | Correct the earlier section. | Phrase check |
| Inflated significance | `This change is a pivotal milestone for the Rust graphics ecosystem.` | After this change, a program can enable the feature `vendored` to build Slang from source without a Vulkan SDK. | Phrase check for the marker word, judgement for others |

### 3.6. Content and Truthfulness

CLD-14. An author MUST give a checkable source for each claim. A claim about the world outside the repository cites a named source. A claim about the repository names the file, the recipe, the measurement, or the section that records it. Vague attribution such as `experts agree` and an unverifiable superlative give the reader nothing to check, and an agent that repeats them spreads an unsupported claim.

CLD-15. An author MUST check every identifier, command, path, feature, version, number, and citation in a draft against the code or the source before the draft is done, and MUST delete what the author cannot check. A plausible name that does not exist becomes a defect when an agent copies it (Section 6.3 of the writing guide), and invented details in generated bug reports cost maintainers time [STENBERG-AI].

| Tell | Example | Rewrite | Checked by |
| --- | --- | --- | --- |
| Vague attribution | `It is widely known that Slang crashes on deeply nested types.` | Slang can crash or abort on a shader whose types nest past its limit of 128 levels, when a program looks up an entry point with a stage or requests its layout, as Section 3.12 of `docs/design/slang-compiler.md` records. | Phrase check for the listed forms, judgement for others |
| Unverifiable superlative | `slang-compiler is the most complete Slang binding for Rust.` | `slang-compiler` covers sessions, modules, linking, code generation, and reflection, and offers no custom file system. | Judgement |
| Invented option | `Set SLANG_SRC_OFFLINE=1 to skip the clone.` | `slang-src` reads no environment variable of its own. A later build in the same target directory reuses the clone without network access. | Judgement |
| Self-certification | `Every entry in this list is verified and accurate.` | Each entry names the smallest known input, the call, and the observed outcome. | Judgement |
| Invented citation or link | A reference to an RFC number or a page that does not exist | A reference whose address the author opened | Judgement, and `rustdoc::broken_intra_doc_links` for links between items in API documentation |

## 4. Tells That the Writing Guide Already Rules

The writing guide already has a rule for each tell in this table, so this guide adds none and cites the section instead.

| Tell | Rule in the writing guide | Checked by |
| --- | --- | --- |
| Em dashes and en dashes | Section 8 | `check-prose` |
| Curly quotes and emoji | Section 8 | `check-prose` |
| Semicolons in prose | Section 8 | `check-prose` |
| Long paragraphs | Section 4.1 | Judgement |
| A list for an argument, or a run of short bullets | Section 4.1 | Judgement |
| Status and diary lines in a design document | Section 3.6 | Judgement |
| Project history in code and API documentation | Section 7.1 | Judgement |
| Metaphor and idiom | Section 6.4 | Judgement |
| A second name for one concept | Section 4.4 | Judgement |
| The same requirement stated twice | Section 4.5 | Judgement |
| "see above" and "as mentioned" | Section 6.1 | Judgement |

RDM-31 of `docs/writing-readmes.md` keeps progress records out of every README.

Wikipedia lists headings in title case as a tell [WIKI-AI]. Section 8 of the writing guide requires title case, so in this repository title case is the rule, and a reviewer does not report it.

## 5. Strength of the Evidence

This section is non-normative. It records how strong the evidence is for each group of tells, so that a reviewer weighs a finding to match.

| Topic | Evidence |
| --- | --- |
| Marker words | Strong. In a study of PubMed abstracts, the excess words of 2024 were mostly style words, with frequency ratios of 28.0 for `delves` and 13.8 for `underscores` [KOBAK-EXCESS]. In peer reviews, `commendable`, `meticulous`, and `intricate` were 9.8, 34.7, and 11.2 times more likely [LIANG-MONITORING]. |
| Single words as proof of origin | Weak. `delve` is ordinary in Nigerian formal English [WILLISON-NIGERIA], and Wikipedia's editors treat only a dense cluster as a strong sign [WIKI-AI]. This guide bans the words because they carry no content, and a reviewer never cites one as evidence of who wrote a text. |
| Em dashes | Weak alone [WIKI-AI]. Section 8 of the writing guide bans them on its own grounds. |
| Differences between models | Each model has a recognizable style, and a classifier told five model families apart with 97.1% accuracy [SUN-IDIOSYNCRASIES]. A list built from one model's habits therefore misses some habits of another. |
| Claudish shorthand and abstract actors | Thin. One forum post and one university article describe them [LESSWRONG-ACTORS] [WATERLOO-CLAUDISH]. |
| README tells | Thin. One article covers generated READMEs [HACKERNOON-README], and no source ties a wall of badges or a `Key Features` list to models beyond Wikipedia's general tells. |
| Lists in Claude's output | Changing. The system prompt of Claude Opus 4 kept lists out of reports and explanations unless asked [ANTHROPIC-OPUS4], and that of Claude Sonnet 5.5 allows them when the content is multifaceted enough [ANTHROPIC-SONNET55]. |

## 6. The Phrase Check

`check-prose` runs the phrase check of this section on every file that it checks, so `just check` and the hooks that run `check-prose` run it too. A reviewer does not report a phrase or a pattern that the check reports, as Hard Rule 4 of `AGENTS.md` requires. For a rule that this guide marks "Phrase check" in part, the check covers only the listed phrases and the patterns of Section 6.3, and the rest of the rule stays a judgement rule.

The check prints each hit as one line in this form, in which the hint names the tell and the plain form to write instead:

```text
path:line: phrase '<text>': <hint>
```

### 6.1. Scope and Matching

The phrase check reads the text that `check-prose` reads under Section 8 of the writing guide, which is every Markdown file and the `///` and `//!` comments of every Rust file, outside fenced code blocks and code spans. It matches as follows:

- It ignores case.
- It matches a fixed phrase only when the character before it and the character after it are not word characters, which are letters, digits, and the underscore. A hyphen is not a word character, so `byte-identical` matches as one phrase and `gated on` does not match inside `investigated on`. A code span separates the words on either side of it.
- It reads each paragraph as one line of text, with every line break and run of whitespace replaced by one space, so that a phrase that wraps in a documentation comment is still found. For a Rust file, it first removes the `///` or `//!` marker and the indentation that all lines of the comment share. A paragraph ends at a blank line, a heading, a code fence, and the start of each list item, and each heading is read as a paragraph of its own.
- It runs the patterns of Section 6.3 on that paragraph text, except the sixth, which runs on each line.
- It skips each list item under a heading whose title, after its section number, is References, at any level, and under every subheading of it, because the title of a cited work keeps its own words, such as `Maintaining Robust Protocols` in the writing guide.

A document that names the phrases, such as this guide, puts each of them in a code span or a fenced code block, where the check does not look.

### 6.2. Fixed Phrases

The file `scripts/prose-phrases.txt` holds the fixed phrases, one on each line, with each apostrophe written as the ASCII apostrophe, because the writing guide bans smart quotes. A line in square brackets holds the hint that the check prints for each phrase below it, up to the next line in square brackets. The file groups the phrases by tell, in the order of this table, and each group follows a subsection of Section 3.

| Group | Section |
| --- | --- |
| Marker words | 3.1 |
| Promotional adjectives | 3.1 |
| Substitutes for "is" and "has" | 3.1 |
| Claudish shorthand | 3.1 |
| Filler | 3.1 |
| Hedges | 3.2 |
| Signposting | 3.3 |
| Formulaic closings | 3.3 |
| Chat leftovers and flattery | 3.5 |
| Vague attribution | 3.6 |
| Leftovers of a chat interface | 3.4 |

An author adds a phrase under the hint of its group. A phrase above the first hint stops the check with exit status 2.

### 6.3. Patterns

`check-prose` also holds six regular expressions, written in the syntax of the Python module `re` and compiled without regard to case. Section 6.1 says which text each one runs on. This table gives what each pattern matches and the tell that it detects, in the order of the script:

| Pattern | Matches | Tell |
| --- | --- | --- |
| First | A comma followed by `highlighting`, `underscoring`, `emphasizing`, `fostering`, `reinforcing`, `solidifying`, or `cementing` | Trailing "-ing" analysis after a comma (CLD-5) |
| Second | `in conclusion`, `in summary`, `in short`, `to summarize`, `to sum up`, or `overall`, followed by a comma, at the start of a paragraph or a sentence | A summary phrase that opens a sentence (CLD-8) |
| Third | `not only` followed by `but also` with no full stop, question mark, or exclamation mark between them | The pivot `not only ... but also` (CLD-4) |
| Fourth | `it is not` or `it's not` followed, with no full stop, question mark, or exclamation mark between them, by a comma and `it is` or `it's` | The pivot `it is not ..., it is` (CLD-4) |
| Fifth | `certainly`, `of course`, `absolutely`, or `sure`, followed directly by an exclamation mark | A chat interjection with an exclamation mark (CLD-12) |
| Sixth | A line that opens an unordered list item with `-`, `*`, or `+` followed by text in bold | An unordered list item that opens with a bold label (CLD-10) |

The first pattern leaves out `showcasing`, because the phrase list holds that word.

### 6.4. False Positives

This table lists where a false positive is likely and how an author resolves it, and it names the words that stay off the list on purpose.

| Phrase or pattern | Likely false positive | Resolution |
| --- | --- | --- |
| `robust`, `robustness` | The title of a cited work, and the technical term for Postel's principle of tolerant input | Reference entries are skipped. In other prose, name the behaviour, such as "accepts malformed input". |
| `serves as` | The sense "is used as", as in a depth texture that is used as a depth target | Write "is used as" or "is". |
| `landscape` | The orientation of an image | Name the size, such as "wider than it is tall". |
| `crucial`, `pivotal`, `testament`, `realm` | A quotation of a source | Paraphrase the quotation. |
| Fourth pattern | A sentence that corrects a view the reader is likely to hold | State the fact in two sentences, as the rewrite of the pivot in Section 3.2 does. |
| Sixth pattern | None expected | The pattern leaves ordered lists out, because the numbered Hard Rules of `AGENTS.md` open with their names in bold. |
| `underscore`, `underscores` | Off the list. The words name the character `_`, which the design documents discuss in identifiers. | The verb stays a judgement rule. |
| `just` | Off the list. It names the task runner, although Google's style guide lists the adverb as a filler word to delete [GOOGLE-WORDS]. | The adverb stays a judgement rule. |
| `easy`, `easily`, `rich` | Off the list, because each has ordinary uses. | Judgement. |
| `reflecting` | Off the list of participles, because Slang reflection is a subject of this repository. | Judgement. |

## 7. Revision Procedure

An author runs these steps on a draft before handing it to review, and a reviewer runs steps 2 to 9 again on every draft that it reviews. Step 1 is a tool check, and the other steps are judgement.

1. Run `check-prose` on each changed file and fix every finding.
2. Read the first sentence of the document and of each section, and delete each one that announces, praises, or restates the title (CLD-8).
3. Read the last sentence of each paragraph, and delete it when it restates the paragraph (CLD-8).
4. Find the actor of each sentence, and replace each abstract actor with the component that acts (CLD-3).
5. For each adjective of quality or size, write the measurement or the property that it stands for, or delete it (CLD-1, CLD-13).
6. Count the items of each list and of each series in a sentence, and keep only the items that exist (CLD-6).
7. Check every identifier, command, path, feature, version, number, and citation, and delete what cannot be checked (CLD-15).
8. Rewrite each list item that opens with a bold label (CLD-10).
9. Where a paragraph still holds a cluster, rewrite it from its facts instead of swapping single words, because a synonym in the same sentence keeps the shape of the tell.

This command runs step 1 on this guide:

```sh
just check-prose docs/avoiding-claudelish.md
```

## 8. Security Considerations

The security consideration of this guide is false content. An author that invents a recipe, an environment variable, a safety condition, or a security property, such as a claim that a build script reaches no network, leads a reader or an agent to rely on behaviour that does not exist. CLD-14 and CLD-15 address this.

The threat model is an author, human or agent, that writes plausible text without checking it, and a reader who trusts the text. No attacker is assumed. A malicious change to a document is out of scope, because review and access control govern it. So is deciding whether a text was machine-written, because Section 5 shows that a single tell does not support that decision.

The residual risk is an invented fact in fluent, plain prose. The phrase check finds fixed phrases and patterns only, and nothing checks a claim in a Markdown document against the code, so such a fact passes every tool and depends on the reviewer.

## 9. References

### 9.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 9.2. Informative References

- [ANTHROPIC-OPUS4] Anthropic, system prompt of Claude Opus 4, release notes, 2025, read 2026-11-17, <https://platform.claude.com/docs/en/release-notes/system-prompts/claude-opus-4>.
- [ANTHROPIC-PROMPTING] Anthropic, "Prompting best practices", read 2026-11-17, <https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices>.
- [ANTHROPIC-SONNET55] Anthropic, system prompt of Claude Sonnet 5.5, release notes, 2026, read 2026-11-17, <https://platform.claude.com/docs/en/release-notes/system-prompts/claude-sonnet-5-5>.
- [ANTHROPIC-SYCOPHANCY] Anthropic, "Towards understanding sycophancy in language models", 2023-10-23, <https://www.anthropic.com/research/towards-understanding-sycophancy-in-language-models>.
- [GOOGLE-WORDS] Google, "Google developer documentation style guide: Word list", read 2026-11-17, <https://developers.google.com/style/word-list>.
- [HACKERNOON-README] HackerNoon, article on treating generated READMEs as first drafts, 2026-09-09, <https://hackernoon.com/treat-ai-generated-readmes-like-first-drafts>.
- [KOBAK-EXCESS] Kobak et al., "Delving into ChatGPT usage in academic writing through excess vocabulary", arXiv:2406.07016, 2024-06-11, published in Science Advances on 2025-07-02, <https://arxiv.org/abs/2406.07016>.
- [LESSWRONG-ACTORS] LessWrong, post on why Claude makes abstract things into actors, 2026-08-28, <https://www.lesswrong.com/posts/odDt3qkqnbc4r2fu6/why-does-claude-seem-to-make-abstract-things-into-actors>.
- [LIANG-MONITORING] Liang et al., "Monitoring AI-Modified Content at Scale", arXiv:2403.07183, 2024-03-11, <https://arxiv.org/abs/2403.07183>.
- [MS-STYLE] Microsoft, "Microsoft Writing Style Guide: Brand voice", read 2026-11-17, <https://learn.microsoft.com/en-us/style-guide/brand-voice-above-all-simple-human>.
- [ORWELL] Orwell, G., "Politics and the English Language", 1946, <https://www.orwellfoundation.com/the-orwell-foundation/orwell/essays-and-other-works/politics-and-the-english-language/>.
- [REGISTER-REVIEW] The Register, article on a switch for pull requests written with AI on GitHub, 2026-02-03, <https://www.theregister.com/2026/02/03/github_kill_switch_pull_requests_ai>.
- [REGISTER-SYCOPHANCY] The Register, article on the flattery of Claude Code, 2025-08-13, <https://www.theregister.com/2025/08/13/claude_codes_copious_coddling_confounds/>.
- [STENBERG-AI] Stenberg, D., "The I in LLM stands for intelligence", 2024-01-02, <https://daniel.haxx.se/blog/2024/01/02/the-i-in-llm-stands-for-intelligence/>.
- [SUN-IDIOSYNCRASIES] Sun et al., "Idiosyncrasies in Large Language Models", ICML 2025, arXiv:2502.12150, <https://arxiv.org/abs/2502.12150>.
- [WATERLOO-CLAUDISH] University of Waterloo, news interview on Claudish, 2026-09-08, <https://uwaterloo.ca/news/media/q-and-experts-what-claudish>.
- [WIKI-AI] Wikipedia, "Wikipedia:Signs of AI writing", read 2026-11-17, <https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing>.
- [WILLISON-NIGERIA] Willison, S., post on the word "delve", 2024-04-18, <https://simonwillison.net/2024/Apr/18/delve/>.
- [WILLISON-SLOP] Willison, S., post on the term "slop", 2024-05-08, <https://simonwillison.net/2024/May/8/slop/>.

## Appendix A. Requirement Summary

| Label | Requirement | Section | Checked by |
| --- | --- | --- | --- |
| CLD-1 | Use no marker word, promotional adjective, or substitute for "is" or "has" that Section 6.2 lists | 3.1 | Phrase check for the listed words, judgement for others |
| CLD-2 | Write out the relation that agent shorthand compresses | 3.1 | Phrase check for the listed shorthand, judgement for other jargon |
| CLD-3 | Make a person, program, or component the actor of each sentence | 3.2 | Judgement |
| CLD-4 | Do not deny a claim that nobody made | 3.2 | Phrase check for two forms, judgement for others |
| CLD-5 | Append no participle phrase of commentary to a sentence | 3.2 | Phrase check for the listed participles, judgement for others |
| CLD-6 | List exactly the items that exist | 3.2 | Judgement |
| CLD-7 | State what is untested or unknown plainly, and what is known with its source | 3.2 | Phrase check for the listed hedges, judgement for others |
| CLD-8 | Open with the content and end when the content ends | 3.3 | Phrase check for signposts and opening summary phrases, judgement for others |
| CLD-9 | Add no closing section of challenges or outlook, and put limits and open questions where the document type puts them | 3.3 | Phrase check for the listed phrases, judgement for others |
| CLD-10 | Open no unordered list item with a bold label | 3.4 | Phrase check |
| CLD-11 | Use bold and headings sparingly | 3.4 | Judgement |
| CLD-12 | Address the reader of the document, with no flattery, chat leftovers, or apology | 3.5 | Phrase check for the listed phrases, judgement for others |
| CLD-13 | State significance as what a user or an agent can do | 3.5 | Judgement |
| CLD-14 | Give a checkable source for each claim | 3.6 | Phrase check for the listed attributions, judgement for others |
| CLD-15 | Check every identifier, command, path, feature, version, number, and citation, and delete what cannot be checked | 3.6 | Judgement |
