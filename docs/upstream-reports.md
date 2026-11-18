# Upstream Reports

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-18 |
| Applies to | Every defect of an upstream project that this repository records and that should be reported to that project, and every request to an upstream project that the owner decided to make |

## 1. Introduction

This document is the one to-do list of the bug reports that the project should make to upstream projects. An upstream project is a project whose code the crates build on, such as Slang, and a report is an issue in that project's public tracker.

The list exists so that a defect that should be reported is not lost between the change that records it and the owner's decision to report it. Section 2 states the rules for an entry, Section 3 lists the entries, and Section 4 holds the one draft written so far.

## 2. Rules for Entries

An agent that records a defect of an upstream project that should be reported adds its entry to Section 3 in the same change, as Hard Rule 14 of `AGENTS.md` requires. An agent that cannot tell whether a defect should be reported adds the entry with the status "Owner to decide" and says why in one sentence, so that the owner decides instead of the agent.

An entry cites the record of the defect by its identifier and does not repeat its input, outcome, or source location, because a second copy diverges from the first (Hard Rule 1 of `AGENTS.md`). The record of a defect is its row in the list of known upstream bugs for its project, which is Section 3.12 of `docs/design/slang-compiler.md` for Slang and Section 3.1 of `docs/design/slang-rhi.md` for slang-rhi. The identifiers in Section 3 are the identifiers of those rows. A request to an upstream project that is not a defect, such as a request to update a submodule, also gets an entry when the owner decided to make it, with an identifier of the form `REQ-<n>` and the section of the design document that states its facts as its record.

A draft is the one exception, because an issue has to state the facts itself. A draft states what the row and the API documentation of the affected call record as observed, and flags anything that it reconstructs in a note for the owner. A change to the row updates the draft in the same change.

An entry holds the identifier, the upstream project, a one-line title that can serve as the title of the issue, and the status. When its draft is written, the draft is a section of its own in this document, and the status names that section.

Nothing in this document is posted without the owner's explicit go-ahead in the current session, because a report speaks for the project in a public repository (Hard Rule 14 of `AGENTS.md`). When a report is posted, the status of its entry and the column "Report" of its row both record the address of the issue, and the section of its draft is removed, because the issue then holds its text.

An entry stays in the list after its report is posted, and its identifier is never reused.

## 3. Entries

Each identifier is a row of the list that Section 2 names for the entry's upstream project, or a request of Section 2, and the column "Upstream project" names the project's repository on GitHub.

| Identifier | Upstream project | Title | Status |
| --- | --- | --- | --- |
| KB-1 | Slang, `shader-slang/slang` | A C++ exception escapes `findAndCheckEntryPoint` and `getLayout` for a type nested past the limit of 128 levels | Reported, <https://github.com/shader-slang/slang/issues/14448> |
| KB-2 | Slang, `shader-slang/slang` | Reflection functions let a C++ exception escape when a length comes from a generic value parameter | Draft not written. It has the same kind of cause as KB-1, so the owner may prefer to add it to issue 14448 instead of a new issue |
| KB-3 | Slang, `shader-slang/slang` | `spReflection_FindTypeByName` crashes the process for some malformed or deeply nested type names | Draft not written |
| KB-4 | Slang, `shader-slang/slang` | `getEntryPointCode` reads and writes out of bounds for an entry point index past the end | Draft not written |
| KB-5 | Slang, `shader-slang/slang` | A static Slang without `slang-glslang` reports the missing library only in the diagnostics of a successful generation of SPIR-V | Owner to decide. The row records this as a behaviour of Slang rather than a defect, so it may not be worth an issue. Draft not written |
| KB-6 | Slang, `shader-slang/slang` | `spReflectionType_GetElementCount` returns 0 instead of `SLANG_UNBOUNDED_SIZE` for an array without a length | Draft not written |
| KB-7 | Slang, `shader-slang/slang` | `spReflection_getGlobalConstantBufferBinding` returns 0 on SPIR-V when the global constant buffer is at another binding | Draft written in Section 4, not posted |
| RKB-1 | slang-rhi, `shader-slang/slang-rhi` | The range check of `readBuffer` can overflow, so a range past the end of a buffer passes it | Draft not written. The defect was read in the source and is not reproduced yet |
| REQ-1 | Slang, `shader-slang/slang` | Update the submodule `external/metal-cpp` to the metal-cpp for macOS 15.2 that slang-rhi pins, so that slang-rhi builds its Metal backend from the Slang tree | Draft not written. Its record is Section 7.4 of `docs/design/slang-rhi-sys.md` |

