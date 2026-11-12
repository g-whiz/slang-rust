use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::check::{Selection, Validator, unfixed_prim};
use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::{
    CType, Enum, Field, Init, Interface, Item, ItemKind, Param, Record, Signature, Ty, Underlying,
    Value,
};
use crate::rules::{MacroAction, RuleSet};

const HEADER: &str = "//! Raw bindings to the Slang API, generated from the Slang headers by the binding generator. Run `just generate-bindings` to regenerate them instead of editing this file.\n\n";

const SYMBOLS_HEADER: &str = "//! The C functions of the raw Slang bindings, generated from the Slang headers by the binding generator. Run `just generate-bindings` to regenerate them instead of editing this file.\n\n";

const SYMBOLS_CRATE: &str = "slang_compiler_sys";

const POINTER_WIDTH_CHECK: &str = "#[cfg(not(target_pointer_width = \"64\"))]\ncompile_error!(\"the Slang bindings support only 64-bit targets: x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS\");\n\n";

const INTERFACE_TRAIT: &str = "Interface";

const RUSTFMT_TIMEOUT: Duration = Duration::from_secs(60);

const POLL_INTERVAL: Duration = Duration::from_millis(10);

const RAW_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];

const SUFFIXED_KEYWORDS: &[&str] = &["crate", "self", "Self", "super", "_"];

pub(crate) struct Emitted {
    pub(crate) bindings: String,
    pub(crate) symbols: String,
}

pub(crate) fn emit(selection: &Selection, rules: &RuleSet) -> Result<Emitted> {
    let mut emitter = Emitter {
        validator: Validator::new(selection, rules, "every target"),
        rules,
        selection,
        types: BTreeMap::new(),
        values: BTreeMap::new(),
        opaque: BTreeMap::new(),
        out: String::new(),
        externs: String::new(),
        functions: Vec::new(),
        trait_emitted: false,
    };
    emitter.run()?;
    let mut bindings = String::from(HEADER);
    bindings.push_str(POINTER_WIDTH_CHECK);
    bindings.push_str(&emitter.out);
    Ok(Emitted {
        bindings,
        symbols: symbols(&emitter.functions),
    })
}

fn symbols(functions: &[String]) -> String {
    let mut text = String::from(SYMBOLS_HEADER);
    text.push_str("pub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[\n");
    for name in functions {
        put(
            &mut text,
            format_args!(
                "    (\"{name}\", {SYMBOLS_CRATE}::{} as *const ()),\n",
                ident(name)
            ),
        );
    }
    text.push_str("];\n");
    text
}

pub(crate) fn rustfmt(text: &str) -> Result<String> {
    format_with("rustfmt", &["--edition", "2024"], text, RUSTFMT_TIMEOUT)
}

fn format_with(program: &str, args: &[&str], text: &str, timeout: Duration) -> Result<String> {
    let failure = |message: String| Error::new(ErrorKind::Rustfmt, message);
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| failure(format!("cannot start {program}")).with_source(error))?;
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(stop(&mut child).map_or_else(
            |error| failure(format!("cannot stop {program}")).with_source(error),
            |()| {
                failure(format!(
                    "cannot connect to the standard streams of {program}"
                ))
            },
        ));
    };
    let (written, status, output, errors) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(text.as_bytes()));
        let output = scope.spawn(move || read_all(stdout));
        let errors = scope.spawn(move || read_all(stderr));
        let status = wait_for(&mut child, timeout);
        (
            joined(writer.join()),
            status,
            joined(output.join()),
            joined(errors.join()),
        )
    });
    let status = status
        .map_err(|error| failure(format!("cannot wait for {program}")).with_source(error))?
        .ok_or_else(|| {
            failure(format!(
                "{program} did not finish within {timeout:?}, so the generator stopped it"
            ))
        })?;
    let read_error = |error: std::io::Error| {
        failure(format!("cannot read the output of {program}")).with_source(error)
    };
    let output = output.map_err(read_error)?;
    let errors = errors.map_err(read_error)?;
    if !status.success() {
        return Err(failure(format!(
            "{program} failed with {status}: {}",
            String::from_utf8_lossy(&errors).trim()
        )));
    }
    written.map_err(|error| {
        failure(format!("cannot write the bindings to {program}")).with_source(error)
    })?;
    String::from_utf8(output).map_err(|error| {
        failure(format!("{program} wrote text that is not UTF-8")).with_source(error)
    })
}

