//! Helpers of the tests that compile Slang code: compilation in one call,
//! a scratch directory for files, and a reader of SPIR-V words.

use std::fs;
use std::path::PathBuf;

use slang_compiler::reflection::Type;
use slang_compiler::{Blob, Compiled, Component, Diagnostics, Module, Result, Session};

/// The first word of every SPIR-V module.
pub(super) const SPIRV_MAGIC: u32 = 0x0723_0203;

/// The opcode of `OpEntryPoint`.
pub(super) const OP_ENTRY_POINT: u32 = 15;

/// The opcode of `OpExtInstImport`.
pub(super) const OP_EXT_INST_IMPORT: u32 = 11;

/// The opcode of `OpVariable`.
pub(super) const OP_VARIABLE: u32 = 59;

/// The opcode of `OpDecorate`.
pub(super) const OP_DECORATE: u32 = 71;

/// The opcode of `OpMemberDecorate`.
pub(super) const OP_MEMBER_DECORATE: u32 = 72;

/// The decoration `ArrayStride`.
pub(super) const DECORATION_ARRAY_STRIDE: u32 = 6;

/// The decoration `Offset`.
pub(super) const DECORATION_OFFSET: u32 = 35;

/// The tool identifier of glslang in the generator word of a SPIR-V header.
pub(super) const GLSLANG_GENERATOR: u32 = 8;

/// One SPIR-V instruction.
#[derive(Debug)]
pub(super) struct Instruction {
    pub(super) opcode: u32,
    pub(super) operands: Vec<u32>,
}

/// Reads SPIR-V bytes as little-endian words, whatever their alignment.
pub(super) fn words(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0, "SPIR-V is a sequence of words");
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect()
}

/// Splits SPIR-V words after the header of five words into instructions.
pub(super) fn instructions(words: &[u32]) -> Vec<Instruction> {
    let mut rest = words.get(5..).expect("a SPIR-V header has five words");
    let mut found = Vec::new();
    while let Some(&first) = rest.first() {
        let count = usize::try_from(first >> 16).expect("a word count fits");
        assert!(count >= 1 && count <= rest.len(), "a valid word count");
        found.push(Instruction {
            opcode: first & 0xFFFF,
            operands: rest[1..count].to_vec(),
        });
        rest = &rest[count..];
    }
    found
}

/// Decodes a literal string of SPIR-V operands, which ends at its first NUL
/// byte.
pub(super) fn literal(operands: &[u32]) -> String {
    let bytes = operands
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .take_while(|byte| *byte != 0)
        .collect::<Vec<_>>();
    String::from_utf8(bytes).expect("a SPIR-V literal is UTF-8")
}

/// Returns the names of the `OpEntryPoint` instructions of SPIR-V words.
pub(super) fn entry_point_names(words: &[u32]) -> Vec<String> {
    instructions(words)
        .iter()
        .filter(|instruction| instruction.opcode == OP_ENTRY_POINT)
        .map(|instruction| literal(&instruction.operands[2..]))
        .collect()
}

/// Returns the literal operands of every decoration `decoration` of SPIR-V
/// words, from `OpDecorate` and `OpMemberDecorate`.
pub(super) fn decorations(words: &[u32], decoration: u32) -> Vec<u32> {
    instructions(words)
        .iter()
        .filter_map(|instruction| match instruction.opcode {
            OP_DECORATE if instruction.operands.get(1) == Some(&decoration) => {
                instruction.operands.get(2).copied()
            }
            OP_MEMBER_DECORATE if instruction.operands.get(2) == Some(&decoration) => {
                instruction.operands.get(3).copied()
            }
            _ => None,
        })
        .collect()
}

/// Returns the number of instructions with `opcode`.
pub(super) fn count(words: &[u32], opcode: u32) -> usize {
    instructions(words)
        .iter()
        .filter(|instruction| instruction.opcode == opcode)
        .count()
}

/// Loads `source` as the module `name`, composes it with its entry point
/// `main`, links, and generates the code of the entry point for the target
/// at `target`.
pub(super) fn compile(
    session: &Session,
    name: &str,
    source: &str,
    target: usize,
) -> Result<Compiled<Blob>> {
    let module = session
        .load_module_from_source(name, &format!("{name}.slang"), source)?
        .into_value();
    let entry_point = module.entry_point("main")?;
    let program = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])?
        .into_value()
        .link()?
        .into_value();
    program.entry_point_code(0, session.target_index(target)?)
}

/// Compiles as [`compile`] does and returns the SPIR-V words.
pub(super) fn compile_spirv(
    session: &Session,
    name: &str,
    source: &str,
    target: usize,
) -> Vec<u32> {
    let code = compile(session, name, source, target)
        .expect("the source compiles")
        .into_value();
    let words = words(code.as_bytes());
    assert_eq!(words.first(), Some(&SPIRV_MAGIC));
    words
}

/// Compiles as [`compile`] does and returns the text of the code.
pub(super) fn compile_text(session: &Session, name: &str, source: &str, target: usize) -> String {
    let code = compile(session, name, source, target)
        .expect("the source compiles")
        .into_value();
    code.to_str().expect("the code is text").to_owned()
}

/// Returns an empty scratch directory for the test `name` under the target
/// directory of Cargo.
pub(super) fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("slang-compiler")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("the old scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("the scratch directory is created");
    dir
}

/// Returns whether diagnostics hold a message with the number `code`, in
/// Slang's form `E` and five digits.
pub(super) fn has_code(diagnostics: &Diagnostics, code: u32) -> bool {
    diagnostics.as_str().contains(&format!("E{code:05}"))
}

/// Finds the type `name` in the layout of `module` for the first target of
/// its session.
///
/// `getLayout` lets a C++ exception escape for a module whose parameters nest
/// types past Slang's limit, so the tests call it only on modules without
/// such parameters.
pub(super) fn find_type<'module>(
    session: &Session,
    module: &'module Module,
    name: &str,
) -> Option<Type<'module>> {
    let target = session.target_index(0).expect("the session has a target");
    module
        .layout(target)
        .expect("Slang lays out the module")
        .into_value()
        .find_type_by_name(name)
        .expect("the name has no NUL byte")
}
