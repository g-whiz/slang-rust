use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use clang::diagnostic::Severity;
use clang::{
    Entity, EntityKind, EvaluationResult, Index, TranslationUnit, Type, TypeKind, Unsaved,
};

use crate::check::Selection;
use crate::emit::put;
use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::{
    CType, Constant, Enum, Enumerator, Field, Function, Init, Interface, Item, ItemKind, Macro,
    Method, Model, Name, Param, Prim, Record, Signature, Ty, Underlying, Value,
};

pub(crate) const TARGETS: [&str; 3] = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
];

const EVALUATION_FILE: &str = "__bindings_generator_evaluate.cpp";
const EVALUATION_PREFIX: &str = "__bindings_generator_value_";

fn shim_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("shim")
}

fn arguments(target: &str, header_dir: &Path) -> Vec<String> {
    vec![
        "-x".to_owned(),
        "c++".to_owned(),
        "-std=c++17".to_owned(),
        "-target".to_owned(),
        target.to_owned(),
        "-nostdinc".to_owned(),
        "-isystem".to_owned(),
        shim_dir().display().to_string(),
        format!("-I{}", header_dir.display()),
    ]
}

pub(crate) fn parse(
    index: &Index<'_>,
    target: &str,
    header_dir: &Path,
    main_header: &str,
) -> Result<Model> {
    let path = header_dir.join(main_header);
    let tu = index
        .parser(&path)
        .arguments(&arguments(target, header_dir))
        .detailed_preprocessing_record(true)
        .parse()
        .map_err(|error| {
            Error::new(
                ErrorKind::Libclang,
                format!("libclang cannot parse {main_header} for target {target}"),
            )
            .with_source(error.to_string())
        })?;
    check_diagnostics(&tu, target, header_dir, |_| None)?;
    let mut extractor = Extractor {
        header_dir,
        files: HashMap::new(),
        model: Model::default(),
        seen: HashSet::new(),
        macros_seen: HashSet::new(),
    };
    extractor.walk(tu.get_entity())?;
    Ok(extractor.model)
}

fn check_diagnostics(
    tu: &TranslationUnit<'_>,
    target: &str,
    header_dir: &Path,
    explain: impl Fn(u32) -> Option<(Location, String)>,
) -> Result<()> {
    let Some(diagnostic) = tu
        .get_diagnostics()
        .into_iter()
        .find(|diagnostic| diagnostic.get_severity() >= Severity::Error)
    else {
        return Ok(());
    };
    let place = diagnostic.get_location().get_file_location();
    let file = place
        .file
        .map(|file| file.get_path())
        .and_then(|path| file_name_in(header_dir, &path));
    if let Some((location, what)) = place
        .file
        .filter(|file| file.get_path().ends_with(EVALUATION_FILE))
        .and_then(|_| explain(place.line))
    {
        return Err(Error::at(
            ErrorKind::Evaluation,
            &location,
            format!(
                "cannot evaluate {what} for target {target}: {}",
                diagnostic.get_text()
            ),
        ));
    }
    let message = format!(
        "libclang reported an error while parsing for target {target}: {}",
        diagnostic.get_text()
    );
    Err(match file {
        Some(file) => Error::at(
            ErrorKind::Diagnostic,
            &Location::new(file, place.line),
            message,
        ),
        None => Error::new(ErrorKind::Diagnostic, message),
    })
}

fn file_name_in(header_dir: &Path, path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    if canonical.parent()? != header_dir {
        return None;
    }
    Some(canonical.file_name()?.to_string_lossy().into_owned())
}

struct Extractor<'a> {
    header_dir: &'a Path,
    files: HashMap<PathBuf, Option<String>>,
    model: Model,
    seen: HashSet<String>,
    macros_seen: HashSet<String>,
}