fn read_all(mut stream: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn joined<T>(result: std::thread::Result<std::io::Result<T>>) -> std::io::Result<T> {
    result.unwrap_or_else(|_| Err(std::io::Error::other("the thread panicked")))
}

fn wait_for(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(error) => return stop(child).and(Err(error)),
        }
    }
    stop(child).map(|()| None)
}

fn stop(child: &mut Child) -> std::io::Result<()> {
    child.kill()?;
    child.wait()?;
    Ok(())
}

struct Emitter<'a> {
    validator: Validator<'a>,
    rules: &'a RuleSet,
    selection: &'a Selection,
    types: BTreeMap<String, Origin>,
    values: BTreeMap<String, Origin>,
    opaque: BTreeMap<String, Origin>,
    out: String,
    externs: String,
    functions: Vec<String>,
    trait_emitted: bool,
}

struct Origin {
    cxx: String,
    location: Location,
}

#[derive(Clone, Copy)]
enum Namespace {
    Type,
    Value,
}

impl<'a> Emitter<'a> {
    fn run(&mut self) -> Result<()> {
        for item in &self.selection.items {
            self.item(item)?;
        }
        for selected in &self.selection.macros {
            let MacroAction::Constant(prim) = selected.action else {
                continue;
            };
            let Some(Value::Integer(value)) = selected.value else {
                return Err(internal(&selected.location, "a macro has no value"));
            };
            self.register(
                Namespace::Value,
                &selected.name,
                &selected.name,
                &selected.location,
            )?;
            put(
                &mut self.out,
                format_args!(
                    "pub const {}: {} = {};\n\n",
                    selected.name,
                    prim.rust(),
                    int_literal(value)
                ),
            );
        }
        let opaque = std::mem::take(&mut self.opaque);
        for (rust, origin) in &opaque {
            self.register(Namespace::Type, rust, &origin.cxx, &origin.location)?;
            self.out.push_str(&opaque_struct(rust));
        }
        if !self.externs.is_empty() {
            self.out.push_str("unsafe extern \"C-unwind\" {\n");
            self.out.push_str(&self.externs);
            self.out.push_str("}\n");
        }
        Ok(())
    }

    fn register(
        &mut self,
        namespace: Namespace,
        rust: &str,
        cxx: &str,
        location: &Location,
    ) -> Result<()> {
        let names = match namespace {
            Namespace::Type => &mut self.types,
            Namespace::Value => &mut self.values,
        };
        if let Some(other) = names.get(rust) {
            return Err(Error::at(
                ErrorKind::NameCollision,
                location,
                format!(
                    "{cxx} and {} ({}) would both have the Rust name {rust}",
                    other.cxx, other.location
                ),
            ));
        }
        names.insert(
            rust.to_owned(),
            Origin {
                cxx: cxx.to_owned(),
                location: location.clone(),
            },
        );
        Ok(())
    }

