//! Compiles a Slang source file and prints what `slang-compiler` can tell
//! about it.
//!
//! The example shows the whole flow of the crate: it creates a global
//! session and a session with one target, loads the file as a module, composes
//! the module with its entry points, links the program, generates the code of
//! each entry point, and reflects the parameters of the program. It prints on
//! standard output, for each entry point, its stage, the size of its code, and
//! its parameters with their types and bindings, and then the global
//! parameters. Slang's warnings go to standard error. On a compile error, it
//! prints Slang's diagnostics on standard error and exits with a failure
//! status.
//!
//! Run it from the root of the workspace on the sample shader:
//!
//! ```sh
//! cargo run -p slang-rust-examples --bin compile-and-reflect -- examples/shaders/scale.slang
//! ```
//!
//! The program loads the Slang shared library of the Vulkan SDK when it
//! starts, so `$VULKAN_SDK/lib` has to be on `LD_LIBRARY_PATH` on Linux or on
//! `DYLD_LIBRARY_PATH` on macOS, and the SDK's `Bin` directory on `PATH` on
//! Windows. Pass `--help` for the options.

#![expect(
    clippy::print_stdout,
    reason = "the example prints its report on standard output"
)]
#![expect(
    clippy::print_stderr,
    reason = "the example prints Slang's diagnostics and its errors on standard error"
)]

#[path = "../shared/report.rs"]
mod report;

use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use slang_compiler::reflection::{LayoutSize, VariableLayout};
use slang_compiler::{
    BindingSpace, Compiled, Component, GlobalSession, ParameterCategory, Session, Target,
    TargetFormat,
};

use crate::report::report;

const USAGE: &str = "\
Usage: compile-and-reflect [OPTIONS] <FILE>

Compiles the Slang source file FILE and prints its entry points and parameters.

Options:
  --target <NAME>      The target format, one of spirv (the default), spirv-asm,
                       glsl, hlsl, wgsl, metal, cpp, cuda, and dxil
  --entry <NAME>       Compile only the entry point NAME, instead of every
                       entry point that the module defines
  --search-path <DIR>  Search DIR for the modules that FILE imports, and may
                       be given more than once
  -h, --help           Print this help
";

const TARGETS: [(&str, TargetFormat); 9] = [
    ("spirv", TargetFormat::Spirv),
    ("spirv-asm", TargetFormat::SpirvAssembly),
    ("glsl", TargetFormat::Glsl),
    ("hlsl", TargetFormat::Hlsl),
    ("wgsl", TargetFormat::Wgsl),
    ("metal", TargetFormat::Metal),
    ("cpp", TargetFormat::CppSource),
    ("cuda", TargetFormat::CudaSource),
    ("dxil", TargetFormat::Dxil),
];

struct Options {
    file: PathBuf,
    target: (&'static str, TargetFormat),
    entry: Option<String>,
    search_paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&*error);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let Some(options) = parse_args(std::env::args_os().skip(1))? else {
        print!("{USAGE}");
        return Ok(());
    };
    compile_and_reflect(&options)
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Option<Options>, String> {
    let mut args = args;
    let mut file = None;
    let mut target = TARGETS[0];
    let mut entry = None;
    let mut search_paths = Vec::new();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(None),
            Some("--target") => {
                let name = value_of("--target", args.next())?;
                target = TARGETS
                    .into_iter()
                    .find(|(known, _)| *known == name)
                    .ok_or_else(|| format!("unknown target {name:?}, see --help"))?;
            }
            Some("--entry") => entry = Some(value_of("--entry", args.next())?),
            Some("--search-path") => {
                let dir = args.next().ok_or("--search-path needs a directory")?;
                search_paths.push(PathBuf::from(dir));
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown option {flag}, see --help"));
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => return Err("expected one Slang file, see --help".to_owned()),
        }
    }
    let file = file.ok_or("missing the Slang file to compile, see --help")?;
    Ok(Some(Options {
        file,
        target,
        entry,
        search_paths,
    }))
}

