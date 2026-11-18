use std::collections::{BTreeMap, HashMap, HashSet};

use crate::emit::TypeNames;
use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::{
    CType, Constant, Evaluation, Field, Function, Init, Interface, Item, ItemKind, Macro, Member,
    Method, Model, Prim, Record, Signature, Ty, Underlying, Value,
};
use crate::rules::{Action, MacroAction, NameRule, RuleSet, matches};

const C_CONVENTION: &str = "Cdecl";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Selection {
    pub(crate) items: Vec<Item>,
    pub(crate) excluded: Vec<Item>,
    pub(crate) external: Vec<Item>,
    pub(crate) macros: Vec<SelectedMacro>,
    pub(crate) without_default: BTreeMap<String, WithoutDefault>,
    pub(crate) microsoft_abi: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WithoutDefault {
    Rule(&'static str),
    Holds { field: String, held: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SelectedMacro {
    pub(crate) name: String,
    pub(crate) location: Location,
    pub(crate) action: MacroAction,
    pub(crate) reason: &'static str,
    pub(crate) value: Option<Value>,
}

pub(crate) fn select(model: Model, rules: &RuleSet, target: &str) -> Result<Selection> {
    let mut used = Used {
        declarations: vec![false; rules.declarations.len()],
        macros: vec![false; rules.macros.len()],
        unnamed_enums: vec![false; rules.unnamed_enums.len()],
    };
    let mut unmatched = Unmatched::default();
    let decisions = match_declarations(&model.items, rules, &mut used, &mut unmatched);
    let macros = match_macros(model.macros, rules, &mut used, &mut unmatched);
    unmatched.check()?;
    used.check(rules)?;
    let mut selection = partition(model.items, &decisions, macros);
    selection.microsoft_abi = model.microsoft_abi;
    apply_class_rules(&mut selection, rules, target)?;
    Ok(selection)
}

pub(crate) fn is_microsoft_abi(normalized_triple: &str) -> bool {
    let mut parts = normalized_triple.split('-').skip(2);
    parts.next() == Some("windows")
        && parts
            .next()
            .is_some_and(|environment| environment.starts_with("msvc"))
}

fn position(rules: &[NameRule], name: &str) -> Option<usize> {
    rules.iter().position(|rule| matches(rule.pattern, name))
}

#[derive(Default)]
struct ClassRulesUsed {
    members: Vec<bool>,
    templates: Vec<bool>,
    uuidless: Vec<bool>,
    overloads: Vec<bool>,
    placeholders: Vec<bool>,
    called_callbacks: Vec<bool>,
}

fn apply_class_rules(selection: &mut Selection, rules: &RuleSet, target: &str) -> Result<()> {
    let mut used = ClassRulesUsed {
        members: vec![false; rules.members.map_or(0, <[NameRule]>::len)],
        templates: vec![false; rules.templates.len()],
        uuidless: vec![false; rules.uuidless.len()],
        overloads: vec![false; rules.overloads.len()],
        placeholders: vec![false; rules.placeholders.len()],
        called_callbacks: vec![false; rules.called_callbacks.len()],
    };
    let mut unmatched_members = Vec::new();
    let mut microsoft_overloads = Vec::new();
    let microsoft_abi = selection.microsoft_abi;
    for item in &mut selection.items {
        let name = item.name.cxx.clone();
        match &mut item.kind {
            ItemKind::Interface(interface) => {
                if !interface.com
                    && let Some(position) = position(rules.uuidless, &name)
                {
                    used.uuidless[position] = true;
                }
                match_members(rules, &interface.members, &mut used, &mut unmatched_members);
                name_overloads(
                    &name,
                    &mut interface.methods,
                    rules,
                    microsoft_abi,
                    &mut used,
                    &mut microsoft_overloads,
                );
                for method in &mut interface.methods {
                    let qualified = format!("{name}::{}", method.rust_name());
                    if let Some(position) = position(rules.placeholders, &qualified) {
                        used.placeholders[position] = true;
                        method.placeholder = true;
                    }
                    mark_called_callbacks(&qualified, method, rules, &mut used);
                }
            }
            ItemKind::Record(record) => {
                match_members(rules, &record.members, &mut used, &mut unmatched_members);
                for field in &record.fields {
                    mark_templates(&field.ty, rules, &mut used);
                }
            }
            _ => {}
        }
    }
    if let Some((location, _)) = microsoft_overloads.first() {
        let names: Vec<&str> = microsoft_overloads
            .iter()
            .map(|(_, name)| name.as_str())
            .collect();
        return Err(Error::at(
            ErrorKind::Unsupported,
            location,
            format!(
                "virtual methods share a name on target {target}, whose C++ ABI orders such methods in a vtable in a way that is not established, so the bindings cannot place them: {}",
                names.join(", ")
            ),
        ));
    }
    if let Some(first) = unmatched_members.first() {
        let location = first.location.clone();
        let descriptions: Vec<String> = unmatched_members
            .iter()
            .map(|member| format!("{} ({}, {})", member.name, member.kind, member.location))
            .collect();
        return Err(Error::at(
            ErrorKind::Unsupported,
            &location,
            format!(
                "{} members of included records and interfaces are not data and not virtual methods, and no rule ignores them: {}",
                descriptions.len(),
                descriptions.join(", ")
            ),
        ));
    }
    stale_class_rules(rules, &used)
}

fn match_members(
    rules: &RuleSet,
    members: &[Member],
    used: &mut ClassRulesUsed,
    unmatched: &mut Vec<Member>,
) {
    let Some(member_rules) = rules.members else {
        return;
    };
    for member in members {
        match position(member_rules, &member.name) {
            Some(position) => used.members[position] = true,
            None => unmatched.push(member.clone()),
        }
    }
}

fn mark_templates(ty: &Ty, rules: &RuleSet, used: &mut ClassRulesUsed) {
    match ty {
        Ty::Template { template, .. } => {
            if let Some(position) = position(rules.templates, template) {
                used.templates[position] = true;
            }
        }
        Ty::Array { element, .. } => mark_templates(element, rules, used),
        _ => {}
    }
}

fn name_overloads(
    owner: &str,
    methods: &mut [Method],
    rules: &RuleSet,
    microsoft_abi: bool,
    used: &mut ClassRulesUsed,
    microsoft: &mut Vec<(Location, String)>,
) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for method in methods.iter() {
        *counts.entry(method.name.clone()).or_default() += 1;
    }
    if microsoft_abi {
        for method in methods
            .iter()
            .filter(|method| counts.get(&method.name).copied().unwrap_or_default() > 1)
        {
            microsoft.push((
                method.location.clone(),
                format!(
                    "{owner}::{}({}) ({})",
                    method.name,
                    method.spelled_params.join(", "),
                    method.location
                ),
            ));
        }
        return;
    }
    for method in methods.iter_mut() {
        if counts.get(&method.name).copied().unwrap_or_default() < 2 {
            continue;
        }
        let qualified = format!("{owner}::{}", method.name);
        if let Some(position) = rules.overloads.iter().position(|rule| {
            rule.method == qualified
                && rule.params.len() == method.spelled_params.len()
                && rule
                    .params
                    .iter()
                    .zip(&method.spelled_params)
                    .all(|(wanted, spelled)| wanted == spelled)
        }) {
            used.overloads[position] = true;
            method.rust = Some(rules.overloads[position].rust.to_owned());
        }
    }
}

fn mark_called_callbacks(
    qualified: &str,
    method: &mut Method,
    rules: &RuleSet,
    used: &mut ClassRulesUsed,
) {
    for param in &mut method.signature.params {
        let (Some(name), Ty::Callback(signature)) = (&param.name, &mut param.ty) else {
            continue;
        };
        if let Some(position) = position(rules.called_callbacks, &format!("{qualified}::{name}")) {
            used.called_callbacks[position] = true;
            signature.called_from_rust = true;
        }
    }
}

fn stale_class_rules(rules: &RuleSet, used: &ClassRulesUsed) -> Result<()> {
    let mut stale = Vec::new();
    let mut name_rules = |kind: &str, list: &[NameRule], used: &[bool]| {
        for (rule, used) in list.iter().zip(used) {
            if !used {
                stale.push(format!(
                    "the {kind} rule {} ({})",
                    rule.pattern, rule.reason
                ));
            }
        }
    };
    name_rules("member", rules.members.unwrap_or_default(), &used.members);
    name_rules("template", rules.templates, &used.templates);
    name_rules("interface", rules.uuidless, &used.uuidless);
    name_rules("placeholder", rules.placeholders, &used.placeholders);
    name_rules(
        "called callback",
        rules.called_callbacks,
        &used.called_callbacks,
    );
    for (rule, used) in rules.overloads.iter().zip(&used.overloads) {
        if !used {
            stale.push(format!(
                "the overload rule {}({}) ({})",
                rule.method,
                rule.params.join(", "),
                rule.reason
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

#[derive(Default)]
struct Decisions {
    actions: HashMap<String, Action>,
    constants_of: HashMap<String, &'static str>,
}

#[derive(Default)]
struct Unmatched {
    first: Option<Location>,
    descriptions: Vec<String>,
}

impl Unmatched {
    fn record(&mut self, description: String, location: &Location) {
        self.first.get_or_insert_with(|| location.clone());
        self.descriptions.push(description);
    }

    fn check(self) -> Result<()> {
        let Some(first) = self.first else {
            return Ok(());
        };
        Err(Error::at(
            ErrorKind::UnmatchedDeclaration,
            &first,
            format!(
                "{} declarations and macros match no rule of the rule set: {}",
                self.descriptions.len(),
                self.descriptions.join(", ")
            ),
        ))
    }
}

fn match_declarations(
    items: &[Item],
    rules: &RuleSet,
    used: &mut Used,
    unmatched: &mut Unmatched,
) -> Decisions {
    let mut decisions = Decisions::default();
    for item in items.iter().filter(|item| item.owner.is_none()) {
        let file = item.location.file();
        let external = rules
            .declaration_position(file, &item.name.cxx)
            .filter(|position| rules.declarations[*position].action == Action::External);
        if let Some(position) = external {
            used.declarations[position] = true;
            decisions
                .actions
                .insert(item.name.cxx.clone(), Action::External);
        } else if let ItemKind::Enum(enumeration) = &item.kind
            && enumeration.unnamed
        {
            let Some(position) = rules.unnamed_enum_position(file, &enumeration.enumerators) else {
                let first = enumeration
                    .enumerators
                    .first()
                    .map_or("no enumerator", |enumerator| enumerator.name.as_str());
                unmatched.record(
                    format!(
                        "the unnamed enumeration that starts with {first} ({})",
                        item.location
                    ),
                    &item.location,
                );
                continue;
            };
            used.unnamed_enums[position] = true;
            decisions
                .actions
                .insert(item.name.cxx.clone(), Action::Include);
            decisions
                .constants_of
                .insert(item.name.cxx.clone(), rules.unnamed_enums[position].typedef);
        } else if let Some(position) = rules.declaration_position(file, &item.name.cxx) {
            used.declarations[position] = true;
            decisions
                .actions
                .insert(item.name.cxx.clone(), rules.declarations[position].action);
        } else {
            unmatched.record(
                format!("{} ({})", item.name.cxx, item.location),
                &item.location,
            );
        }
    }
    decisions
}

fn match_macros(
    macros: Vec<Macro>,
    rules: &RuleSet,
    used: &mut Used,
    unmatched: &mut Unmatched,
) -> Vec<SelectedMacro> {
    let mut selected = Vec::new();
    for Macro { name, location } in macros {
        let Some(position) = rules.macro_position(location.file(), &name) else {
            unmatched.record(format!("macro {name} ({location})"), &location);
            continue;
        };
        used.macros[position] = true;
        let rule = &rules.macros[position];
        if rule.action != MacroAction::Exclude {
            selected.push(SelectedMacro {
                name,
                location,
                action: rule.action,
                reason: rule.reason,
                value: None,
            });
        }
    }
    selected
}

fn partition(items: Vec<Item>, decisions: &Decisions, macros: Vec<SelectedMacro>) -> Selection {
    let owners: HashMap<String, String> = items
        .iter()
        .filter_map(|item| Some((item.name.cxx.clone(), item.owner.clone()?)))
        .collect();
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    let mut external = Vec::new();
    for mut item in items {
        let root = root_owner(&owners, &item.name.cxx);
        if let ItemKind::Enum(enumeration) = &mut item.kind
            && let Some(typedef) = decisions.constants_of.get(&item.name.cxx)
        {
            enumeration.constants_of = Some((*typedef).to_owned());
        }
        match decisions.actions.get(root) {
            Some(Action::Include) => included.push(item),
            Some(Action::External) => external.push(item),
            _ => excluded.push(item),
        }
    }
    Selection {
        items: included,
        excluded,
        external,
        macros,
        without_default: BTreeMap::new(),
        microsoft_abi: false,
    }
}

enum Slot {
    Macro(usize),
    Field(usize, usize),
}

pub(crate) fn evaluate(
    selection: &mut Selection,
    rules: &RuleSet,
    evaluate: impl FnOnce(&[Evaluation]) -> Result<Vec<Value>>,
) -> Result<()> {
    let (evaluations, slots) = evaluations(selection, rules)?;
    if !evaluations.is_empty() {
        for (slot, value) in slots.into_iter().zip(evaluate(&evaluations)?) {
            match slot {
                Slot::Macro(position) => selection.macros[position].value = Some(value),
                Slot::Field(item, field) => {
                    if let ItemKind::Record(record) = &mut selection.items[item].kind {
                        record.fields[field].init = Init::Value(value);
                    }
                }
            }
        }
    }
    selection.without_default = without_default(selection, rules)?;
    Ok(())
}

fn without_default(
    selection: &Selection,
    rules: &RuleSet,
) -> Result<BTreeMap<String, WithoutDefault>> {
    let mut marked = BTreeMap::new();
    let mut stale = Vec::new();
    for rule in rules.without_default {
        let mut named = false;
        for item in &selection.items {
            let ItemKind::Record(record) = &item.kind else {
                continue;
            };
            if !matches(rule.pattern, &item.name.cxx) {
                continue;
            }
            named = true;
            if record
                .fields
                .iter()
                .any(|field| field.init == Init::Expression)
            {
                marked.insert(item.name.cxx.clone(), WithoutDefault::Rule(rule.reason));
            } else {
                stale.push(format!(
                    "the rule without Default {} ({}), whose struct {} has a default that the bindings reproduce",
                    rule.pattern, rule.reason, item.name.cxx
                ));
            }
        }
        if !named {
            stale.push(format!(
                "the rule without Default {} ({}), which names no included struct",
                rule.pattern, rule.reason
            ));
        }
    }
    if !stale.is_empty() {
        return Err(Error::new(
            ErrorKind::StaleRule,
            format!(
                "{} rules of the rule set decide no declaration or macro of the headers, so remove or correct them: {}",
                stale.len(),
                stale.join(", ")
            ),
        ));
    }
    let kinds: HashMap<&str, &ItemKind> = selection
        .items
        .iter()
        .map(|item| (item.name.cxx.as_str(), &item.kind))
        .collect();
    loop {
        let mut added = Vec::new();
        for item in &selection.items {
            let ItemKind::Record(record) = &item.kind else {
                continue;
            };
            if marked.contains_key(&item.name.cxx) {
                continue;
            }
            let holds = record.fields.iter().find_map(|field| {
                let held = held_record(&field.ty, &kinds)?;
                marked.contains_key(held).then(|| WithoutDefault::Holds {
                    field: field
                        .name
                        .clone()
                        .unwrap_or_else(|| "(anonymous member)".to_owned()),
                    held: held.to_owned(),
                })
            });
            if let Some(holds) = holds {
                added.push((item.name.cxx.clone(), holds));
            }
        }
        if added.is_empty() {
            return Ok(marked);
        }
        marked.extend(added);
    }
}

fn held_record<'a>(ty: &'a Ty, kinds: &HashMap<&str, &'a ItemKind>) -> Option<&'a str> {
    match ty {
        Ty::Array { element, .. } => held_record(element, kinds),
        Ty::Named(name) => match kinds.get(name.as_str())? {
            ItemKind::Record(_) => Some(name),
            ItemKind::Typedef(inner) => held_record(inner, kinds),
            _ => None,
        },
        _ => None,
    }
}

fn evaluations(selection: &Selection, rules: &RuleSet) -> Result<(Vec<Evaluation>, Vec<Slot>)> {
    let in_place = rules.initializers_in_place;
    let mut evaluations = Vec::new();
    let mut slots = Vec::new();
    for (position, selected) in selection.macros.iter().enumerate() {
        evaluations.push(Evaluation {
            expression: format!("({})", selected.name),
            location: selected.location.clone(),
            what: format!(
                "macro {}, which a rule includes as {}",
                selected.name, selected.reason
            ),
        });
        slots.push(Slot::Macro(position));
    }
    for (item_position, item) in selection.items.iter().enumerate() {
        let ItemKind::Record(record) = &item.kind else {
            continue;
        };
        if position(rules.without_default, &item.name.cxx).is_some() {
            continue;
        }
        for (field_position, field) in record.fields.iter().enumerate() {
            if field.init != Init::Expression {
                continue;
            }
            let field_name = field.name.clone().unwrap_or_default();
            let what = format!(
                "the default member initializer of {}::{field_name}",
                item.name.cxx
            );
            if in_place {
                return Err(Error::at(
                    ErrorKind::Evaluation,
                    &field.location,
                    format!(
                        "cannot evaluate {what}, because libclang returned no integer or floating-point value for the initializer"
                    ),
                ));
            }
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
            });
            slots.push(Slot::Field(item_position, field_position));
        }
    }
    Ok((evaluations, slots))
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
    external: HashMap<&'a str, &'a Item>,
    external_types: Option<&'a TypeNames>,
}

pub(crate) fn validate(
    selection: &Selection,
    rules: &RuleSet,
    target: &str,
    external_types: Option<&TypeNames>,
) -> Result<()> {
    let validator = Validator::new(selection, rules, target, external_types);
    validator.validate()
}

impl<'a> Validator<'a> {
    pub(crate) fn new(
        selection: &'a Selection,
        rules: &'a RuleSet,
        target: &'a str,
        external_types: Option<&'a TypeNames>,
    ) -> Self {
        let by_name = |items: &'a [Item]| {
            items
                .iter()
                .map(|item| (item.name.cxx.as_str(), item))
                .collect::<HashMap<_, _>>()
        };
        Self {
            selection,
            rules,
            target,
            included: by_name(&selection.items),
            excluded: by_name(&selection.excluded),
            external: by_name(&selection.external),
            external_types,
        }
    }

    pub(crate) fn included(&self, cxx: &str) -> Option<&'a Item> {
        self.included
            .get(cxx)
            .or_else(|| self.external.get(cxx))
            .copied()
    }

    pub(crate) fn is_external(&self, cxx: &str) -> bool {
        !self.included.contains_key(cxx) && self.external.contains_key(cxx)
    }

    pub(crate) fn uuid_item(&self) -> Option<&'a Item> {
        self.selection
            .items
            .iter()
            .chain(&self.selection.external)
            .find(|item| item.name.rust.as_deref() == Some(self.rules.uuid_type))
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
            ItemKind::Typedef(ty) => self.typedef(item, ty),
            ItemKind::Constant(constant) => self.constant(item, constant),
            ItemKind::Function(function) => self.function(item, function),
            ItemKind::Enum(enumeration) => self.enumeration(item, enumeration),
            ItemKind::Record(record) => self.record(item, record),
            ItemKind::Interface(interface) => self.interface(item, interface),
        }
    }

    fn typedef(&self, item: &'a Item, ty: &'a Ty) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
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

    fn constant(&self, item: &'a Item, constant: &'a Constant) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
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
        if let Ty::Pointer { .. } = self.resolve(&constant.ty) {
            let (min, max) = Prim::Usize.range();
            return match value {
                Value::Integer(address) if (min..=max).contains(&address) => Ok(()),
                _ => Err(Error::at(
                    ErrorKind::Evaluation,
                    location,
                    format!(
                        "the pointer constant {name} has a value that is not an address of the target"
                    ),
                )),
            };
        }
        self.check_value(&constant.ty, value, location, &format!("constant {name}"))
    }

    fn function(&self, item: &'a Item, function: &'a Function) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
        if !function.c_linkage {
            return Err(Self::unsupported(
                location,
                format!(
                    "function {name} has C++ linkage, so the library exports it only under a mangled name that differs between compilers"
                ),
            ));
        }
        if function.inline {
            return Err(Self::unsupported(
                location,
                format!(
                    "function {name} is defined inline in the header, so the library does not export it"
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
            self.field(name, record, field)?;
        }
        Ok(())
    }

    fn field(&self, name: &str, record: &'a Record, field: &'a Field) -> Result<()> {
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
        match &field.init {
            Init::Null
                if !matches!(
                    self.resolve(&field.ty),
                    Ty::Pointer { .. } | Ty::Callback(_)
                ) =>
            {
                Err(Self::unsupported(
                    &field.location,
                    format!("{what} is initialized with nullptr but is not a pointer"),
                ))
            }
            Init::Expression
                if matches!(
                    self.selection.without_default.get(name),
                    Some(WithoutDefault::Rule(_))
                ) =>
            {
                Ok(())
            }
            Init::Expression => Err(Error::at(
                ErrorKind::Evaluation,
                &field.location,
                format!("the default member initializer of {what} was not evaluated"),
            )),
            Init::Value(value) => self.check_value(&field.ty, *value, &field.location, &what),
            Init::Constant(constant) => {
                match self.included(constant.as_str()).map(|item| &item.kind) {
                    Some(ItemKind::Constant(Constant { ty, value: Some(_) }))
                        if self.resolve(ty) == self.resolve(&field.ty) =>
                    {
                        Ok(())
                    }
                    _ => Err(Error::at(
                        ErrorKind::Evaluation,
                        &field.location,
                        format!(
                            "the default member initializer of {what} names {constant}, which the bindings do not declare as a constant of the type of the field"
                        ),
                    )),
                }
            }
            Init::None | Init::Null | Init::Zero => Ok(()),
        }
    }

    fn interface(&self, item: &'a Item, interface: &'a Interface) -> Result<()> {
        if !interface.com {
            if position(self.rules.uuidless, &item.name.cxx).is_none() {
                return Err(Self::unsupported(
                    &item.location,
                    format!(
                        "{} is a class with virtual methods and no UUID, which the bindings cannot express",
                        item.name.cxx
                    ),
                ));
            }
            if !interface.bases.is_empty() {
                return Err(Self::unsupported(
                    &item.location,
                    format!(
                        "class {} has virtual methods, no UUID, and a base class, whose vtable prefix the bindings cannot state",
                        item.name.cxx
                    ),
                ));
            }
            Self::interface_shape(item, interface)?;
            return self.interface_methods(item, interface);
        }
        Self::interface_shape(item, interface)?;
        self.interface_uuid(item, interface)?;
        self.interface_methods(item, interface)
    }

    fn interface_shape(item: &Item, interface: &Interface) -> Result<()> {
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
        Ok(())
    }

    fn interface_uuid(&self, item: &'a Item, interface: &'a Interface) -> Result<()> {
        let name = &item.name.cxx;
        let location = &item.location;
        if interface.uuid.is_none() {
            return Err(Self::unsupported(
                location,
                format!(
                    "cannot read the UUID of interface {name} from the body of getTypeGuid as eleven integers"
                ),
            ));
        }
        if !matches!(
            self.uuid_item().map(|candidate| &candidate.kind),
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
        Ok(())
    }

    fn interface_methods(&self, item: &'a Item, interface: &'a Interface) -> Result<()> {
        let name = &item.name.cxx;
        let mut names = HashSet::new();
        for base in self.chain(item, interface)?.iter().skip(1) {
            for method in &base.methods {
                names.insert(method.rust_name());
            }
        }
        for method in &interface.methods {
            let what = format!("method {name}::{}", method.name);
            if !names.insert(method.rust_name()) {
                return Err(Self::unsupported(
                    &method.location,
                    format!(
                        "{what} overloads or overrides another method of the same name, so its vtable slot cannot be told from the declaration order, and no rule names it with its parameter types ({})",
                        method.spelled_params.join(", ")
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
            if method.placeholder {
                self.placeholder(&method.signature, &method.location, &what)?;
            } else {
                self.signature(&method.signature, &method.location, &what, false)?;
            }
        }
        Ok(())
    }

    fn by_value_aggregate(&self, ty: &'a Ty) -> bool {
        let Ty::Named(name) = self.resolve(ty) else {
            return false;
        };
        self.included(name)
            .or_else(|| self.excluded(name))
            .is_some_and(|item| matches!(item.kind, ItemKind::Record(_)))
    }

    fn placeholder(&self, signature: &'a Signature, location: &Location, what: &str) -> Result<()> {
        if signature.variadic {
            return Err(Self::unsupported(
                location,
                format!("{what} is variadic, which the bindings cannot declare"),
            ));
        }
        let mut aggregate = false;
        for (position, param) in signature.params.iter().enumerate() {
            if self.by_value_aggregate(&param.ty) {
                aggregate = true;
                continue;
            }
            if matches!(param.ty, Ty::Array { .. }) {
                continue;
            }
            let param_name = param.name.clone().unwrap_or_else(|| format!("#{position}"));
            self.ty(
                &param.ty,
                Context::Value,
                location,
                &format!("parameter {param_name} of {what}"),
            )?;
        }
        if self.by_value_aggregate(&signature.result) {
            aggregate = true;
        } else if signature.result != Ty::Void {
            self.ty(
                &signature.result,
                Context::Value,
                location,
                &format!("the return value of {what}"),
            )?;
        }
        if !aggregate {
            return Err(Self::unsupported(
                location,
                format!(
                    "a rule keeps {what} as a placeholder slot, but it passes no aggregate by value, so the bindings can declare it"
                ),
            ));
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
            Ty::Template { template, args } => {
                let ([arg], Some(_), Context::Field) = (
                    args.as_slice(),
                    position(self.rules.templates, template),
                    context,
                ) else {
                    return Err(Self::unsupported(
                        location,
                        format!(
                            "{what} has the type of the template {template}, which no rule maps to a pointer to its one argument in a field"
                        ),
                    ));
                };
                self.ty(arg, Context::Pointee, location, what)
            }
        }
    }

    fn named(&self, name: &str, context: Context, location: &Location, what: &str) -> Result<()> {
        if self.is_external(name) {
            self.external_reference(name, context, location, what)?;
        }
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
                    "{what} uses {name} by value, but a rule excludes it as {reason}, so the bindings declare it only as an opaque type behind a pointer"
                ),
            ));
        }
        Err(Self::unsupported(
            location,
            format!("{what} refers to {name}, which the headers do not declare"),
        ))
    }

    fn external_reference(
        &self,
        name: &str,
        context: Context,
        location: &Location,
        what: &str,
    ) -> Result<()> {
        let rust = self
            .included(name)
            .and_then(|item| item.name.rust.as_deref());
        let crate_name = self.rules.external_crate.unwrap_or("the external crate");
        let found = match (rust, self.external_types) {
            (Some(rust), Some(types)) => {
                types.included.contains(rust)
                    || matches!(context, Context::Pointee | Context::Typedef)
                        && types.opaque.contains(rust)
            }
            _ => false,
        };
        if found {
            return Ok(());
        }
        Err(Self::unsupported(
            location,
            format!(
                "{what} refers to the Slang type {name}, which the bindings of {crate_name} do not declare in a form that this use needs"
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Enumerator, Name};
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
            external: Vec::new(),
            macros: Vec::new(),
            without_default: BTreeMap::new(),
            microsoft_abi: false,
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
        validate(&selection(items), &SLANG, "test-target", None).expect_err("validation fails")
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

    fn void_pointer() -> Ty {
        Ty::Pointer {
            pointee: Box::new(Ty::Void),
            is_const: true,
        }
    }

    #[test]
    fn validate_refuses_a_pointer_field_with_a_value_other_than_nullptr() {
        let record = item(
            "R",
            ItemKind::Record(Record {
                union: false,
                size: 8,
                align: 8,
                fields: vec![Field {
                    name: Some("pointer".to_owned()),
                    location: Location::new("slang.h", 1),
                    ty: void_pointer(),
                    offset_bits: 0,
                    bit_width: None,
                    init: Init::Value(Value::Integer(8)),
                    spelling: None,
                }],
                members: Vec::new(),
                has_bases: false,
            }),
        );
        let error = refusal(vec![record]);
        assert_refusal(
            &error,
            ErrorKind::Unsupported,
            "field R::pointer is a pointer initialized with a value other than nullptr",
        );
    }

    #[test]
    fn validate_accepts_a_pointer_constant_with_an_address_and_refuses_any_other_value() {
        for address in [0, i128::from(u64::MAX)] {
            validate(
                &selection(vec![constant(void_pointer(), Value::Integer(address))]),
                &SLANG,
                "test-target",
                None,
            )
            .unwrap_or_else(|error| panic!("{address}: {error}"));
        }
        for value in [
            Value::Integer(-1),
            Value::Integer(i128::from(u64::MAX) + 1),
            Value::Float(1.0),
        ] {
            assert_refusal(
                &refusal(vec![constant(void_pointer(), value)]),
                ErrorKind::Evaluation,
                "the pointer constant C has a value that is not an address of the target",
            );
        }
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
                    spelling: None,
                }],
                members: Vec::new(),
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
                    spelling: None,
                }],
                members: Vec::new(),
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
            None,
        )
        .expect("the values fit");
    }

    #[test]
    fn is_microsoft_abi_holds_for_a_normalized_windows_triple_of_the_msvc_environment_only() {
        assert!(is_microsoft_abi("x86_64-pc-windows-msvc19.33.0"));
        assert!(is_microsoft_abi("x86_64-unknown-windows-msvc19.33.0"));
        assert!(is_microsoft_abi("aarch64-unknown-windows-msvc19.33.0"));
        assert!(is_microsoft_abi("x86_64-pc-windows-msvc"));
        assert!(!is_microsoft_abi("x86_64-pc-windows-gnu"));
        assert!(!is_microsoft_abi("x86_64-w64-windows-gnu"));
        assert!(!is_microsoft_abi("x86_64-pc-windows-gnullvm"));
        assert!(!is_microsoft_abi("x86_64-pc-windows-itanium"));
        assert!(!is_microsoft_abi("x86_64-pc-windows-cygnus"));
        assert!(!is_microsoft_abi("x86_64-unknown-linux-gnu"));
        assert!(!is_microsoft_abi("arm64-apple-macosx10.4.0"));
        assert!(!is_microsoft_abi("x86_64-unknown-linux-msvc"));
        assert!(!is_microsoft_abi("msvc"));
    }

    fn method(name: &str, params: &[&str]) -> Method {
        Method {
            name: name.to_owned(),
            rust: None,
            location: Location::new("slang-rhi.h", 1),
            signature: Signature {
                params: Vec::new(),
                result: Ty::Void,
                variadic: false,
                convention: C_CONVENTION.to_owned(),
                called_from_rust: false,
            },
            pure: true,
            placeholder: false,
            spelled_params: params.iter().map(|param| (*param).to_owned()).collect(),
            canonical: crate::model::CanonicalSignature::default(),
        }
    }

    fn rhi_interface(members: Vec<Member>, methods: Vec<Method>) -> Item {
        Item {
            name: Name {
                cxx: "rhi::IThing".to_owned(),
                rust: Some("IThing".to_owned()),
            },
            location: Location::new("slang-rhi.h", 1),
            owner: None,
            kind: ItemKind::Interface(Interface {
                com: true,
                bases: Vec::new(),
                uuid: None,
                methods,
                members,
                has_fields: false,
                has_virtual_destructor: false,
            }),
        }
    }

    const OVERLOADS: &[crate::rules::OverloadRule] = &[
        crate::rules::OverloadRule {
            method: "rhi::IThing::read",
            params: &["int"],
            rust: "read",
            reason: "a test overload",
        },
        crate::rules::OverloadRule {
            method: "rhi::IThing::read",
            params: &["const char *"],
            rust: "read_name",
            reason: "a test overload",
        },
    ];

    const TEST_RHI: RuleSet = RuleSet {
        members: Some(&[NameRule {
            pattern: "rhi::IThing::helper",
            reason: "a test member",
        }]),
        overloads: OVERLOADS,
        placeholders: &[NameRule {
            pattern: "rhi::IThing::read_name",
            reason: "a test placeholder",
        }],
        uuidless: &[],
        templates: &[],
        ..SLANG
    };

    fn member(name: &str) -> Member {
        Member {
            name: name.to_owned(),
            kind: "Method".to_owned(),
            location: Location::new("slang-rhi.h", 2),
        }
    }

    #[test]
    fn apply_class_rules_names_overloads_and_placeholders_by_rule() {
        let mut selection = selection(vec![rhi_interface(
            vec![member("rhi::IThing::helper")],
            vec![
                method("read", &["int"]),
                method("other", &[]),
                method("read", &["const char *"]),
            ],
        )]);
        apply_class_rules(&mut selection, &TEST_RHI, "x86_64-unknown-linux-gnu")
            .expect("every rule decides something");
        let ItemKind::Interface(interface) = &selection.items[0].kind else {
            panic!("the item is an interface");
        };
        let names: Vec<(&str, bool)> = interface
            .methods
            .iter()
            .map(|method| (method.rust_name(), method.placeholder))
            .collect();
        assert_eq!(
            names,
            [("read", false), ("other", false), ("read_name", true)]
        );
    }

    #[test]
    fn apply_class_rules_refuses_a_member_without_a_rule_and_overloads_on_the_microsoft_abi() {
        let mut members = selection(vec![rhi_interface(
            vec![member("rhi::IThing::helper"), member("rhi::IThing::other")],
            vec![method("read", &["int"]), method("read", &["const char *"])],
        )]);
        let error = apply_class_rules(&mut members, &TEST_RHI, "x86_64-unknown-linux-gnu")
            .expect_err("the member has no rule");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert_eq!(
            error.message(),
            "1 members of included records and interfaces are not data and not virtual methods, and no rule ignores them: rhi::IThing::other (Method, slang-rhi.h:2)"
        );
        let mut overloads = selection(vec![rhi_interface(
            vec![member("rhi::IThing::helper")],
            vec![method("read", &["int"]), method("read", &["const char *"])],
        )]);
        overloads.microsoft_abi = true;
        let error = apply_class_rules(&mut overloads, &TEST_RHI, "x86_64-pc-windows-msvc")
            .expect_err("the Microsoft ABI refuses overloads");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert!(
            error
                .message()
                .ends_with("rhi::IThing::read(int) (slang-rhi.h:1), rhi::IThing::read(const char *) (slang-rhi.h:1)"),
            "{error}"
        );
    }

    #[test]
    fn apply_class_rules_marks_only_the_callback_parameter_that_a_rule_names_as_called_from_rust() {
        let callback = || {
            Ty::Callback(Box::new(Signature {
                params: Vec::new(),
                result: Ty::Void,
                variadic: false,
                convention: C_CONVENTION.to_owned(),
                called_from_rust: false,
            }))
        };
        let mut submit = method("submit", &[]);
        submit.signature.params = ["func", "deleter", "payload"]
            .into_iter()
            .zip([callback(), callback(), Ty::Prim(Prim::U64)])
            .map(|(name, ty)| crate::model::Param {
                name: Some(name.to_owned()),
                ty,
            })
            .collect();
        let mut selection = selection(vec![rhi_interface(
            vec![member("rhi::IThing::helper")],
            vec![
                submit,
                method("read", &["int"]),
                method("read", &["const char *"]),
            ],
        )]);
        let mut rules = TEST_RHI;
        rules.called_callbacks = &[NameRule {
            pattern: "rhi::IThing::submit::func",
            reason: "a test callback",
        }];
        apply_class_rules(&mut selection, &rules, "x86_64-unknown-linux-gnu")
            .expect("every rule decides something");
        let ItemKind::Interface(interface) = &selection.items[0].kind else {
            panic!("the item is an interface");
        };
        let called: Vec<Option<bool>> = interface.methods[0]
            .signature
            .params
            .iter()
            .map(|param| match &param.ty {
                Ty::Callback(signature) => Some(signature.called_from_rust),
                _ => None,
            })
            .collect();
        assert_eq!(called, [Some(true), Some(false), None]);
        rules.called_callbacks = &[NameRule {
            pattern: "rhi::IThing::submit::payload",
            reason: "a test callback",
        }];
        let error = apply_class_rules(&mut selection, &rules, "x86_64-unknown-linux-gnu")
            .expect_err("the rule names no callback");
        assert_eq!(error.kind(), ErrorKind::StaleRule);
        assert!(
            error.message().contains(
                "the called callback rule rhi::IThing::submit::payload (a test callback)"
            ),
            "{error}"
        );
    }

    #[test]
    fn apply_class_rules_reports_every_rule_of_a_new_kind_that_decides_nothing() {
        let mut selection = selection(Vec::new());
        let error = apply_class_rules(&mut selection, &TEST_RHI, "x86_64-unknown-linux-gnu")
            .expect_err("the rules decide nothing");
        assert_eq!(error.kind(), ErrorKind::StaleRule);
        assert!(
            error.message().starts_with("4 rules of the rule set"),
            "{error}"
        );
    }

    #[test]
    fn unfixed_prim_is_unsigned_unless_an_enumerator_is_negative() {
        assert_eq!(unfixed_prim(&unfixed_enum(true, &[0, 1])), Prim::U32);
        assert_eq!(unfixed_prim(&unfixed_enum(false, &[-1, 1])), Prim::I32);
    }
}