    fn item(&mut self, item: &'a Item) -> Result<()> {
        match &item.kind {
            ItemKind::Typedef(ty) => self.typedef(item, ty),
            ItemKind::Enum(enumeration) => self.enumeration(item, enumeration),
            ItemKind::Record(record) => self.record(item, record),
            ItemKind::Interface(interface) => self.interface(item, interface),
            ItemKind::Function(function) => {
                let name = rust_name_of(item)?;
                self.register(Namespace::Value, &name, &item.name.cxx, &item.location)?;
                let params = self.params(&function.signature.params, None)?;
                let result = self.result(&function.signature.result)?;
                put(
                    &mut self.externs,
                    format_args!("    pub fn {}({params}){result};\n", ident(&name)),
                );
                self.functions.push(name);
                Ok(())
            }
            ItemKind::Constant(constant) => {
                let name = rust_name_of(item)?;
                self.register(Namespace::Value, &name, &item.name.cxx, &item.location)?;
                let Some(value) = constant.value else {
                    return Err(internal(&item.location, "a constant has no value"));
                };
                let ty = self.rust_ty(&constant.ty)?;
                let value = self.value_expr(&constant.ty, value, &item.location)?;
                put(
                    &mut self.out,
                    format_args!("pub const {name}: {ty} = {value};\n\n"),
                );
                Ok(())
            }
            ItemKind::Opaque => {
                let name = rust_name_of(item)?;
                self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
                self.out.push_str(&opaque_struct(&name));
                Ok(())
            }
            ItemKind::Unsupported(_) => Err(internal(
                &item.location,
                "an unsupported declaration reached the emitter",
            )),
        }
    }

    fn typedef(&mut self, item: &'a Item, ty: &'a Ty) -> Result<()> {
        let name = rust_name_of(item)?;
        if name == self.rules.result_type {
            self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
            self.register(Namespace::Value, &name, &item.name.cxx, &item.location)?;
            put(
                &mut self.out,
                format_args!(
                    "#[repr(transparent)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub struct {name}(pub i32);\n\n"
                ),
            );
            let mut constants = String::new();
            for selected in &self.selection.macros {
                if selected.action != MacroAction::ResultCode {
                    continue;
                }
                let Some(Value::Integer(value)) = selected.value else {
                    return Err(internal(&selected.location, "a result code has no value"));
                };
                let bits = if value > i128::from(i32::MAX) {
                    value - (1_i128 << 32)
                } else {
                    value
                };
                put(
                    &mut constants,
                    format_args!(
                        "    pub const {}: Self = Self({});\n",
                        selected.name,
                        int_literal(bits)
                    ),
                );
            }
            if !constants.is_empty() {
                put(
                    &mut self.out,
                    format_args!("impl {name} {{\n{constants}}}\n\n"),
                );
            }
            return Ok(());
        }
        if let Some(prim) = self.rules.pointer_sized(&name) {
            self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
            put(
                &mut self.out,
                format_args!("pub type {name} = {};\n\n", prim.rust()),
            );
            return Ok(());
        }
        let target = self.rust_ty(ty)?;
        if target == name {
            return Ok(());
        }
        self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
        put(
            &mut self.out,
            format_args!("pub type {name} = {target};\n\n"),
        );
        Ok(())
    }

    fn underlying(&mut self, enumeration: &'a Enum) -> Result<String> {
        match &enumeration.underlying {
            Underlying::Fixed(ty) => self.rust_ty(ty),
            Underlying::Unfixed { .. } => Ok(unfixed_prim(enumeration).rust().to_owned()),
        }
    }

    fn enumeration(&mut self, item: &'a Item, enumeration: &'a Enum) -> Result<()> {
        if let Some(typedef) = &enumeration.constants_of {
            return self.flag_constants(item, enumeration, typedef);
        }
        let underlying = self.underlying(enumeration)?;
        let name = rust_name_of(item)?;
        self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
        self.register(Namespace::Value, &name, &item.name.cxx, &item.location)?;
        put(
            &mut self.out,
            format_args!(
                "#[repr(transparent)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub struct {name}(pub {underlying});\n\n"
            ),
        );
        if !enumeration.enumerators.is_empty() {
            put(&mut self.out, format_args!("impl {name} {{\n"));
            for enumerator in &enumeration.enumerators {
                put(
                    &mut self.out,
                    format_args!(
                        "    pub const {}: Self = Self({});\n",
                        ident(&enumerator.name),
                        int_literal(enumerator.value)
                    ),
                );
            }
            self.out.push_str("}\n\n");
        }
        Ok(())
    }

