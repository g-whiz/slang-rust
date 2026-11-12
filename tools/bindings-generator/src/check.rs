use std::collections::{HashMap, HashSet};

use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::{
    CType, Init, Interface, Item, ItemKind, Macro, Model, Prim, Record, Signature, Ty, Underlying,
    Value,
};
use crate::rules::{Action, MacroAction, RuleSet};

const C_CONVENTION: &str = "Cdecl";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Selection {
    pub(crate) items: Vec<Item>,
    pub(crate) excluded: Vec<Item>,
    pub(crate) macros: Vec<SelectedMacro>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SelectedMacro {
    pub(crate) name: String,
    pub(crate) location: Location,
    pub(crate) action: MacroAction,
    pub(crate) reason: &'static str,
    pub(crate) value: Option<Value>,
}

pub(crate) fn select(model: Model, rules: &RuleSet) -> Result<Selection> {
    let mut used = Used {
        declarations: vec![false; rules.declarations.len()],
        macros: vec![false; rules.macros.len()],
        unnamed_enums: vec![false; rules.unnamed_enums.len()],
    };
    let mut actions: HashMap<String, Action> = HashMap::new();
    let mut constants_of: HashMap<String, &'static str> = HashMap::new();
    let mut unmatched = Vec::new();
    let mut first_unmatched = None;
    for item in model.items.iter().filter(|item| item.owner.is_none()) {
        let file = item.location.file();
        if let ItemKind::Enum(enumeration) = &item.kind
            && enumeration.unnamed
        {
            if let Some(position) = rules.unnamed_enum_position(file, &enumeration.enumerators) {
                used.unnamed_enums[position] = true;
                actions.insert(item.name.cxx.clone(), Action::Include);
                constants_of.insert(item.name.cxx.clone(), rules.unnamed_enums[position].typedef);
            } else {
                first_unmatched.get_or_insert_with(|| item.location.clone());
                let first = enumeration
                    .enumerators
                    .first()
                    .map_or("no enumerator", |enumerator| enumerator.name.as_str());
                unmatched.push(format!(
                    "the unnamed enumeration that starts with {first} ({})",
                    item.location
                ));
            }
        } else if let Some(position) = rules.declaration_position(file, &item.name.cxx) {
            used.declarations[position] = true;
            actions.insert(item.name.cxx.clone(), rules.declarations[position].action);
        } else {
            first_unmatched.get_or_insert_with(|| item.location.clone());
            unmatched.push(format!("{} ({})", item.name.cxx, item.location));
        }
    }
    let mut macros = Vec::new();
    for Macro { name, location } in model.macros {
        let Some(position) = rules.macro_position(&name) else {
            unmatched.push(format!("macro {name} ({location})"));
            first_unmatched.get_or_insert(location);
            continue;
        };
        used.macros[position] = true;
        let rule = &rules.macros[position];
        if rule.action != MacroAction::Exclude {
            macros.push(SelectedMacro {
                name,
                location,
                action: rule.action,
                reason: rule.reason,
                value: None,
            });
        }
    }
    if let Some(first) = first_unmatched {
        return Err(Error::at(
            ErrorKind::UnmatchedDeclaration,
            &first,
            format!(
                "{} declarations and macros match no rule of the rule set: {}",
                unmatched.len(),
                unmatched.join(", ")
            ),
        ));
    }
    used.check(rules)?;
    let owners: HashMap<String, String> = model
        .items
        .iter()
        .filter_map(|item| Some((item.name.cxx.clone(), item.owner.clone()?)))
        .collect();
    let mut items = Vec::new();
    let mut excluded = Vec::new();
    for mut item in model.items {
        let root = root_owner(&owners, &item.name.cxx);
        if let ItemKind::Enum(enumeration) = &mut item.kind
            && let Some(typedef) = constants_of.get(&item.name.cxx)
        {
            enumeration.constants_of = Some((*typedef).to_owned());
        }
        if actions.get(root) == Some(&Action::Include) {
            items.push(item);
        } else {
            excluded.push(item);
        }
    }
    Ok(Selection {
        items,
        excluded,
        macros,
    })
}

fn root_owner<'a>(owners: &'a HashMap<String, String>, name: &'a str) -> &'a str {
    let mut current = name;
    for _ in 0..=owners.len() {
        match owners.get(current) {
            Some(owner) => current = owner,
            None => break,
        }
    }
    current
}

struct Used {
    declarations: Vec<bool>,
    macros: Vec<bool>,
    unnamed_enums: Vec<bool>,
}

impl Used {
    fn check(&self, rules: &RuleSet) -> Result<()> {
        let mut stale = Vec::new();
        for (rule, used) in rules.declarations.iter().zip(&self.declarations) {
            if !used {
                stale.push(format!(
                    "the declaration rule {}{} ({})",
                    rule.pattern,
                    rule.header
                        .map_or_else(String::new, |header| format!(" in {header}")),
                    rule.reason
                ));
            }
        }
        for (rule, used) in rules.macros.iter().zip(&self.macros) {
            if !used {
                stale.push(format!("the macro rule {} ({})", rule.pattern, rule.reason));
            }
        }
        for (rule, used) in rules.unnamed_enums.iter().zip(&self.unnamed_enums) {
            if !used {
                stale.push(format!(
                    "the rule that assigns the unnamed enumeration with the enumerator {} in {} to {} ({})",
                    rule.enumerator, rule.header, rule.typedef, rule.reason
                ));
            }
        }
        if stale.is_empty() {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::StaleRule,
            format!(
                "{} rules of the rule set decide no declaration or macro of the headers, so remove or correct them: {}",
                stale.len(),
                stale.join(", ")
            ),
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Value,
    Field,
    Pointee,
    Typedef,
}

pub(crate) struct Validator<'a> {
    selection: &'a Selection,
    rules: &'a RuleSet,
    target: &'a str,
    included: HashMap<&'a str, &'a Item>,
    excluded: HashMap<&'a str, &'a Item>,
}

pub(crate) fn validate(selection: &Selection, rules: &RuleSet, target: &str) -> Result<()> {
    let validator = Validator::new(selection, rules, target);
    validator.validate()
}

impl<'a> Validator<'a> {
    pub(crate) fn new(selection: &'a Selection, rules: &'a RuleSet, target: &'a str) -> Self {
        Self {
            selection,
            rules,
            target,
            included: selection
                .items
                .iter()
                .map(|item| (item.name.cxx.as_str(), item))
                .collect(),
            excluded: selection
                .excluded
                .iter()
                .map(|item| (item.name.cxx.as_str(), item))
                .collect(),
        }
    }

    pub(crate) fn included(&self, cxx: &str) -> Option<&'a Item> {
        self.included.get(cxx).copied()
    }

    pub(crate) fn excluded(&self, cxx: &str) -> Option<&'a Item> {
        self.excluded.get(cxx).copied()
    }

    pub(crate) fn resolve(&self, ty: &'a Ty) -> &'a Ty {
        let mut current = ty;
        while let Ty::Named(name) = current {
            match self.included(name).map(|item| &item.kind) {
                Some(ItemKind::Typedef(inner)) if !self.is_result_type(name) => current = inner,
                _ => break,
            }
        }
        current
    }

    pub(crate) fn is_result_type(&self, cxx: &str) -> bool {
        self.included(cxx).is_some_and(|item| {
            matches!(item.kind, ItemKind::Typedef(_))
                && item.name.rust.as_deref() == Some(self.rules.result_type)
        })
    }

    pub(crate) fn integer_range(&self, ty: &'a Ty) -> Option<(i128, i128)> {
        match self.resolve(ty) {
            Ty::Bool => Some((0, 1)),
            Ty::C(c) => c.range(),
            Ty::Prim(prim) => Some(prim.range()),
            Ty::Named(name) if self.is_result_type(name) => Some(Prim::I32.range()),
            Ty::Named(name) => match self.included(name).map(|item| &item.kind) {
                Some(ItemKind::Enum(enumeration)) => self.enum_range(enumeration),
                Some(ItemKind::Typedef(_)) => self.pointer_sized_range(name),
                _ => None,
            },
            _ => None,
        }
    }

    fn pointer_sized_range(&self, cxx: &str) -> Option<(i128, i128)> {
        let item = self.included(cxx)?;
        self.rules
            .pointer_sized(item.name.rust.as_deref()?)
            .map(Prim::range)
    }

    pub(crate) fn enum_range(&self, enumeration: &'a crate::model::Enum) -> Option<(i128, i128)> {
        match &enumeration.underlying {
            Underlying::Fixed(ty) => self.integer_range(ty),
            Underlying::Unfixed { .. } => Some(unfixed_prim(enumeration).range()),
        }
    }

    fn validate(&self) -> Result<()> {
        for item in &self.selection.items {
            self.item(item)?;
        }
        self.macros()?;
        self.result_type()?;
        Ok(())
    }

    fn unsupported(location: &Location, message: impl Into<String>) -> Error {
        Error::at(ErrorKind::Unsupported, location, message)
    }

    fn item(&self, item: &'a Item) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
        match &item.kind {
            ItemKind::Unsupported(description) => Err(Self::unsupported(
                location,
                format!("{name} is {description}, which the bindings cannot express"),
            )),
            ItemKind::Opaque => Ok(()),
            ItemKind::Typedef(ty) => {
                if let Some(prim) = item
                    .name
                    .rust
                    .as_deref()
                    .and_then(|rust| self.rules.pointer_sized(rust))
                    && self.integer_range(ty).is_none()
                {
                    return Err(Self::unsupported(
                        location,
                        format!(
                            "typedef {name} is mapped to {} but is not an integer type",
                            prim.rust()
                        ),
                    ));
                }
                self.ty(ty, Context::Typedef, location, &format!("typedef {name}"))
            }
            ItemKind::Constant(constant) => {
                let Some(value) = constant.value else {
                    return Err(Error::at(
                        ErrorKind::Evaluation,
                        location,
                        format!("cannot evaluate the constant {name}"),
                    ));
                };
                self.ty(
                    &constant.ty,
                    Context::Value,
                    location,
                    &format!("constant {name}"),
                )?;
                self.check_value(&constant.ty, value, location, &format!("constant {name}"))
            }
            ItemKind::Function(function) => {
                if !function.c_linkage {
                    return Err(Self::unsupported(
                        location,
                        format!(
                            "function {name} has C++ linkage, so the library exports it only under a mangled name that differs between compilers (SCS-3)"
                        ),
                    ));
                }
                if function.inline {
                    return Err(Self::unsupported(
                        location,
                        format!(
                            "function {name} is defined inline in the header, so the library does not export it (SCS-3)"
                        ),
                    ));
                }
                self.signature(
                    &function.signature,
                    location,
                    &format!("function {name}"),
                    true,
                )
            }
            ItemKind::Enum(enumeration) => self.enumeration(item, enumeration),
            ItemKind::Record(record) => self.record(item, record),
            ItemKind::Interface(interface) => self.interface(item, interface),
        }
    }

    fn enumeration(&self, item: &'a Item, enumeration: &'a crate::model::Enum) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
        if let Underlying::Fixed(ty) = &enumeration.underlying {
            self.ty(ty, Context::Value, location, &format!("enumeration {name}"))?;
        } else if enumeration.size != 4 {
            return Err(Self::unsupported(
                location,
                format!(
                    "enumeration {name} has no fixed underlying type and a size of {} bytes, but only 4 bytes map to u32 or i32",
                    enumeration.size
                ),
            ));
        }
        let Some((min, max)) = self.enum_range(enumeration) else {
            return Err(Self::unsupported(
                location,
                format!("enumeration {name} has an underlying type that is not an integer type"),
            ));
        };
        for enumerator in &enumeration.enumerators {
            if enumerator.value < min || enumerator.value > max {
                return Err(Error::at(
                    ErrorKind::Evaluation,
                    location,
                    format!(
                        "enumerator {}::{} has the value {}, which does not fit the underlying type of the enumeration",
                        name, enumerator.name, enumerator.value
                    ),
                ));
            }
        }
        if let Some(typedef) = &enumeration.constants_of {
            self.flags_typedef(item, enumeration, typedef)?;
        }
        Ok(())
    }

    fn flags_typedef(
        &self,
        item: &'a Item,
        enumeration: &'a crate::model::Enum,
        typedef: &str,
    ) -> Result<()> {
        let location = &item.location;
        let range = match self.included(typedef).map(|typedef| &typedef.kind) {
            Some(ItemKind::Typedef(inner)) if !self.is_result_type(typedef) => {
                self.integer_range(inner)
            }
            _ => None,
        };
        let Some((min, max)) = range else {
            return Err(Self::unsupported(
                location,
                format!(
                    "a rule assigns the unnamed enumeration that starts with {} to {typedef}, which is not an included typedef of an integer type",
                    enumeration
                        .enumerators
                        .first()
                        .map_or("no enumerator", |enumerator| enumerator.name.as_str())
                ),
            ));
        };
        for enumerator in &enumeration.enumerators {
            if enumerator.value < min || enumerator.value > max {
                return Err(Error::at(
                    ErrorKind::Evaluation,
                    location,
                    format!(
                        "enumerator {} has the value {}, which does not fit the type {typedef} that a rule assigns it to",
                        enumerator.name, enumerator.value
                    ),
                ));
            }
        }
        Ok(())
    }

    fn record(&self, item: &'a Item, record: &'a Record) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
        if record.has_bases {
            return Err(Self::unsupported(
                location,
                format!(
                    "record {name} has a base class, whose layout a Rust struct cannot express"
                ),
            ));
        }
        if record.fields.is_empty() {
            return Err(Self::unsupported(
                location,
                format!(
                    "record {name} has no fields, so it has size 1 in C++ but size 0 as a Rust struct"
                ),
            ));
        }
        for field in &record.fields {
            let field_name = field.name.as_deref().unwrap_or("(anonymous member)");
            let what = format!("field {name}::{field_name}");
            if field.bit_width.is_some() {
                return Err(Self::unsupported(
                    &field.location,
                    format!("{what} is a bit-field, which a Rust struct cannot express"),
                ));
            }
            if record.union && field.init != Init::None {
                return Err(Self::unsupported(
                    &field.location,
                    format!("{what} has a default member initializer in a union"),
                ));
            }
            self.ty(&field.ty, Context::Field, &field.location, &what)?;
            match field.init {
                Init::Null => {
                    if !matches!(
                        self.resolve(&field.ty),
                        Ty::Pointer { .. } | Ty::Callback(_)
                    ) {
                        return Err(Self::unsupported(
                            &field.location,
                            format!("{what} is initialized with nullptr but is not a pointer"),
                        ));
                    }
                }
                Init::Expression => {
                    return Err(Error::at(
                        ErrorKind::Evaluation,
                        &field.location,
                        format!("the default member initializer of {what} was not evaluated"),
                    ));
                }
                Init::Value(value) => self.check_value(&field.ty, value, &field.location, &what)?,
                Init::None | Init::Zero => {}
            }
        }
        Ok(())
    }

    fn interface(&self, item: &'a Item, interface: &'a Interface) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
        if interface.bases.len() > 1 {
            return Err(Self::unsupported(
                location,
                format!(
                    "interface {name} has {} base classes, but a vtable struct can only start with one base vtable",
                    interface.bases.len()
                ),
            ));
        }
        if interface.has_fields {
            return Err(Self::unsupported(
                location,
                format!(
                    "interface {name} has a data member, which a COM-style interface cannot have"
                ),
            ));
        }
        if interface.has_virtual_destructor {
            return Err(Self::unsupported(
                location,
                format!(
                    "interface {name} has a virtual destructor, whose vtable slots differ between compilers"
                ),
            ));
        }
        if interface.uuid.is_none() {
            return Err(Self::unsupported(
                location,
                format!(
                    "cannot read the UUID of interface {name} from the body of getTypeGuid as eleven integers"
                ),
            ));
        }
        if !matches!(
            self.selection
                .items
                .iter()
                .find(|candidate| candidate.name.rust.as_deref() == Some(self.rules.uuid_type))
                .map(|candidate| &candidate.kind),
            Some(ItemKind::Record(record)) if record.fields.len() == 4
        ) {
            return Err(Self::unsupported(
                location,
                format!(
                    "interface {name} needs the struct {} with four fields for its UUID, and the bindings do not include it",
                    self.rules.uuid_type
                ),
            ));
        }
        let mut names = HashSet::new();
        for base in self.chain(item, interface)?.iter().skip(1) {
            for method in &base.methods {
                names.insert(method.name.as_str());
            }
        }
        for method in &interface.methods {
            let what = format!("method {name}::{}", method.name);
            if !names.insert(method.name.as_str()) {
                return Err(Self::unsupported(
                    &method.location,
                    format!(
                        "{what} overloads or overrides another method of the same name, so its vtable slot cannot be told from the declaration order"
                    ),
                ));
            }
            if !method.pure {
                return Err(Self::unsupported(
                    &method.location,
                    format!(
                        "{what} is not pure virtual, so the interface is not a COM-style interface"
                    ),
                ));
            }
            if method.signature.convention != C_CONVENTION {
                return Err(Self::unsupported(
                    &method.location,
                    format!(
                        "{what} uses the calling convention {} on target {}, which differs from the C calling convention that the other methods use and that extern \"system-unwind\" means on this target",
                        method.signature.convention, self.target
                    ),
                ));
            }
            self.signature(&method.signature, &method.location, &what, false)?;
        }
        Ok(())
    }

    pub(crate) fn chain(
        &self,
        item: &'a Item,
        interface: &'a Interface,
    ) -> Result<Vec<&'a Interface>> {
        let mut chain = vec![interface];
        let mut current = interface;
        while let Some(base) = current.bases.first() {
            let Some(ItemKind::Interface(next)) = self.included(base).map(|base| &base.kind) else {
                let reason = if self.excluded(base).is_some() {
                    "which a rule excludes"
                } else {
                    "which is not an included interface"
                };
                return Err(Self::unsupported(
                    &item.location,
                    format!("interface {} derives from {base}, {reason}", item.name.cxx),
                ));
            };
            chain.push(next);
            current = next;
        }
        Ok(chain)
    }

    fn signature(
        &self,
        signature: &'a Signature,
        location: &Location,
        what: &str,
        check_convention: bool,
    ) -> Result<()> {
        if signature.variadic {
            return Err(Self::unsupported(
                location,
                format!("{what} is variadic, which the bindings cannot declare"),
            ));
        }
        if check_convention && signature.convention != C_CONVENTION {
            return Err(Self::unsupported(
                location,
                format!(
                    "{what} uses the calling convention {} on target {}, but the bindings declare it as extern \"C-unwind\"",
                    signature.convention, self.target
                ),
            ));
        }
        for (position, param) in signature.params.iter().enumerate() {
            let param_name = param.name.clone().unwrap_or_else(|| format!("#{position}"));
            self.ty(
                &param.ty,
                Context::Value,
                location,
                &format!("parameter {param_name} of {what}"),
            )?;
        }
        if signature.result != Ty::Void {
            self.ty(
                &signature.result,
                Context::Value,
                location,
                &format!("the return value of {what}"),
            )?;
        }
        Ok(())
    }

    fn ty(&self, ty: &'a Ty, context: Context, location: &Location, what: &str) -> Result<()> {
        match ty {
            Ty::Unsupported(spelling) => Err(Self::unsupported(
                location,
                format!("{what} has the type {spelling}, which the bindings cannot express"),
            )),
            Ty::Void if context == Context::Pointee => Ok(()),
            Ty::Void => Err(Self::unsupported(
                location,
                format!("{what} has the type void outside a pointer"),
            )),
            Ty::Bool | Ty::C(_) | Ty::Prim(_) => Ok(()),
            Ty::Pointer { pointee, .. } => self.ty(pointee, Context::Pointee, location, what),
            Ty::Array { element, .. } => {
                if context == Context::Value {
                    return Err(Self::unsupported(
                        location,
                        format!("{what} is an array, which C passes as a pointer"),
                    ));
                }
                self.ty(element, Context::Field, location, what)
            }
            Ty::Callback(signature) => self.signature(
                signature,
                location,
                &format!("the callback type of {what}"),
                true,
            ),
            Ty::Named(name) => self.named(name, context, location, what),
        }
    }

    fn named(&self, name: &str, context: Context, location: &Location, what: &str) -> Result<()> {
        if let Some(item) = self.included(name) {
            return match &item.kind {
                ItemKind::Typedef(inner) => {
                    if self.is_result_type(name) {
                        Ok(())
                    } else {
                        self.ty(inner, context, location, what)
                    }
                }
                ItemKind::Enum(_) => Ok(()),
                ItemKind::Record(_) if context != Context::Value => Ok(()),
                ItemKind::Record(_) => Err(Self::unsupported(
                    location,
                    format!(
                        "{what} passes the aggregate {name} by value, which the Microsoft ABI returns or passes through a hidden pointer"
                    ),
                )),
                ItemKind::Interface(_) | ItemKind::Opaque
                    if matches!(context, Context::Pointee | Context::Typedef) =>
                {
                    Ok(())
                }
                ItemKind::Interface(_) | ItemKind::Opaque => Err(Self::unsupported(
                    location,
                    format!(
                        "{what} uses {name} by value, but it can only be used through a pointer"
                    ),
                )),
                ItemKind::Function(_) | ItemKind::Constant(_) | ItemKind::Unsupported(_) => {
                    Err(Self::unsupported(
                        location,
                        format!("{what} refers to {name}, which is not a type"),
                    ))
                }
            };
        }
        if let Some(item) = self.excluded(name) {
            let pointer_only = matches!(
                item.kind,
                ItemKind::Record(_) | ItemKind::Interface(_) | ItemKind::Opaque
            );
            if pointer_only && matches!(context, Context::Pointee | Context::Typedef) {
                return Ok(());
            }
            let reason = self
                .rules
                .declaration_rule(item.location.file(), &item.name.cxx)
                .map_or("", |rule| rule.reason);
            return Err(Self::unsupported(
                location,
                format!(
                    "{what} uses {name} by value, but a rule excludes it as {reason}, so the bindings declare it only as an opaque type behind a pointer (SCS-4)"
                ),
            ));
        }
        Err(Self::unsupported(
            location,
            format!("{what} refers to {name}, which the headers do not declare"),
        ))
    }

    fn check_value(&self, ty: &'a Ty, value: Value, location: &Location, what: &str) -> Result<()> {
        let resolved = self.resolve(ty);
        match (resolved, value) {
            (Ty::C(CType::Float | CType::Double), Value::Integer(_)) => Ok(()),
            (Ty::C(ty @ (CType::Float | CType::Double)), Value::Float(float)) => {
                if !float.is_finite() {
                    return Err(Error::at(
                        ErrorKind::Evaluation,
                        location,
                        format!(
                            "{what} has the value {float:?}, which is not a finite number and has no Rust literal"
                        ),
                    ));
                }
                if *ty == CType::Float && float.abs() > f64::from(f32::MAX) {
                    return Err(Error::at(
                        ErrorKind::Evaluation,
                        location,
                        format!("{what} has the value {float:?}, which does not fit its type f32"),
                    ));
                }
                Ok(())
            }
            (Ty::Pointer { .. } | Ty::Callback(_), _) => Err(Self::unsupported(
                location,
                format!("{what} is a pointer initialized with a value other than nullptr"),
            )),
            (_, Value::Integer(integer)) => match self.integer_range(ty) {
                Some((min, max)) if integer >= min && integer <= max => Ok(()),
                Some(_) => Err(Error::at(
                    ErrorKind::Evaluation,
                    location,
                    format!("{what} has the value {integer}, which does not fit its type"),
                )),
                None => Err(Self::unsupported(
                    location,
                    format!("{what} has a value but its type is not an integer type"),
                )),
            },
            (_, Value::Float(_)) => Err(Self::unsupported(
                location,
                format!("{what} has a floating-point value but an integer type"),
            )),
        }
    }

    fn macros(&self) -> Result<()> {
        for selected in &self.selection.macros {
            let what = format!(
                "macro {}, which a rule includes as {}",
                selected.name, selected.reason
            );
            let Some(Value::Integer(value)) = selected.value else {
                return Err(Error::at(
                    ErrorKind::Evaluation,
                    &selected.location,
                    format!("{what} has no integer value"),
                ));
            };
            let fits = match selected.action {
                MacroAction::ResultCode => {
                    (i128::from(i32::MIN)..=i128::from(u32::MAX)).contains(&value)
                }
                MacroAction::Constant(prim) => {
                    let (min, max) = prim.range();
                    (min..=max).contains(&value)
                }
                MacroAction::Exclude => true,
            };
            if !fits {
                return Err(Error::at(
                    ErrorKind::Evaluation,
                    &selected.location,
                    format!("{what} has the value {value}, which does not fit its type"),
                ));
            }
            if selected.action == MacroAction::ResultCode
                && !self
                    .selection
                    .items
                    .iter()
                    .any(|item| self.is_result_type(&item.name.cxx))
            {
                return Err(Self::unsupported(
                    &selected.location,
                    format!(
                        "{what} is a result code, but the bindings do not include the typedef {}",
                        self.rules.result_type
                    ),
                ));
            }
        }
        Ok(())
    }

    fn result_type(&self) -> Result<()> {
        for item in &self.selection.items {
            if !self.is_result_type(&item.name.cxx) {
                continue;
            }
            let ItemKind::Typedef(ty) = &item.kind else {
                continue;
            };
            if !matches!(self.resolve(ty), Ty::Prim(Prim::I32) | Ty::C(CType::Int)) {
                return Err(Self::unsupported(
                    &item.location,
                    format!(
                        "the result type {} is not a typedef of a 32-bit signed integer",
                        item.name.cxx
                    ),
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn unfixed_prim(enumeration: &crate::model::Enum) -> Prim {
    if enumeration
        .enumerators
        .iter()
        .any(|enumerator| enumerator.value < 0)
    {
        Prim::I32
    } else {
        Prim::U32
    }
}

fn normalized(selection: &Selection) -> Selection {
    let mut copy = selection.clone();
    for item in copy.items.iter_mut().chain(copy.excluded.iter_mut()) {
        if let ItemKind::Enum(enumeration) = &mut item.kind
            && let Underlying::Unfixed { canonical_signed } = &mut enumeration.underlying
        {
            *canonical_signed = false;
        }
    }
    copy
}

pub(crate) fn compare(selections: &[(&str, Selection)]) -> Result<Selection> {
    let Some(((first_target, first), rest)) = selections.split_first() else {
        return Err(Error::new(
            ErrorKind::TargetDifference,
            "no target was parsed",
        ));
    };
    let reference = normalized(first);
    for (target, selection) in rest {
        let other = normalized(selection);
        if other == reference {
            continue;
        }
        return Err(difference(first_target, &reference, target, &other));
    }
    Ok(reference)
}

fn difference(first_target: &str, first: &Selection, target: &str, other: &Selection) -> Error {
    let between = format!("between {first_target} and {target}");
    for position in 0..first.items.len().max(other.items.len()) {
        match (first.items.get(position), other.items.get(position)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(a), Some(b)) => {
                return Error::at(
                    ErrorKind::TargetDifference,
                    &a.location,
                    format!(
                        "declaration {} differs {between}, and no rule accepts the difference: {}",
                        a.name.cxx,
                        first_difference(&format!("{a:?}"), &format!("{b:?}"))
                    ),
                );
            }
            (Some(item), None) | (None, Some(item)) => {
                return Error::at(
                    ErrorKind::TargetDifference,
                    &item.location,
                    format!(
                        "declaration {} exists for only one of the targets {first_target} and {target}",
                        item.name.cxx
                    ),
                );
            }
            (None, None) => {}
        }
    }
    let first_text = format!("{:?} {:?}", first.macros, first.excluded);
    let other_text = format!("{:?} {:?}", other.macros, other.excluded);
    Error::new(
        ErrorKind::TargetDifference,
        format!(
            "the macros or the excluded declarations differ {between}: {}",
            first_difference(&first_text, &other_text)
        ),
    )
}

fn first_difference(first: &str, other: &str) -> String {
    let start = first
        .char_indices()
        .zip(other.chars())
        .find(|((_, a), b)| a != b)
        .map_or_else(|| first.len().min(other.len()), |((index, _), _)| index);
    let excerpt = |text: &str| -> String {
        let before = text.get(..start).unwrap_or(text);
        let from = before
            .char_indices()
            .rev()
            .nth(60)
            .map_or(0, |(index, _)| index);
        text.get(from..).unwrap_or(text).chars().take(160).collect()
    };
    format!("`{}` versus `{}`", excerpt(first), excerpt(other))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Constant, Enumerator, Field, Name};
    use crate::rules::SLANG;

    fn item(cxx: &str, kind: ItemKind) -> Item {
        Item {
            name: Name {
                cxx: cxx.to_owned(),
                rust: Some(cxx.to_owned()),
            },
            location: Location::new("slang.h", 1),
            owner: None,
            kind,
        }
    }

    fn unfixed(canonical_signed: bool, values: &[i128]) -> ItemKind {
        ItemKind::Enum(unfixed_enum(canonical_signed, values))
    }

    fn unfixed_enum(canonical_signed: bool, values: &[i128]) -> crate::model::Enum {
        crate::model::Enum {
            unnamed: false,
            constants_of: None,
            underlying: Underlying::Unfixed { canonical_signed },
            size: 4,
            enumerators: values
                .iter()
                .enumerate()
                .map(|(position, value)| Enumerator {
                    name: format!("E{position}"),
                    value: *value,
                })
                .collect(),
        }
    }

    fn selection(items: Vec<Item>) -> Selection {
        Selection {
            items,
            excluded: Vec::new(),
            macros: Vec::new(),
        }
    }

    fn constant(ty: Ty, value: Value) -> Item {
        item(
            "C",
            ItemKind::Constant(Constant {
                ty,
                value: Some(value),
            }),
        )
    }

    fn refusal(items: Vec<Item>) -> Error {
        validate(&selection(items), &SLANG, "test-target").expect_err("validation fails")
    }

    fn assert_refusal(error: &Error, kind: ErrorKind, message: &str) {
        assert_eq!(error.kind(), kind, "{error}");
        assert_eq!(
            error.location(),
            Some(&Location::new("slang.h", 1)),
            "{error}"
        );
        assert_eq!(error.message(), message);
    }

    #[test]
    fn validate_refuses_a_floating_point_value_on_an_integer_type() {
        let error = refusal(vec![constant(Ty::Prim(Prim::I32), Value::Float(1.5))]);
        assert_refusal(
            &error,
            ErrorKind::Unsupported,
            "constant C has a floating-point value but an integer type",
        );
    }

    #[test]
    fn validate_refuses_a_pointer_with_a_value_other_than_nullptr() {
        let pointer = Ty::Pointer {
            pointee: Box::new(Ty::Void),
            is_const: true,
        };
        let error = refusal(vec![constant(pointer, Value::Integer(8))]);
        assert_refusal(
            &error,
            ErrorKind::Unsupported,
            "constant C is a pointer initialized with a value other than nullptr",
        );
    }

    #[test]
    fn validate_refuses_an_integer_value_on_a_type_that_is_not_an_integer() {
        let record = item(
            "R",
            ItemKind::Record(Record {
                union: false,
                size: 4,
                align: 4,
                fields: vec![Field {
                    name: Some("value".to_owned()),
                    location: Location::new("slang.h", 1),
                    ty: Ty::Prim(Prim::I32),
                    offset_bits: 0,
                    bit_width: None,
                    init: Init::None,
                }],
                has_bases: false,
            }),
        );
        let holder = item(
            "H",
            ItemKind::Record(Record {
                union: false,
                size: 4,
                align: 4,
                fields: vec![Field {
                    name: Some("inner".to_owned()),
                    location: Location::new("slang.h", 1),
                    ty: Ty::Named("R".to_owned()),
                    offset_bits: 0,
                    bit_width: None,
                    init: Init::Value(Value::Integer(1)),
                }],
                has_bases: false,
            }),
        );
        let error = refusal(vec![record, holder]);
        assert_refusal(
            &error,
            ErrorKind::Unsupported,
            "field H::inner has a value but its type is not an integer type",
        );
    }

    #[test]
    fn validate_refuses_an_enumeration_whose_underlying_type_is_not_an_integer() {
        let enumeration = item(
            "E",
            ItemKind::Enum(crate::model::Enum {
                unnamed: false,
                constants_of: None,
                underlying: Underlying::Fixed(Ty::C(CType::Float)),
                size: 4,
                enumerators: Vec::new(),
            }),
        );
        let error = refusal(vec![enumeration]);
        assert_refusal(
            &error,
            ErrorKind::Unsupported,
            "enumeration E has an underlying type that is not an integer type",
        );
    }

    #[test]
    fn validate_refuses_a_value_that_does_not_fit_f32() {
        let error = refusal(vec![constant(Ty::C(CType::Float), Value::Float(1e300))]);
        assert_refusal(
            &error,
            ErrorKind::Evaluation,
            "constant C has the value 1e300, which does not fit its type f32",
        );
    }

    #[test]
    fn validate_accepts_a_finite_value_that_fits_f32_or_f64() {
        validate(
            &selection(vec![
                constant(Ty::C(CType::Float), Value::Float(f64::from(f32::MAX))),
                constant(Ty::C(CType::Double), Value::Float(1e300)),
            ]),
            &SLANG,
            "test-target",
        )
        .expect("the values fit");
    }

    #[test]
    fn compare_accepts_a_different_signedness_of_an_unfixed_enumeration() {
        let linux = selection(vec![item("E", unfixed(false, &[1, 2]))]);
        let windows = selection(vec![item("E", unfixed(true, &[1, 2]))]);
        let merged = compare(&[("linux", linux), ("windows", windows)]).expect("accepted");
        assert_eq!(merged.items.len(), 1);
    }

    #[test]
    fn compare_rejects_a_different_layout_and_names_the_declaration_and_targets() {
        let record = |size| {
            ItemKind::Record(Record {
                union: false,
                size,
                align: 4,
                fields: Vec::new(),
                has_bases: false,
            })
        };
        let linux = selection(vec![item("S", record(4))]);
        let windows = selection(vec![item("S", record(8))]);
        let error = compare(&[("linux", linux), ("windows", windows)]).expect_err("rejected");
        assert_eq!(error.kind(), ErrorKind::TargetDifference);
        assert!(
            error
                .message()
                .contains("declaration S differs between linux and windows")
        );
        assert!(error.message().contains("size: 4"));
        assert!(error.message().contains("size: 8"));
    }

    #[test]
    fn compare_rejects_a_declaration_that_exists_for_one_target_only() {
        let linux = selection(vec![item("A", ItemKind::Opaque)]);
        let windows = selection(Vec::new());
        let error = compare(&[("linux", linux), ("windows", windows)]).expect_err("rejected");
        assert!(
            error
                .message()
                .contains("exists for only one of the targets")
        );
    }

    #[test]
    fn compare_rejects_a_macro_value_that_differs_between_targets() {
        let with_value = |value| Selection {
            macros: vec![SelectedMacro {
                name: "WIDTH".to_owned(),
                location: Location::new("slang.h", 1),
                action: MacroAction::Constant(Prim::I32),
                reason: "a width",
                value: Some(Value::Integer(value)),
            }],
            ..selection(Vec::new())
        };
        let error =
            compare(&[("linux", with_value(1)), ("windows", with_value(2))]).expect_err("rejected");
        assert_eq!(error.kind(), ErrorKind::TargetDifference);
        assert_eq!(error.location(), None);
        assert!(
            error.message().starts_with(
                "the macros or the excluded declarations differ between linux and windows: "
            ),
            "{error}"
        );
        assert!(error.message().contains("Integer(1)"), "{error}");
        assert!(error.message().contains("Integer(2)"), "{error}");
    }

    #[test]
    fn compare_rejects_an_excluded_declaration_that_differs_between_targets() {
        let excluding = |name| Selection {
            excluded: vec![item(name, ItemKind::Opaque)],
            ..selection(Vec::new())
        };
        let error = compare(&[("linux", excluding("A")), ("windows", excluding("B"))])
            .expect_err("rejected");
        assert_eq!(error.kind(), ErrorKind::TargetDifference);
        assert!(
            error.message().starts_with(
                "the macros or the excluded declarations differ between linux and windows: "
            ),
            "{error}"
        );
    }

    #[test]
    fn compare_rejects_an_empty_list_of_targets() {
        let error = compare(&[]).expect_err("rejected");
        assert_eq!(error.kind(), ErrorKind::TargetDifference);
        assert_eq!(error.message(), "no target was parsed");
    }

    #[test]
    fn unfixed_prim_is_unsigned_unless_an_enumerator_is_negative() {
        assert_eq!(unfixed_prim(&unfixed_enum(true, &[0, 1])), Prim::U32);
        assert_eq!(unfixed_prim(&unfixed_enum(false, &[-1, 1])), Prim::I32);
    }

    #[test]
    fn first_difference_shows_both_sides_near_the_change() {
        let text = first_difference("abc size: 4 xyz", "abc size: 8 xyz");
        assert_eq!(text, "`abc size: 4 xyz` versus `abc size: 8 xyz`");
    }
}