impl Extractor<'_> {
    fn location(&mut self, entity: Entity<'_>) -> Option<Location> {
        let place = entity.get_location()?.get_file_location();
        let path = place.file?.get_path();
        let header_dir = self.header_dir;
        let file = self
            .files
            .entry(path)
            .or_insert_with_key(|path| file_name_in(header_dir, path))
            .clone()?;
        Some(Location::new(file, place.line))
    }

    fn in_headers(&mut self, entity: Entity<'_>) -> bool {
        self.location(entity).is_some()
    }

    fn push(&mut self, item: Item) {
        self.seen.insert(item.name.cxx.clone());
        self.model.items.push(item);
    }

    fn walk(&mut self, parent: Entity<'_>) -> Result<()> {
        for child in parent.get_children() {
            match child.get_kind() {
                EntityKind::MacroDefinition => self.macro_definition(child),
                EntityKind::MacroExpansion | EntityKind::InclusionDirective => {}
                _ => {
                    if let Some(location) = self.location(child) {
                        self.declaration(child, &location)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn macro_definition(&mut self, entity: Entity<'_>) {
        if entity.is_function_like_macro() || entity.is_builtin_macro() {
            return;
        }
        let Some(location) = self.location(entity) else {
            return;
        };
        let Some(name) = entity.get_name() else {
            return;
        };
        if self.macros_seen.insert(name.clone()) {
            self.model.macros.push(Macro { name, location });
        }
    }

    fn declaration(&mut self, entity: Entity<'_>, location: &Location) -> Result<()> {
        match entity.get_kind() {
            EntityKind::Namespace | EntityKind::LinkageSpec | EntityKind::UnexposedDecl => {
                self.walk(entity)
            }
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl => {
                self.record_declaration(entity, location)
            }
            EntityKind::EnumDecl => {
                if entity.is_definition() {
                    self.enumeration(entity, location);
                }
                Ok(())
            }
            EntityKind::TypedefDecl | EntityKind::TypeAliasDecl => {
                self.typedef(entity, location);
                Ok(())
            }
            EntityKind::FunctionDecl => {
                self.function(entity, location);
                Ok(())
            }
            EntityKind::VarDecl => {
                self.variable(entity, location);
                Ok(())
            }
            kind => {
                let name = qualified_name(entity);
                let description = format!("a declaration of kind {kind:?}");
                if !self.seen.contains(&name.cxx) {
                    self.push(Item {
                        name,
                        location: location.clone(),
                        owner: None,
                        kind: ItemKind::Unsupported(description),
                    });
                }
                Ok(())
            }
        }
    }

    fn record_declaration(&mut self, entity: Entity<'_>, location: &Location) -> Result<()> {
        if !entity.is_definition() {
            if entity.get_definition().is_none() {
                self.opaque(entity, location);
            }
            return Ok(());
        }
        let name = qualified_name(entity);
        if self.seen.contains(&name.cxx) {
            return Ok(());
        }
        if is_anonymous(entity) {
            self.push(Item {
                name,
                location: location.clone(),
                owner: None,
                kind: ItemKind::Unsupported("an anonymous record outside a record".to_owned()),
            });
            return Ok(());
        }
        let children = entity.get_children();
        let has_uuid = children.iter().any(|child| {
            child.get_kind() == EntityKind::Method
                && child.get_name().as_deref() == Some("getTypeGuid")
                && child.is_static_method()
        });
        let has_virtual = children.iter().any(|child| {
            matches!(
                child.get_kind(),
                EntityKind::Method | EntityKind::Destructor
            ) && child.is_virtual_method()
        });
        let has_bases = children
            .iter()
            .any(|child| child.get_kind() == EntityKind::BaseSpecifier);
        self.nested_declarations(&children)?;
        let kind = if has_uuid && (has_virtual || has_bases) {
            ItemKind::Interface(self.interface(&children))
        } else if has_virtual {
            ItemKind::Unsupported("a class with virtual methods and no UUID".to_owned())
        } else {
            ItemKind::Record(self.record(entity, &name, location, has_bases)?)
        };
        self.push(Item {
            name,
            location: location.clone(),
            owner: None,
            kind,
        });
        Ok(())
    }

    fn nested_declarations(&mut self, children: &[Entity<'_>]) -> Result<()> {
        for child in children {
            let kind = child.get_kind();
            let nested = matches!(
                kind,
                EntityKind::StructDecl
                    | EntityKind::ClassDecl
                    | EntityKind::UnionDecl
                    | EntityKind::EnumDecl
                    | EntityKind::TypedefDecl
                    | EntityKind::TypeAliasDecl
            );
            let record = matches!(
                kind,
                EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
            );
            if !nested || record && (is_anonymous(*child) || !child.is_definition()) {
                continue;
            }
            if let Some(location) = self.location(*child) {
                self.declaration(*child, &location)?;
            }
        }
        Ok(())
    }

    fn opaque(&mut self, entity: Entity<'_>, location: &Location) {
        let name = qualified_name(entity);
        if !self.seen.contains(&name.cxx) {
            self.push(Item {
                name,
                location: location.clone(),
                owner: None,
                kind: ItemKind::Opaque,
            });
        }
    }

    fn interface(&mut self, children: &[Entity<'_>]) -> Interface {
        let mut interface = Interface {
            bases: Vec::new(),
            uuid: Vec::new(),
            methods: Vec::new(),
            has_fields: false,
            has_virtual_destructor: false,
        };
        for child in children {
            match child.get_kind() {
                EntityKind::BaseSpecifier => {
                    let base = child
                        .get_type()
                        .and_then(|ty| ty.get_canonical_type().get_declaration())
                        .map_or_else(String::new, |base| qualified_name(base).cxx);
                    interface.bases.push(base);
                }
                EntityKind::FieldDecl => interface.has_fields = true,
                EntityKind::Destructor => {
                    interface.has_virtual_destructor |= child.is_virtual_method();
                }
                EntityKind::Method if child.is_virtual_method() => {
                    let location = self
                        .location(*child)
                        .unwrap_or_else(|| Location::new("<unknown>", 0));
                    interface.methods.push(Method {
                        name: child.get_name().unwrap_or_default(),
                        location,
                        signature: self.signature(*child),
                        pure: child.is_pure_virtual_method(),
                    });
                }
                EntityKind::Method if child.get_name().as_deref() == Some("getTypeGuid") => {
                    interface.uuid = uuid(*child);
                }
                _ => {}
            }
        }
        interface
    }

    fn record(
        &mut self,
        entity: Entity<'_>,
        name: &Name,
        location: &Location,
        has_bases: bool,
    ) -> Result<Record> {
        let ty = entity.get_type().ok_or_else(|| {
            Error::at(
                ErrorKind::Libclang,
                location,
                format!("libclang reports no type for {}", name.cxx),
            )
        })?;
        let layout_error = |what: &str| {
            Error::at(
                ErrorKind::Unsupported,
                location,
                format!("libclang cannot report the {what} of {}", name.cxx),
            )
        };
        let size = ty
            .get_sizeof()
            .map_err(|error| layout_error("size").with_source(error))?;
        let align = ty
            .get_alignof()
            .map_err(|error| layout_error("alignment").with_source(error))?;
        let mut fields = Vec::new();
        for field in ty.get_fields().unwrap_or_default() {
            let field_location = self.location(field).unwrap_or_else(|| location.clone());
            let field_type = field
                .get_type()
                .ok_or_else(|| layout_error("field types"))?;
            let anonymous = field_type
                .get_canonical_type()
                .get_declaration()
                .filter(|declaration| is_anonymous(*declaration));
            if let Some(declaration) = anonymous {
                self.anonymous_member(declaration, name, &field_location)?;
            }
            let offset_bits = field
                .get_offset_of_field()
                .map_err(|error| layout_error("field offsets").with_source(error))?;
            fields.push(Field {
                name: if anonymous.is_some() {
                    None
                } else {
                    field.get_name()
                },
                location: field_location,
                ty: self.ty(field_type),
                offset_bits,
                bit_width: if field.is_bit_field() {
                    field.get_bit_field_width()
                } else {
                    None
                },
                init: if anonymous.is_some() {
                    Init::None
                } else {
                    initializer(field)
                },
            });
        }
        Ok(Record {
            union: entity.get_kind() == EntityKind::UnionDecl,
            size,
            align,
            fields,
            has_bases,
        })
    }

    fn anonymous_member(
        &mut self,
        declaration: Entity<'_>,
        owner: &Name,
        location: &Location,
    ) -> Result<()> {
        let name = qualified_name(declaration);
        if self.seen.contains(&name.cxx) {
            return Ok(());
        }
        let children = declaration.get_children();
        self.nested_declarations(&children)?;
        let has_bases = children
            .iter()
            .any(|child| child.get_kind() == EntityKind::BaseSpecifier);
        let record = self.record(declaration, &name, location, has_bases)?;
        self.push(Item {
            name,
            location: location.clone(),
            owner: Some(owner.cxx.clone()),
            kind: ItemKind::Record(record),
        });
        Ok(())
    }

    fn enumeration(&mut self, entity: Entity<'_>, location: &Location) {
        let name = qualified_name(entity);
        if self.seen.contains(&name.cxx) {
            return;
        }
        let underlying_type = entity.get_enum_underlying_type();
        let canonical_signed =
            underlying_type.is_some_and(|ty| ty.get_canonical_type().is_signed_integer());
        let underlying = match underlying_type {
            Some(ty) if entity.is_scoped() || has_fixed_underlying_type(entity) => {
                Underlying::Fixed(self.ty(ty))
            }
            _ => Underlying::Unfixed { canonical_signed },
        };
        let enumerators = entity
            .get_children()
            .into_iter()
            .filter(|child| child.get_kind() == EntityKind::EnumConstantDecl)
            .map(|child| {
                let (signed, unsigned) = child.get_enum_constant_value().unwrap_or_default();
                Enumerator {
                    name: child.get_name().unwrap_or_default(),
                    value: if canonical_signed {
                        i128::from(signed)
                    } else {
                        i128::from(unsigned)
                    },
                }
            })
            .collect();
        let size = entity
            .get_type()
            .and_then(|ty| ty.get_sizeof().ok())
            .unwrap_or_default();
        self.push(Item {
            name,
            location: location.clone(),
            owner: None,
            kind: ItemKind::Enum(Enum {
                underlying,
                size,
                enumerators,
            }),
        });
    }

    fn typedef(&mut self, entity: Entity<'_>, location: &Location) {
        let name = qualified_name(entity);
        if self.seen.contains(&name.cxx) {
            return;
        }
        let ty = entity.get_typedef_underlying_type().map_or_else(
            || Ty::Unsupported("an unknown type".to_owned()),
            |ty| self.ty(ty),
        );
        self.push(Item {
            name,
            location: location.clone(),
            owner: None,
            kind: ItemKind::Typedef(ty),
        });
    }

    fn function(&mut self, entity: Entity<'_>, location: &Location) {
        let name = qualified_name(entity);
        if self.seen.contains(&name.cxx) {
            return;
        }
        let plain = entity.get_name().unwrap_or_default();
        let c_linkage = entity
            .get_mangled_name()
            .is_some_and(|mangled| mangled == plain || mangled.strip_prefix('_') == Some(&plain));
        let function = Function {
            signature: self.signature(entity),
            c_linkage,
            inline: entity.is_definition() || entity.is_inline_function(),
        };
        self.push(Item {
            name,
            location: location.clone(),
            owner: None,
            kind: ItemKind::Function(function),
        });
    }

    fn variable(&mut self, entity: Entity<'_>, location: &Location) {
        let name = qualified_name(entity);
        if self.seen.contains(&name.cxx) {
            return;
        }
        let ty = entity.get_type();
        let kind = match ty {
            Some(ty) if ty.is_const_qualified() => ItemKind::Constant(Constant {
                ty: self.ty(ty),
                value: entity.evaluate().as_ref().and_then(value),
            }),
            _ => ItemKind::Unsupported("a variable that is not a compile-time constant".to_owned()),
        };
        self.push(Item {
            name,
            location: location.clone(),
            owner: None,
            kind,
        });
    }

    fn signature(&mut self, entity: Entity<'_>) -> Signature {
        let params = entity
            .get_arguments()
            .unwrap_or_default()
            .into_iter()
            .map(|argument| Param {
                name: argument.get_name().filter(|name| !name.is_empty()),
                ty: argument.get_type().map_or_else(
                    || Ty::Unsupported("an unknown type".to_owned()),
                    |ty| self.ty(ty),
                ),
            })
            .collect();
        let function_type = entity.get_type();
        Signature {
            params,
            result: entity.get_result_type().map_or(Ty::Void, |ty| self.ty(ty)),
            variadic: entity.is_variadic(),
            convention: convention(function_type),
        }
    }

    fn callback(&mut self, function_type: Type<'_>) -> Signature {
        let params = function_type
            .get_argument_types()
            .unwrap_or_default()
            .into_iter()
            .map(|ty| Param {
                name: None,
                ty: self.ty(ty),
            })
            .collect();
        Signature {
            params,
            result: function_type
                .get_result_type()
                .map_or(Ty::Void, |ty| self.ty(ty)),
            variadic: function_type.is_variadic(),
            convention: convention(Some(function_type)),
        }
    }

    fn ty(&mut self, ty: Type<'_>) -> Ty {
        match ty.get_kind() {
            TypeKind::Elaborated => ty
                .get_elaborated_type()
                .map_or_else(|| unsupported(ty), |inner| self.ty(inner)),
            TypeKind::Typedef => {
                let Some(declaration) = ty.get_declaration() else {
                    return unsupported(ty);
                };
                if self.in_headers(declaration) {
                    return Ty::Named(qualified_name(declaration).cxx);
                }
                declaration
                    .get_name()
                    .and_then(|name| Prim::from_std_typedef(&name))
                    .map_or_else(|| unsupported(ty), Ty::Prim)
            }
            TypeKind::Record | TypeKind::Enum => {
                let Some(declaration) = ty.get_declaration() else {
                    return unsupported(ty);
                };
                let Some(location) = self.location(declaration) else {
                    return unsupported(ty);
                };
                if ty.get_kind() == TypeKind::Record && declaration.get_definition().is_none() {
                    self.opaque(declaration, &location);
                }
                Ty::Named(qualified_name(declaration).cxx)
            }
            TypeKind::Pointer | TypeKind::LValueReference | TypeKind::RValueReference => {
                let Some(pointee) = ty.get_pointee_type() else {
                    return unsupported(ty);
                };
                if matches!(
                    pointee.get_canonical_type().get_kind(),
                    TypeKind::FunctionPrototype | TypeKind::FunctionNoPrototype
                ) {
                    return Ty::Callback(Box::new(self.callback(pointee)));
                }
                Ty::Pointer {
                    is_const: pointee.is_const_qualified(),
                    pointee: Box::new(self.ty(pointee)),
                }
            }
            TypeKind::ConstantArray => match (ty.get_element_type(), ty.get_size()) {
                (Some(element), Some(len)) => Ty::Array {
                    element: Box::new(self.ty(element)),
                    len,
                },
                _ => unsupported(ty),
            },
            TypeKind::Void => Ty::Void,
            TypeKind::Bool => Ty::Bool,
            TypeKind::CharS | TypeKind::CharU => Ty::C(CType::Char),
            TypeKind::SChar => Ty::C(CType::SChar),
            TypeKind::UChar => Ty::C(CType::UChar),
            TypeKind::Short => Ty::C(CType::Short),
            TypeKind::UShort => Ty::C(CType::UShort),
            TypeKind::Int => Ty::C(CType::Int),
            TypeKind::UInt => Ty::C(CType::UInt),
            TypeKind::Long => Ty::C(CType::Long),
            TypeKind::ULong => Ty::C(CType::ULong),
            TypeKind::LongLong => Ty::C(CType::LongLong),
            TypeKind::ULongLong => Ty::C(CType::ULongLong),
            TypeKind::Float => Ty::C(CType::Float),
            TypeKind::Double => Ty::C(CType::Double),
            _ => unsupported(ty),
        }
    }
}

fn qualified_name(entity: Entity<'_>) -> Name {
    let mut cxx = Vec::new();
    let mut rust = Vec::new();
    let (own_cxx, own_rust) = segment(entity);
    cxx.push(own_cxx);
    rust.push(own_rust);
    let mut current = entity;
    while let Some(parent) = current.get_semantic_parent() {
        match parent.get_kind() {
            EntityKind::TranslationUnit => break,
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl => {
                let (parent_cxx, parent_rust) = segment(parent);
                cxx.push(parent_cxx);
                rust.push(parent_rust);
            }
            EntityKind::Namespace => {
                cxx.push(parent.get_name().unwrap_or_default());
            }
            _ => {}
        }
        current = parent;
    }
    cxx.reverse();
    rust.reverse();
    let unnamed = rust.iter().any(Option::is_none);
    Name {
        cxx: cxx.join("::"),
        rust: if unnamed {
            None
        } else {
            Some(rust.into_iter().flatten().collect::<Vec<_>>().join("_"))
        },
    }
}

fn unsupported(ty: Type<'_>) -> Ty {
    Ty::Unsupported(ty.get_display_name())
}

fn convention(ty: Option<Type<'_>>) -> String {
    ty.and_then(|ty| ty.get_calling_convention()).map_or_else(
        || "unknown".to_owned(),
        |convention| format!("{convention:?}"),
    )
}

fn is_anonymous(entity: Entity<'_>) -> bool {
    entity
        .get_name()
        .is_none_or(|name| name.is_empty() || name.starts_with('('))
}

fn segment(entity: Entity<'_>) -> (String, Option<String>) {
    if !is_anonymous(entity) {
        let name = entity.get_name().unwrap_or_default();
        return (name.clone(), Some(name));
    }
    let is_record = matches!(
        entity.get_kind(),
        EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
    );
    let index = entity
        .get_semantic_parent()
        .and_then(|parent| parent.get_type())
        .and_then(|parent| parent.get_fields())
        .and_then(|fields| {
            fields
                .iter()
                .filter_map(|field| {
                    field
                        .get_type()
                        .and_then(|ty| ty.get_canonical_type().get_declaration())
                })
                .filter(|declaration| is_anonymous(*declaration))
                .position(|declaration| declaration == entity)
        });
    if let (true, Some(index)) = (is_record, index) {
        return (format!("(anonymous {index})"), Some(format!("anon{index}")));
    }
    let line = entity
        .get_location()
        .map_or(0, |location| location.get_file_location().line);
    (format!("({:?} at line {line})", entity.get_kind()), None)
}

fn has_fixed_underlying_type(entity: Entity<'_>) -> bool {
    let Some(range) = entity.get_range() else {
        return false;
    };
    for token in range.tokenize() {
        match token.get_spelling().as_str() {
            "{" => return false,
            ":" => return true,
            _ => {}
        }
    }
    false
}

fn initializer(field: Entity<'_>) -> Init {
    let Some(range) = field.get_range() else {
        return Init::None;
    };
    let tokens: Vec<String> = range
        .tokenize()
        .iter()
        .map(clang::token::Token::get_spelling)
        .collect();
    let mut depth = 0_usize;
    let mut equals = None;
    for (index, token) in tokens.iter().enumerate() {
        match token.as_str() {
            "[" | "(" => depth += 1,
            "]" | ")" => depth = depth.saturating_sub(1),
            "=" if depth == 0 => {
                equals = Some(index);
                break;
            }
            _ => {}
        }
    }
    let Some(equals) = equals else {
        return Init::None;
    };
    match &tokens[equals + 1..] {
        [null] if null == "nullptr" => Init::Null,
        [open, close] if open == "{" && close == "}" => Init::Zero,
        _ => Init::Expression,
    }
}

fn uuid(method: Entity<'_>) -> Vec<u64> {
    fn find_init_list(entity: Entity<'_>) -> Option<Entity<'_>> {
        if entity.get_kind() == EntityKind::InitListExpr {
            return Some(entity);
        }
        entity.get_children().into_iter().find_map(find_init_list)
    }
    fn flatten(entity: Entity<'_>, values: &mut Vec<Option<u64>>) {
        for child in entity.get_children() {
            if child.get_kind() == EntityKind::InitListExpr {
                flatten(child, values);
            } else {
                values.push(match child.evaluate() {
                    Some(EvaluationResult::SignedInteger(value)) => u64::try_from(value).ok(),
                    Some(EvaluationResult::UnsignedInteger(value)) => Some(value),
                    _ => None,
                });
            }
        }
    }
    let Some(list) = find_init_list(method) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    flatten(list, &mut values);
    values
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn value(result: &EvaluationResult) -> Option<Value> {
    match *result {
        EvaluationResult::SignedInteger(value) => Some(Value::Integer(i128::from(value))),
        EvaluationResult::UnsignedInteger(value) => Some(Value::Integer(i128::from(value))),
        EvaluationResult::Float(value) => Some(Value::Float(value)),
        _ => None,
    }
}

struct Evaluation {
    expression: String,
    location: Location,
    what: String,
    target: Slot,
}

enum Slot {
    Macro(usize),
    Field(usize, usize),
}

fn evaluations(selection: &Selection) -> Result<Vec<Evaluation>> {
    let mut evaluations = Vec::new();
    for (position, selected) in selection.macros.iter().enumerate() {
        evaluations.push(Evaluation {
            expression: format!("({})", selected.name),
            location: selected.location.clone(),
            what: format!(
                "macro {}, which a rule includes as {}",
                selected.name, selected.reason
            ),
            target: Slot::Macro(position),
        });
    }
    for (item_position, item) in selection.items.iter().enumerate() {
        let ItemKind::Record(record) = &item.kind else {
            continue;
        };
        for (field_position, field) in record.fields.iter().enumerate() {
            if field.init != Init::Expression {
                continue;
            }
            let field_name = field.name.clone().unwrap_or_default();
            let what = format!(
                "the default member initializer of {}::{field_name}",
                item.name.cxx
            );
            if item.name.cxx.contains('(') {
                return Err(Error::at(
                    ErrorKind::Unsupported,
                    &field.location,
                    format!("cannot evaluate {what}, because the record has no name"),
                ));
            }
            evaluations.push(Evaluation {
                expression: format!("{}{{}}.{field_name}", item.name.cxx),
                location: field.location.clone(),
                what,
                target: Slot::Field(item_position, field_position),
            });
        }
    }
    Ok(evaluations)
}

pub(crate) fn evaluate(
    index: &Index<'_>,
    target: &str,
    header_dir: &Path,
    main_header: &str,
    selection: &mut Selection,
) -> Result<()> {
    let evaluations = evaluations(selection)?;
    if evaluations.is_empty() {
        return Ok(());
    }
    let mut source = format!("#include \"{main_header}\"\n");
    for (position, evaluation) in evaluations.iter().enumerate() {
        put(
            &mut source,
            format_args!(
                "constexpr auto {EVALUATION_PREFIX}{position} = {};\n",
                evaluation.expression
            ),
        );
    }
    let path = header_dir.join(EVALUATION_FILE);
    let tu = index
        .parser(&path)
        .arguments(&arguments(target, header_dir))
        .unsaved(&[Unsaved::new(&path, &source)])
        .parse()
        .map_err(|error| {
            Error::new(
                ErrorKind::Libclang,
                format!("libclang cannot parse the evaluation file for target {target}"),
            )
            .with_source(error.to_string())
        })?;
    check_diagnostics(&tu, target, header_dir, |line| {
        let position = usize::try_from(line).ok()?.checked_sub(2)?;
        evaluations
            .get(position)
            .map(|evaluation| (evaluation.location.clone(), evaluation.what.clone()))
    })?;
    let mut values: Vec<Option<Value>> = vec![None; evaluations.len()];
    for child in tu.get_entity().get_children() {
        if child.get_kind() != EntityKind::VarDecl {
            continue;
        }
        let position = child
            .get_name()
            .and_then(|name| name.strip_prefix(EVALUATION_PREFIX)?.parse::<usize>().ok());
        if let Some(slot) = position.and_then(|position| values.get_mut(position)) {
            *slot = child.evaluate().as_ref().and_then(value);
        }
    }
    for (evaluation, value) in evaluations.iter().zip(values) {
        let Some(value) = value else {
            return Err(Error::at(
                ErrorKind::Evaluation,
                &evaluation.location,
                format!(
                    "cannot evaluate {} for target {target}: libclang returned no integer or floating-point value",
                    evaluation.what
                ),
            ));
        };
        match evaluation.target {
            Slot::Macro(position) => selection.macros[position].value = Some(value),
            Slot::Field(item, field) => {
                if let ItemKind::Record(record) = &mut selection.items[item].kind {
                    record.fields[field].init = Init::Value(value);
                }
            }
        }
    }
    Ok(())
}