    fn flag_constants(
        &mut self,
        item: &'a Item,
        enumeration: &'a Enum,
        typedef: &str,
    ) -> Result<()> {
        let Some(typedef_item) = self.validator.included(typedef) else {
            return Err(internal(
                &item.location,
                "the typedef of an unnamed enumeration is not included",
            ));
        };
        let ItemKind::Typedef(inner) = &typedef_item.kind else {
            return Err(internal(
                &item.location,
                "the type of an unnamed enumeration is not a typedef",
            ));
        };
        let ty = rust_name_of(typedef_item)?;
        for enumerator in &enumeration.enumerators {
            self.register(
                Namespace::Value,
                &enumerator.name,
                &format!("{}::{}", item.name.cxx, enumerator.name),
                &item.location,
            )?;
            let value = self.value_expr(inner, Value::Integer(enumerator.value), &item.location)?;
            put(
                &mut self.out,
                format_args!("pub const {}: {ty} = {value};\n\n", ident(&enumerator.name)),
            );
        }
        Ok(())
    }

    fn record(&mut self, item: &'a Item, record: &'a Record) -> Result<()> {
        let name = rust_name_of(item)?;
        self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
        let field_names = field_names(&record.fields);
        let keyword = if record.union { "union" } else { "struct" };
        let derive = if record.union {
            "Clone, Copy"
        } else {
            "Clone, Copy, Debug"
        };
        put(
            &mut self.out,
            format_args!("#[repr(C)]\n#[derive({derive})]\npub {keyword} {name} {{\n"),
        );
        for (field, field_name) in record.fields.iter().zip(&field_names) {
            let ty = self.rust_ty(&field.ty)?;
            put(&mut self.out, format_args!("    pub {field_name}: {ty},\n"));
        }
        self.out.push_str("}\n\n");
        if record.union {
            put(
                &mut self.out,
                format_args!(
                    "impl ::core::fmt::Debug for {name} {{\n    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{\n        f.debug_struct(\"{name}\").finish_non_exhaustive()\n    }}\n}}\n\n"
                ),
            );
        }
        if has_default(record) {
            let mut initializers = String::new();
            for (field, field_name) in record.fields.iter().zip(&field_names) {
                let value = match field.init {
                    Init::Value(value) => self.value_expr(&field.ty, value, &field.location)?,
                    Init::None | Init::Null | Init::Zero | Init::Expression => {
                        self.zero_expr(&field.ty, &field.location)?
                    }
                };
                put(
                    &mut initializers,
                    format_args!("            {field_name}: {value},\n"),
                );
            }
            put(
                &mut self.out,
                format_args!(
                    "impl Default for {name} {{\n    fn default() -> Self {{\n        Self {{\n{initializers}        }}\n    }}\n}}\n\n"
                ),
            );
        }
        self.out.push_str("const _: () = {\n");
        put(
            &mut self.out,
            format_args!("    assert!(size_of::<{name}>() == {});\n", record.size),
        );
        put(
            &mut self.out,
            format_args!("    assert!(align_of::<{name}>() == {});\n", record.align),
        );
        for (field, field_name) in record.fields.iter().zip(&field_names) {
            put(
                &mut self.out,
                format_args!(
                    "    assert!(::core::mem::offset_of!({name}, {field_name}) == {});\n",
                    field.offset_bits / 8
                ),
            );
        }
        self.out.push_str("};\n\n");
        Ok(())
    }

    fn uuid_fields(&self) -> Result<Vec<String>> {
        let uuid = self
            .selection
            .items
            .iter()
            .find(|item| item.name.rust.as_deref() == Some(self.rules.uuid_type));
        match uuid.map(|item| (item, &item.kind)) {
            Some((_, ItemKind::Record(record))) => Ok(field_names(&record.fields)),
            Some((item, _)) => Err(internal(&item.location, "the UUID type is not a struct")),
            None => Err(Error::new(
                ErrorKind::Unsupported,
                "internal error in the binding generator: the bindings do not include the UUID type",
            )),
        }
    }

