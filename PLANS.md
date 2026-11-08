### TODO

- bindgen bindings for slang.h
- compile features for optional components (e.g. CUDA, OptiX, NVAPI, CPU target)
- multiple ffi packages to bind different optional components. details tbd, but for example:
    - slang-compiler-sys: core compiler
    - slang-llvm-sys: cpu target codegen
- ...
- idiomatic rust "slang" crate with design inspired by the SlangPy API

### If it gets to this point:

- contributor guidelines:
    - all PRs must be linked to an issue (otherwise auto-closed)
    - all feature-related discussion must happen in the issue
    - PR discussion is limited strictly to review of PR code
    - absolutely no AI usage allowed for drafting issues, PR descriptions, or comments
    - strict zero-tolerance for emojis in code and commit messages :P
    - PRs lacking appropriate commit hygiene will be closed without further review (
      see https://gist.github.com/efischer19/5d05178edd20054b3a9d85bd656f54f2#2-small-patchy-changes)
- AI usage policy
    - AI usage for writing code is allowed, but must be disclosed in the PR description
    - AI usage for writing documentation is allowed, but must be disclosed in the PR description
    - AI usage for writing commit messages is strictly prohibited
- github workflows for PR lints, style checks, etc.

### DevOps ToDo

- docker image(s) for build environment
- github workflows for CI & publishing crate