The status of an entry is one of these values:

- Draft not written: The defect should be reported or the request made, and no draft exists yet.
- Draft written: The draft is in a section of this document and is not posted.
- Owner to decide: The agent that added the entry could not tell whether the defect should be reported.
- Reported: The issue is posted, and the status gives its address.

## 4. Draft Report for KB-7

Section 4.1 lists what the owner checks before posting, and is not part of the issue. Sections 4.2 to 4.8 are the text of the issue, in its order, and each of their headings becomes a heading of the issue.

### 4.1. Checks Before Posting

The row of KB-7 records the declarations of the shader, the target, that the entry point `main` reads `k`, and the outcome on x86_64 Linux. The rest of the draft is a reconstruction:

- The attributes, the parameters, and the body of `main` in Section 4.4.
- The steps in Section 4.5.
- The C++ names in Sections 4.5 and 4.7, which translate the Rust names in the row.
- The version in Section 4.3, which is that of the recorded Slang, because the row names only the dynamic route.

Before posting, the owner or an agent with the owner's go-ahead checks that Sections 4.4 and 4.5 reproduce the outcome of Section 4.7 with the recorded Slang. The repository records no `slangc` command line that reproduces the defect, so the draft names none.

### 4.2. Title

`spReflection_getGlobalConstantBufferBinding` returns 0 on SPIR-V when the global constant buffer is at another binding

### 4.3. Version and Platform

The Slang version is `2026.17-25-gc7954ebd1`, the Slang of the Vulkan SDK 1.4.363.1, used as the shared library of that SDK. The platform is x86_64 Linux. The record names no other platform.

### 4.4. Shader

This module declares two buffers at bindings 0 and 1 and one global uniform, which Slang places in the global constant buffer:

```slang
[[vk::binding(0)]] RWStructuredBuffer<float> data : register(u0);
[[vk::binding(1)]] RWStructuredBuffer<float> more : register(u1);
uniform float k;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    data[id.x] = k;
}
```

### 4.5. Steps to Reproduce

1. Create a global session, and a session with one target whose format is `SLANG_SPIRV`.
2. Load the module of Section 4.4 from source, find the entry point `main`, compose the module and the entry point, and link the composite.
3. Call `getLayout(0)` on the linked program, and then `getGlobalConstantBufferBinding()` on the returned `slang::ProgramLayout`, which calls `spReflection_getGlobalConstantBufferBinding`.
4. Generate the SPIR-V of the entry point with `getEntryPointCode(0, 0)` and disassemble it.

### 4.6. Expected Result

`getGlobalConstantBufferBinding()` returns 2, the binding that the generated SPIR-V gives the global constant buffer.

### 4.7. Observed Result

`getGlobalConstantBufferBinding()` returns 0, while the generated SPIR-V contains `OpDecorate %globalParams Binding 2`.

The one public call observed to give 2 is `getOffset(SLANG_PARAMETER_CATEGORY_DESCRIPTOR_TABLE_SLOT)` on the variable layout that `getGlobalParamsVarLayout()` returns.

For a program without global uniform data, `getGlobalConstantBufferBinding()` also returns 0, and so does that offset. A caller therefore cannot tell a global constant buffer at binding 0 from no global constant buffer by either value. Only the kind of the type layout of the global parameters, which is `slang::TypeReflection::Kind::ConstantBuffer` when the buffer exists, shows whether there is one.

### 4.8. Suspected Cause

`spReflection_getGlobalConstantBufferBinding` looks for the resource kind `ConstantBuffer` and returns 0 when it finds none, in `source/slang/slang-reflection-api.cpp`, lines 5514 to 5523. For a Vulkan target, the allocation of the binding records the kind `DescriptorTableSlot` instead, in `source/slang/slang-parameter-binding.cpp`, lines 3271 to 3292. The line numbers are those of the commit `c7954ebd1`.