    fn interface(&mut self, item: &'a Item, interface: &'a Interface) -> Result<()> {
        let name = rust_name_of(item)?;
        let vtable = format!("{name}Vtbl");
        if !self.trait_emitted {
            self.register(
                Namespace::Type,
                INTERFACE_TRAIT,
                INTERFACE_TRAIT,
                &item.location,
            )?;
            put(
                &mut self.out,
                format_args!(
                    "pub unsafe trait {INTERFACE_TRAIT} {{\n    const UUID: {};\n}}\n\n",
                    self.rules.uuid_type
                ),
            );
            self.trait_emitted = true;
        }
        self.register(Namespace::Type, &name, &item.name.cxx, &item.location)?;
        self.register(Namespace::Type, &vtable, &item.name.cxx, &item.location)?;
        put(
            &mut self.out,
            format_args!(
                "#[repr(C)]\n#[derive(Debug)]\npub struct {name} {{\n    pub vtable: *const {vtable},\n}}\n\n"
            ),
        );
        put(
            &mut self.out,
            format_args!("#[repr(C)]\n#[derive(Debug)]\npub struct {vtable} {{\n"),
        );
        if let Some(base) = interface.bases.first() {
            let base = self.rust_name(base, &item.location)?;
            put(&mut self.out, format_args!("    pub base: {base}Vtbl,\n"));
        }
        for method in &interface.methods {
            let params = self.params(&method.signature.params, Some(&name))?;
            let result = self.result(&method.signature.result)?;
            put(
                &mut self.out,
                format_args!(
                    "    pub {}: unsafe extern \"system-unwind\" fn({params}){result},\n",
                    ident(&method.name)
                ),
            );
        }
        self.out.push_str("}\n\n");
        let uuid_fields = self.uuid_fields()?;
        let (Some(guid), [data1, data2, data3, data4]) = (interface.uuid, uuid_fields.as_slice())
        else {
            return Err(internal(&item.location, "the UUID has the wrong shape"));
        };
        let bytes: Vec<String> = guid
            .data4
            .iter()
            .map(|byte| hex_literal(u64::from(*byte), 2))
            .collect();
        put(
            &mut self.out,
            format_args!(
                "unsafe impl {INTERFACE_TRAIT} for {name} {{\n    const UUID: {uuid} = {uuid} {{\n        {}: {},\n        {}: {},\n        {}: {},\n        {}: [{}],\n    }};\n}}\n\n",
                data1,
                hex_literal(u64::from(guid.data1), 8),
                data2,
                hex_literal(u64::from(guid.data2), 4),
                data3,
                hex_literal(u64::from(guid.data3), 4),
                data4,
                bytes.join(", "),
                uuid = self.rules.uuid_type,
            ),
        );
        let chain = self.validator.chain(item, interface)?;
        put(&mut self.out, format_args!("impl {name} {{\n"));
        for (depth, owner) in chain.iter().enumerate().rev() {
            for method in &owner.methods {
                self.thunk(method, depth)?;
            }
        }
        self.out.push_str("}\n\n");
        Ok(())
    }

    fn thunk(&mut self, method: &'a crate::model::Method, depth: usize) -> Result<()> {
        let params = self.params(&method.signature.params, Some("Self"))?;
        let result = self.result(&method.signature.result)?;
        let names = param_names(&method.signature.params);
        let path = ".base".repeat(depth);
        let this = if depth == 0 { "this" } else { "this.cast()" };
        let mut arguments = vec![this.to_owned()];
        arguments.extend(names);
        let method_name = ident(&method.name);
        let terminator = if method.signature.result == Ty::Void {
            ";"
        } else {
            ""
        };
        put(
            &mut self.out,
            format_args!(
                "    pub unsafe fn {method_name}({params}){result} {{\n        unsafe {{ ((*(*this).vtable){path}.{method_name})({}){terminator} }}\n    }}\n\n",
                arguments.join(", ")
            ),
        );
        Ok(())
    }

    fn params(&mut self, params: &'a [Param], this: Option<&str>) -> Result<String> {
        let names = param_names(params);
        let mut rendered = Vec::new();
        if let Some(this) = this {
            rendered.push(format!("this: *mut {this}"));
        }
        for (param, name) in params.iter().zip(names) {
            rendered.push(format!("{name}: {}", self.rust_ty(&param.ty)?));
        }
        Ok(rendered.join(", "))
    }