fn value_of(flag: &str, value: Option<OsString>) -> Result<String, String> {
    value
        .ok_or_else(|| format!("{flag} needs a value"))?
        .into_string()
        .map_err(|value| format!("the value {} of {flag} is not valid UTF-8", value.display()))
}

fn compile_and_reflect(options: &Options) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&options.file)
        .map_err(|error| format!("cannot read {}: {error}", options.file.display()))?;
    let path = options
        .file
        .to_str()
        .ok_or("the file path is not valid UTF-8")?;
    let name = options
        .file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("the file name is not valid UTF-8")?;

    let global = GlobalSession::new()?;
    let mut builder = Session::builder(&global).target(Target::new(options.target.1));
    for dir in &options.search_paths {
        builder = builder.search_path(dir);
    }
    let session = builder.build()?;
    let target = session.target_index(0)?;

    let module = with_warnings(session.load_module_from_source(name, path, &source)?);
    let entry_points = match &options.entry {
        Some(entry) => vec![module.entry_point(entry)?],
        None => module.entry_points()?.collect::<Result<Vec<_>, _>>()?,
    };
    let mut components = vec![Component::Module(&module)];
    components.extend(entry_points.iter().map(Component::EntryPoint));
    let program = with_warnings(with_warnings(session.compose(&components)?).link()?);
    let layout = with_warnings(program.layout(target)?);

    println!("target: {}", options.target.0);
    for (position, entry) in layout.entry_point_layouts().enumerate() {
        let code = with_warnings(program.entry_point_code(position, target)?);
        let stage = entry
            .stage()?
            .map_or("unknown".to_owned(), |s| format!("{s:?}"));
        println!();
        println!(
            "entry point {} ({stage})",
            entry.name()?.unwrap_or("<unnamed>")
        );
        match code.as_spirv_words() {
            Some(words) if options.target.1 == TargetFormat::Spirv => {
                println!(
                    "  code: {} bytes, {} words",
                    code.as_bytes().len(),
                    words.len()
                );
            }
            _ => println!("  code: {} bytes", code.as_bytes().len()),
        }
        println!("  parameters:");
        for parameter in entry.parameter_layouts() {
            print_parameter(parameter, "    ")?;
        }
    }
    println!();
    println!("global parameters:");
    for parameter in layout.parameter_layouts() {
        print_parameter(parameter, "  ")?;
    }
    Ok(())
}

fn with_warnings<T>(compiled: Compiled<T>) -> T {
    if !compiled.diagnostics.is_empty() {
        eprintln!("{}", compiled.diagnostics);
    }
    compiled.value
}

fn print_parameter(parameter: VariableLayout<'_>, indent: &str) -> Result<(), Box<dyn Error>> {
    let name = match parameter.variable() {
        Some(variable) => variable.name()?.unwrap_or("<unnamed>"),
        None => "<unnamed>",
    };
    let Some(layout) = parameter.type_layout() else {
        println!("{indent}{name}: no layout");
        return Ok(());
    };
    let ty = match layout.ty() {
        Some(ty) => ty.full_name()?,
        None => "<unknown type>".to_owned(),
    };
    println!("{indent}{name}: {ty}");
    if let Some(semantic) = parameter.semantic_name()? {
        println!("{indent}  semantic {semantic}");
    }
    for category in layout.categories() {
        let category = category?;
        let offset = amount(parameter.offset(category).amount);
        let size = amount(layout.size(category).amount);
        if category == ParameterCategory::Uniform {
            println!("{indent}  uniform data at byte offset {offset}, {size} bytes");
        } else {
            let space = parameter
                .binding_space(category)
                .map_or("none".to_owned(), |space| {
                    BindingSpace::get(space).to_string()
                });
            println!("{indent}  {category:?} binding {offset} in space {space}, count {size}");
        }
    }
    Ok(())
}

fn amount(size: LayoutSize) -> String {
    match size {
        LayoutSize::Finite(amount) => amount.to_string(),
        LayoutSize::Unbounded => "unbounded".to_owned(),
        _ => "unknown".to_owned(),
    }
}
