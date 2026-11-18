use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::check::{Selection, Validator, WithoutDefault, unfixed_prim};
use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::{
    CType, Enum, Field, Init, Interface, Item, ItemKind, Param, Record, Signature, Ty, Underlying,
    Value,
};
use crate::rules::{MacroAction, RuleSet};

const PROBE_FUNCTION: &str = "slang_rhi_sys_abi_probe_slots";

const INTERFACE_TRAIT: &str = "Interface";

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
    pub(crate) abi_probe: Option<String>,
    pub(crate) types: TypeNames,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeNames {
    pub(crate) included: BTreeSet<String>,
    pub(crate) opaque: BTreeSet<String>,
}

pub(crate) fn emit(selection: &Selection, rules: &RuleSet, target: &str) -> Result<Emitted> {
    let mut emitter = Emitter {
        validator: Validator::new(selection, rules, target, None),
        rules,
        selection,
        types: BTreeMap::new(),
        values: BTreeMap::new(),
        opaque: BTreeMap::new(),
        opaque_names: BTreeSet::new(),
        out: String::new(),
        externs: String::new(),
        functions: Vec::new(),
        slots: Vec::new(),
        trait_emitted: false,
    };
    emitter.run()?;
    let mut bindings = format!(
        "#[cfg(not(target_pointer_width = \"64\"))]\ncompile_error!(\"the {} bindings support only 64-bit targets\");\n\n",
        rules.library
    );
    bindings.push_str(&emitter.out);
    let mut symbols = symbols(rules.own_crate, &emitter.functions);
    let abi_probe = if rules.abi_probe {
        symbols.push_str(&slot_list(rules.own_crate, &emitter.slots));
        Some(crate::probe::probe(
            selection,
            rules,
            Some(emitter.slots.as_slice()),
        )?)
    } else {
        None
    };
    let opaque = emitter.opaque_names;
    let included = emitter
        .types
        .into_keys()
        .filter(|name| !opaque.contains(name))
        .collect();
    Ok(Emitted {
        bindings,
        symbols,
        abi_probe,
        types: TypeNames { included, opaque },
    })
}

fn symbols(own_crate: &str, functions: &[String]) -> String {
    let mut text = String::from("pub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[\n");
    for name in functions {
        put(
            &mut text,
            format_args!(
                "    (\"{name}\", {own_crate}::{} as *const ()),\n",
                ident(name)
            ),
        );
    }
    text.push_str("];\n");
    text
}

pub(crate) struct Slot {
    pub(crate) interface_cxx: String,
    pub(crate) interface_rust: String,
    pub(crate) method: String,
    pub(crate) field: String,
    pub(crate) canonical: crate::model::CanonicalSignature,
}

fn slot_list(own_crate: &str, slots: &[Slot]) -> String {
    let mut text = format!(
        "unsafe extern \"C\" {{\n    pub(super) fn {PROBE_FUNCTION}(out: *mut usize, capacity: usize) -> usize;\n}}\n\npub(super) const VTABLE_SLOTS: &[(&str, usize)] = &[\n"
    );
    for slot in slots {
        put(
            &mut text,
            format_args!(
                "    (\"{}::{}\", ::core::mem::offset_of!({own_crate}::{}Vtbl, {})),\n",
                slot.interface_rust, slot.field, slot.interface_rust, slot.field
            ),
        );
    }
    text.push_str("];\n");
    text
}