    fn result(&mut self, ty: &'a Ty) -> Result<String> {
        if *ty == Ty::Void {
            return Ok(String::new());
        }
        Ok(format!(" -> {}", self.rust_ty(ty)?))
    }

    fn rust_name(&mut self, cxx: &str, location: &Location) -> Result<String> {
        if let Some(item) = self.validator.included(cxx) {
            return rust_name_of(item);
        }
        if let Some(item) = self.validator.excluded(cxx) {
            let name = rust_name_of(item)?;
            self.opaque.entry(name.clone()).or_insert_with(|| Origin {
                cxx: item.name.cxx.clone(),
                location: item.location.clone(),
            });
            return Ok(name);
        }
        Err(internal(
            location,
            &format!("{cxx} is neither included nor excluded"),
        ))
    }

    fn rust_ty(&mut self, ty: &'a Ty) -> Result<String> {
        let unknown = Location::new("<bindings>", 0);
        Ok(match ty {
            Ty::Void => "::core::ffi::c_void".to_owned(),
            Ty::Bool => "bool".to_owned(),
            Ty::C(c) => c.rust().to_owned(),
            Ty::Prim(prim) => prim.rust().to_owned(),
            Ty::Named(name) => self.rust_name(name, &unknown)?,
            Ty::Pointer { pointee, is_const } => format!(
                "*{} {}",
                if *is_const { "const" } else { "mut" },
                self.rust_ty(pointee)?
            ),
            Ty::Array { element, len } => format!("[{}; {len}]", self.rust_ty(element)?),
            Ty::Callback(signature) => self.callback(signature)?,
            Ty::Unsupported(spelling) => {
                return Err(internal(
                    &unknown,
                    &format!("the unsupported type {spelling} reached the emitter"),
                ));
            }
        })
    }

    fn callback(&mut self, signature: &'a Signature) -> Result<String> {
        let mut params = Vec::new();
        for param in &signature.params {
            params.push(self.rust_ty(&param.ty)?);
        }
        let result = self.result(&signature.result)?;
        Ok(format!(
            "Option<unsafe extern \"C\" fn({}){result}>",
            params.join(", ")
        ))
    }

    fn zero_expr(&mut self, ty: &'a Ty, location: &Location) -> Result<String> {
        Ok(match ty {
            Ty::Bool => "false".to_owned(),
            Ty::C(CType::Float | CType::Double) => "0.0".to_owned(),
            Ty::C(_) | Ty::Prim(_) => "0".to_owned(),
            Ty::Pointer { is_const: true, .. } => "::core::ptr::null()".to_owned(),
            Ty::Pointer {
                is_const: false, ..
            } => "::core::ptr::null_mut()".to_owned(),
            Ty::Callback(_) => "None".to_owned(),
            Ty::Array { element, len } => {
                format!("[{}; {len}]", self.zero_expr(element, location)?)
            }
            Ty::Named(name) => {
                let Some(item) = self.validator.included(name) else {
                    return Err(internal(location, "a field has an excluded type by value"));
                };
                let rust = rust_name_of(item)?;
                match &item.kind {
                    ItemKind::Typedef(_) if self.validator.is_result_type(name) => {
                        format!("{rust}(0)")
                    }
                    ItemKind::Typedef(inner) => self.zero_expr(inner, location)?,
                    ItemKind::Enum(_) => format!("{rust}(0)"),
                    ItemKind::Record(record) if has_default(record) => format!("{rust}::default()"),
                    ItemKind::Record(record) => {
                        let names = field_names(&record.fields);
                        let mut fields = Vec::new();
                        for (field, field_name) in record.fields.iter().zip(names) {
                            fields.push(format!(
                                "{field_name}: {}",
                                self.zero_expr(&field.ty, location)?
                            ));
                            if record.union {
                                break;
                            }
                        }
                        format!("{rust} {{ {} }}", fields.join(", "))
                    }
                    _ => {
                        return Err(internal(
                            location,
                            "a field has a type without a zero value",
                        ));
                    }
                }
            }
            Ty::Void | Ty::Unsupported(_) => {
                return Err(internal(
                    location,
                    "a field has a type without a zero value",
                ));
            }
        })
    }