struct Emitter<'a> {
    validator: Validator<'a>,
    rules: &'a RuleSet,
    selection: &'a Selection,
    types: BTreeMap<String, Origin>,
    values: BTreeMap<String, Origin>,
    opaque: BTreeMap<String, Origin>,
    opaque_names: BTreeSet<String>,
    out: String,
    externs: String,
    functions: Vec<String>,
    slots: Vec<Slot>,
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
            self.opaque_names.insert(rust.clone());
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
                self.opaque_names.insert(name.clone());
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
        let ty = self.path_of(typedef_item)?;
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
        } else if name == self.rules.uuid_type {
            "Clone, Copy, Debug, PartialEq, Eq, Hash"
        } else {
            "Clone, Copy, Debug"
        };
        let without_default = self.selection.without_default.get(&item.name.cxx);
        if let Some(without_default) = without_default {
            let doc = self.without_default_doc(keyword, without_default)?;
            put(&mut self.out, format_args!("#[doc = {doc:?}]\n"));
        }
        put(
            &mut self.out,
            format_args!("#[repr(C)]\n#[derive({derive})]\npub {keyword} {name} {{\n"),
        );
        for (field, field_name) in record.fields.iter().zip(&field_names) {
            let ty = self.rust_ty(&field.ty)?;
            if without_default.is_some() {
                let doc = field_default_doc(field);
                put(&mut self.out, format_args!("    #[doc = {doc:?}]\n"));
            }
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
        if has_default(record) && without_default.is_none() {
            let mut initializers = String::new();
            for (field, field_name) in record.fields.iter().zip(&field_names) {
                let value = match &field.init {
                    Init::Value(value) => self.value_expr(&field.ty, *value, &field.location)?,
                    Init::Constant(constant) => self.rust_name(constant, &field.location)?,
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

    fn without_default_doc(
        &self,
        keyword: &str,
        without_default: &WithoutDefault,
    ) -> Result<String> {
        let why = match without_default {
            WithoutDefault::Rule(reason) => {
                format!("{reason}, so the bindings cannot reproduce its C++ default")
            }
            WithoutDefault::Holds { field, held } => {
                let held = self
                    .validator
                    .included(held)
                    .map_or_else(|| Ok(held.clone()), |held| self.path_of(held))?;
                format!("its field `{field}` holds `{held}` by value, which has none")
            }
        };
        Ok(format!(
            "This {keyword} has no `Default` implementation, because {why}. A caller states every field, and the documentation of each field gives its C++ default member initializer."
        ))
    }

    fn path_of(&self, item: &Item) -> Result<String> {
        let name = rust_name_of(item)?;
        match self.rules.external_crate {
            Some(external) if self.validator.is_external(&item.name.cxx) => {
                Ok(format!("::{external}::{name}"))
            }
            _ => Ok(name),
        }
    }

    fn crate_path(&self, name: &str) -> String {
        self.rules.external_crate.map_or_else(
            || name.to_owned(),
            |external| format!("::{external}::{name}"),
        )
    }

    fn uuid_fields(&self) -> Result<Vec<String>> {
        let uuid = self.validator.uuid_item();
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
        self.interface_structs(item, interface, &name)?;
        if interface.com {
            self.interface_uuid_impl(item, interface, &name)?;
        }
        if self.rules.abi_probe {
            for method in &interface.methods {
                self.slots.push(Slot {
                    interface_cxx: item.name.cxx.clone(),
                    interface_rust: name.clone(),
                    method: method.name.clone(),
                    field: ident(method.rust_name()),
                    canonical: method.canonical.clone(),
                });
            }
        }
        self.interface_thunks(item, interface, &name)
    }

    fn interface_structs(
        &mut self,
        item: &'a Item,
        interface: &'a Interface,
        name: &str,
    ) -> Result<()> {
        let vtable = format!("{name}Vtbl");
        if !self.trait_emitted && self.rules.external_crate.is_none() {
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
        self.register(Namespace::Type, name, &item.name.cxx, &item.location)?;
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
            if method.placeholder {
                put(
                    &mut self.out,
                    format_args!(
                        "    pub {}: *const ::core::ffi::c_void,\n",
                        ident(method.rust_name())
                    ),
                );
                continue;
            }
            let params = self.params(&method.signature.params, Some(name))?;
            let result = self.result(&method.signature.result)?;
            put(
                &mut self.out,
                format_args!(
                    "    pub {}: unsafe extern \"system-unwind\" fn({params}){result},\n",
                    ident(method.rust_name())
                ),
            );
        }
        self.out.push_str("}\n\n");
        Ok(())
    }

    fn interface_uuid_impl(
        &mut self,
        item: &'a Item,
        interface: &'a Interface,
        name: &str,
    ) -> Result<()> {
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
        let uuid = self.crate_path(self.rules.uuid_type);
        let interface_trait = self.crate_path(INTERFACE_TRAIT);
        put(
            &mut self.out,
            format_args!(
                "unsafe impl {interface_trait} for {name} {{\n    const UUID: {uuid} = {uuid} {{\n        {}: {},\n        {}: {},\n        {}: {},\n        {}: [{}],\n    }};\n}}\n\n",
                data1,
                hex_literal(u64::from(guid.data1), 8),
                data2,
                hex_literal(u64::from(guid.data2), 4),
                data3,
                hex_literal(u64::from(guid.data3), 4),
                data4,
                bytes.join(", "),
            ),
        );
        Ok(())
    }

    fn interface_thunks(
        &mut self,
        item: &'a Item,
        interface: &'a Interface,
        name: &str,
    ) -> Result<()> {
        let chain = self.validator.chain(item, interface)?;
        put(&mut self.out, format_args!("impl {name} {{\n"));
        for (depth, owner) in chain.iter().enumerate().rev() {
            for method in owner.methods.iter().filter(|method| !method.placeholder) {
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
        let method_name = ident(method.rust_name());
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
            return self.path_of(item);
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
            Ty::Template { args, .. } => match args.as_slice() {
                [arg] => format!("*mut {}", self.rust_ty(arg)?),
                _ => {
                    return Err(internal(
                        &unknown,
                        "a template type without exactly one argument reached the emitter",
                    ));
                }
            },
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
        let abi = if signature.called_from_rust {
            "C-unwind"
        } else {
            "C"
        };
        Ok(format!(
            "Option<unsafe extern \"{abi}\" fn({}){result}>",
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
            }
            | Ty::Template { .. } => "::core::ptr::null_mut()".to_owned(),
            Ty::Callback(_) => "None".to_owned(),
            Ty::Array { element, len } => {
                format!("[{}; {len}]", self.zero_expr(element, location)?)
            }
            Ty::Named(name) => {
                let (item, rust) =
                    self.included_type(name, location, "a field has an excluded type by value")?;
                match &item.kind {
                    ItemKind::Typedef(_) if self.validator.is_result_type(name) => {
                        format!("{rust}(0)")
                    }
                    ItemKind::Typedef(inner) => self.zero_expr(inner, location)?,
                    ItemKind::Enum(_) => format!("{rust}(0)"),
                    ItemKind::Record(_) if self.selection.without_default.contains_key(name) => {
                        return Err(internal(
                            location,
                            "a default value needs a struct without Default",
                        ));
                    }
                    ItemKind::Record(record) if has_default(record) => format!("{rust}::default()"),
                    ItemKind::Record(record) => self.zero_record(&rust, record, location)?,
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

    fn zero_record(
        &mut self,
        rust: &str,
        record: &'a Record,
        location: &Location,
    ) -> Result<String> {
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
        Ok(format!("{rust} {{ {} }}", fields.join(", ")))
    }

    fn included_type(
        &self,
        name: &str,
        location: &Location,
        message: &str,
    ) -> Result<(&'a Item, String)> {
        let Some(item) = self.validator.included(name) else {
            return Err(internal(location, message));
        };
        Ok((item, self.path_of(item)?))
    }

    fn value_expr(&mut self, ty: &'a Ty, value: Value, location: &Location) -> Result<String> {
        let integer = match value {
            Value::Integer(integer) => integer,
            Value::Float(float) => return Ok(format!("{float:?}")),
        };
        Ok(match ty {
            Ty::Named(name) => {
                let (item, rust) =
                    self.included_type(name, location, "a value has an excluded type")?;
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
            Ty::Pointer { is_const: true, .. } => format!(
                "::core::ptr::without_provenance({}_usize)",
                int_literal(integer)
            ),
            Ty::Pointer {
                is_const: false, ..
            } => format!(
                "::core::ptr::without_provenance_mut({}_usize)",
                int_literal(integer)
            ),
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

fn field_default_doc(field: &Field) -> String {
    field.spelling.as_ref().map_or_else(
        || "The C++ declaration of this field has no default member initializer.".to_owned(),
        |spelling| format!("The C++ default member initializer of this field is `= {spelling}`."),
    )
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
pub(crate) fn put(out: &mut String, text: std::fmt::Arguments<'_>) {
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
}