    fn value_expr(&mut self, ty: &'a Ty, value: Value, location: &Location) -> Result<String> {
        let integer = match value {
            Value::Integer(integer) => integer,
            Value::Float(float) => return Ok(format!("{float:?}")),
        };
        Ok(match ty {
            Ty::Named(name) => {
                let Some(item) = self.validator.included(name) else {
                    return Err(internal(location, "a value has an excluded type"));
                };
                let rust = rust_name_of(item)?;
                match &item.kind {
                    ItemKind::Typedef(_) if self.validator.is_result_type(name) => {
                        format!("{rust}({})", int_literal(integer))
                    }
                    ItemKind::Typedef(inner) => self.value_expr(inner, value, location)?,
                    ItemKind::Enum(_) => format!("{rust}({})", int_literal(integer)),
                    _ => {
                        return Err(internal(
                            location,
                            "a value has a type that is not a number",
                        ));
                    }
                }
            }
            Ty::Bool => (integer != 0).to_string(),
            Ty::C(CType::Float | CType::Double) => format!("{integer}.0"),
            _ => int_literal(integer),
        })
    }
}

fn internal(location: &Location, message: &str) -> Error {
    Error::at(
        ErrorKind::Unsupported,
        location,
        format!("internal error in the binding generator: {message}"),
    )
}

fn rust_name_of(item: &Item) -> Result<String> {
    item.name.rust.clone().ok_or_else(|| {
        Error::at(
            ErrorKind::Unsupported,
            &item.location,
            format!("{} has no name that Rust code can refer to", item.name.cxx),
        )
    })
}

fn has_default(record: &Record) -> bool {
    record.fields.iter().any(|field| field.init != Init::None)
}

fn opaque_struct(name: &str) -> String {
    format!(
        "#[repr(C)]\n#[derive(Debug)]\npub struct {name} {{\n    _data: [u8; 0],\n    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,\n}}\n\n"
    )
}

#[expect(
    clippy::expect_used,
    reason = "the fmt::Write implementation of String never returns an error"
)]
fn put(out: &mut String, text: std::fmt::Arguments<'_>) {
    std::fmt::Write::write_fmt(out, text).expect("internal error: writing to a String cannot fail");
}

pub(crate) fn ident(name: &str) -> String {
    if SUFFIXED_KEYWORDS.contains(&name) {
        format!("{name}_")
    } else if RAW_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_owned()
    }
}

fn unique(name: String, used: &mut HashSet<String>) -> String {
    let mut name = name;
    while !used.insert(name.clone()) {
        name.push('_');
    }
    name
}

fn param_names(params: &[Param]) -> Vec<String> {
    let mut used = HashSet::from(["this".to_owned()]);
    params
        .iter()
        .enumerate()
        .map(|(position, param)| {
            let name = param
                .name
                .as_deref()
                .map_or_else(|| format!("arg{position}"), ident);
            unique(name, &mut used)
        })
        .collect()
}

fn field_names(fields: &[Field]) -> Vec<String> {
    let mut used = HashSet::new();
    let mut anonymous = 0_usize;
    fields
        .iter()
        .map(|field| {
            let name = if let Some(name) = &field.name {
                ident(name)
            } else {
                anonymous += 1;
                format!("anon{}", anonymous - 1)
            };
            unique(name, &mut used)
        })
        .collect()
}

fn group(digits: &str, size: usize) -> String {
    let mut grouped = String::new();
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position).is_multiple_of(size) {
            grouped.push('_');
        }
        grouped.push(digit);
    }
    grouped
}

pub(crate) fn int_literal(value: i128) -> String {
    let digits = value.unsigned_abs().to_string();
    let grouped = if digits.len() > 4 {
        group(&digits, 3)
    } else {
        digits
    };
    if value < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

pub(crate) fn hex_literal(value: u64, width: usize) -> String {
    let digits = format!("{value:0width$X}");
    if digits.len() > 4 {
        format!("0x{}", group(&digits, 4))
    } else {
        format!("0x{digits}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_literal_groups_long_numbers_by_three_digits() {
        assert_eq!(int_literal(0), "0");
        assert_eq!(int_literal(1024), "1024");
        assert_eq!(int_literal(65_535), "65_535");
        assert_eq!(int_literal(-2_147_467_259), "-2_147_467_259");
        assert_eq!(
            int_literal(i128::from(u64::MAX)),
            "18_446_744_073_709_551_615"
        );
    }

    #[test]
    fn hex_literal_pads_to_the_width_and_groups_by_four_digits() {
        assert_eq!(hex_literal(0xC140_B5FD, 8), "0xC140_B5FD");
        assert_eq!(hex_literal(0x0C78, 4), "0x0C78");
        assert_eq!(hex_literal(0x6, 2), "0x06");
    }

    #[test]
    fn ident_escapes_rust_keywords() {
        assert_eq!(ident("type"), "r#type");
        assert_eq!(ident("use"), "r#use");
        assert_eq!(ident("self"), "self_");
        assert_eq!(ident("count"), "count");
    }

    #[test]
    fn param_names_number_unnamed_parameters_and_keep_them_unique() {
        let param = |name: Option<&str>| Param {
            name: name.map(str::to_owned),
            ty: Ty::Bool,
        };
        let names = param_names(&[param(None), param(Some("arg0")), param(Some("this"))]);
        assert_eq!(names, ["arg0", "arg0_", "this_"]);
    }

    #[cfg(target_os = "linux")]
    mod ffi {
        use std::time::{Duration, Instant};

        use super::super::{format_with, rustfmt};
        use crate::error::ErrorKind;

        const LONG: Duration = Duration::from_secs(60);

        #[test]
        fn rustfmt_formats_valid_rust() {
            let text = rustfmt("pub   const A : u8=1;").expect("rustfmt formats the text");
            assert_eq!(text, "pub const A: u8 = 1;\n");
        }

        #[test]
        fn rustfmt_reports_its_exit_status_and_standard_error_on_invalid_rust() {
            let error = rustfmt("fn (").expect_err("rustfmt rejects the text");
            assert_eq!(error.kind(), ErrorKind::Rustfmt, "{error}");
            assert!(
                error
                    .message()
                    .starts_with("rustfmt failed with exit status: 1: "),
                "{error}"
            );
            assert!(error.message().contains("error"), "{error}");
        }

        #[test]
        fn format_with_reports_a_program_that_cannot_start() {
            let error = format_with("bindings-generator-test-no-such-rustfmt", &[], "", LONG)
                .expect_err("the program is missing");
            assert_eq!(error.kind(), ErrorKind::Rustfmt, "{error}");
            assert_eq!(
                error.message(),
                "cannot start bindings-generator-test-no-such-rustfmt"
            );
            assert!(std::error::Error::source(&error).is_some(), "{error}");
        }

        #[test]
        fn format_with_reports_the_status_of_a_program_that_exits_before_reading_its_input() {
            let text = "x".repeat(16 << 20);
            let error = format_with("sh", &["-c", "echo refused >&2; exit 3"], &text, LONG)
                .expect_err("the program fails");
            assert_eq!(error.kind(), ErrorKind::Rustfmt, "{error}");
            assert_eq!(error.message(), "sh failed with exit status: 3: refused");
        }

        #[test]
        fn format_with_stops_a_program_that_does_not_finish_in_time() {
            let start = Instant::now();
            let error = format_with(
                "sh",
                &["-c", "exec sleep 30"],
                "",
                Duration::from_millis(200),
            )
            .expect_err("the program is stopped");
            assert!(start.elapsed() < Duration::from_secs(20), "{error}");
            assert_eq!(error.kind(), ErrorKind::Rustfmt, "{error}");
            assert_eq!(
                error.message(),
                "sh did not finish within 200ms, so the generator stopped it"
            );
        }
    }
}
