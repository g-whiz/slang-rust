use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr;

use slang_compiler_sys::{
    CompilerOptionEntry, CompilerOptionName, CompilerOptionValue, CompilerOptionValueKind,
    SlangEmitSpirvMethod,
};

use crate::enums::{
    DebugInfoLevel, FloatingPointMode, LanguageVersion, LineDirectiveMode, OptimizationLevel,
    Toggle,
};
use crate::error::{Error, ErrorKind, Operation, Result};
use crate::strings::c_string;

/// The name of a compiler option of Slang, checked against the options of the
/// Slang build that this crate was built for.
///
/// Slang names each compiler option by a number. This type holds a number
/// that names an option of the pinned Slang build, for the raw setters
/// [`CompilerOptions::set_int`] and [`CompilerOptions::set_string`], which
/// reach the options that [`CompilerOptions`] has no typed setter for. Take
/// the number from the constants of
/// [`slang_compiler_sys::CompilerOptionName`] or from Slang's documentation.
///
/// # Examples
///
/// This example names the option that turns on the uniformity analysis of
/// Slang:
///
/// ```
/// use slang_compiler::OptionName;
/// use slang_compiler_sys::CompilerOptionName;
///
/// let name = OptionName::new(CompilerOptionName::ValidateUniformity.0)?;
/// # let _ = name;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OptionName(CompilerOptionName);

impl OptionName {
    /// Checks that a number names a compiler option of the pinned Slang
    /// build, and returns the name.
    ///
    /// Slang numbers its options from 0 up to a sentinel, `CountOf`, and
    /// gives a later option a number above that sentinel. Three numbers below
    /// the sentinel name no option that Slang stores: 79, a removed option,
    /// 111, an old sentinel, and 159, an option of Slang's command line only.
    ///
    /// # Examples
    ///
    /// See the example of [`OptionName`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `value` is
    /// negative, is not below the sentinel `CountOf` of the pinned Slang
    /// build, or is 79, 111, or 159.
    pub fn new(value: i32) -> Result<Self> {
        let name = CompilerOptionName(value);
        let stored = (0..CompilerOptionName::CountOf.0).contains(&value)
            && !matches!(
                name,
                CompilerOptionName::REMOVED_SerialIR
                    | CompilerOptionName::CountOfParsableOptions
                    | CompilerOptionName::GetCompilerPath
            );
        if stored {
            Ok(Self(name))
        } else {
            Err(
                Error::new(ErrorKind::InvalidArgument, Operation::CreateOptionName)
                    .with_subject(format!("`{value}`"))
                    .with_detail(
                        "the number names no compiler option that the pinned Slang build stores"
                            .to_owned(),
                    ),
            )
        }
    }
}

/// A list of compiler options of Slang, which change how Slang checks and
/// compiles shader source.
///
/// A shader is a program that runs on a graphics processing unit (GPU), and
/// Slang compiles it for a target, a format of code such as SPIR-V, the
/// binary shader format of Vulkan. Pass the options to a whole session with
/// [`SessionBuilder::options`](crate::SessionBuilder::options), or to one
/// target with [`Target::options`](crate::Target::options). Each target
/// starts from the options of its session, and its own options override them.
/// An option that is never set keeps Slang's default.
///
/// Each typed setter sets one option and replaces any value of that option
/// that the list held. The three warning setters add a value each time,
/// because Slang applies every value of those options. The raw setters
/// [`CompilerOptions::set_int`] and [`CompilerOptions::set_string`] add an
/// entry for any option that Slang stores, and are `unsafe`, because Slang
/// does not check the value of an option, and a value that it does not
/// expect is undefined behaviour inside Slang.
///
/// Do not set these options through the raw setters. Some make Slang write
/// files or print outside the diagnostics, which this crate cannot return:
/// every option whose name starts with `Dump` or `Report`,
/// `PreprocessorOutput`, and `OutputIncludes`. Others serve only Slang's
/// command line or its own build, and Slang documents no effect of them on
/// its library: the options of the command line, the options that build the
/// core module, Slang's module of built-in types and functions, and the
/// options that write reproduction files.
///
/// # Examples
///
/// This example builds options for an optimized compile without debug
/// information that treats every warning as an error:
///
/// ```
/// use slang_compiler::{CompilerOptions, DebugInfoLevel, OptimizationLevel};
///
/// let mut options = CompilerOptions::default();
/// options
///     .optimization(OptimizationLevel::High)
///     .debug_info(DebugInfoLevel::None)
///     .warnings_as_errors("all");
/// # let _ = options;
/// ```
#[derive(Clone, Debug, Default)]
pub struct CompilerOptions {
    entries: Vec<Entry>,
}

/// The options that the pinned Slang build never reads from the options of a
/// target, with Slang's name of each. It reads `LanguageVersion` only from
/// the session when it parses and checks source, and the three warning
/// options only when it applies the options of the session or of a linked
/// program to its diagnostics.
const SESSION_ONLY: [(CompilerOptionName, &str); 4] = [
    (CompilerOptionName::LanguageVersion, "LanguageVersion"),
    (CompilerOptionName::WarningsAsErrors, "WarningsAsErrors"),
    (CompilerOptionName::EnableWarning, "EnableWarning"),
    (CompilerOptionName::DisableWarning, "DisableWarning"),
];

#[derive(Clone, Debug)]
struct Entry {
    name: CompilerOptionName,
    value: Value,
}

#[derive(Clone, Debug)]
enum Value {
    Int(i32, i32),
    Text(String, String),
}

impl CompilerOptions {
    /// Sets the version of the Slang language whose rules apply to the
    /// source, as Slang's option `LanguageVersion`.
    ///
    /// The pinned Slang build never reads this option from the options of a
    /// target, so [`SessionBuilder::build`](crate::SessionBuilder::build)
    /// rejects a [`Target`](crate::Target) whose options set it. Set it in the
    /// options of the session.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn language_version(&mut self, version: LanguageVersion) -> &mut Self {
        self.replace_int(
            CompilerOptionName::LanguageVersion,
            version.to_raw().0.cast_signed(),
        )
    }

    /// Adds warnings that Slang reports as errors, as Slang's option
    /// `WarningsAsErrors`.
    ///
    /// A warning is a message of Slang about source that compiles but is
    /// probably wrong. `warnings` is a comma-separated list of the names or
    /// numbers of warnings, such as `"30081,unused-variable"`, or `"all"` for
    /// every warning. Slang passes the text on unchanged, and it reports a
    /// name that it does not know in the diagnostics of the call that applies
    /// the option, such as the load of a module.
    ///
    /// The pinned Slang build never reads this option from the options of a
    /// target, so [`SessionBuilder::build`](crate::SessionBuilder::build)
    /// rejects a [`Target`](crate::Target) whose options set it. Set it in the
    /// options of the session.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn warnings_as_errors(&mut self, warnings: impl Into<String>) -> &mut Self {
        self.push_text(CompilerOptionName::WarningsAsErrors, warnings.into())
    }

    /// Adds warnings that Slang reports, which are off by default, as Slang's
    /// option `EnableWarning`.
    ///
    /// `warnings` is a comma-separated list of the names or numbers of
    /// warnings. Slang passes the text on unchanged, and it reports a name
    /// that it does not know in the diagnostics of the call that applies the
    /// option, such as the load of a module.
    ///
    /// The pinned Slang build never reads this option from the options of a
    /// target, so [`SessionBuilder::build`](crate::SessionBuilder::build)
    /// rejects a [`Target`](crate::Target) whose options set it. Set it in the
    /// options of the session.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn enable_warning(&mut self, warnings: impl Into<String>) -> &mut Self {
        self.push_text(CompilerOptionName::EnableWarning, warnings.into())
    }

    /// Adds one warning that Slang does not report, as Slang's option
    /// `DisableWarning`.
    ///
    /// `warning` is the name or the number of one warning, not a list. Slang
    /// passes the text on unchanged, and it reports a name that it does not
    /// know in the diagnostics of the call that applies the option, such as
    /// the load of a module.
    ///
    /// The pinned Slang build never reads this option from the options of a
    /// target, so [`SessionBuilder::build`](crate::SessionBuilder::build)
    /// rejects a [`Target`](crate::Target) whose options set it. Set it in the
    /// options of the session.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn disable_warning(&mut self, warning: impl Into<String>) -> &mut Self {
        self.push_text(CompilerOptionName::DisableWarning, warning.into())
    }

    /// Sets how much Slang optimizes the generated code, as Slang's option
    /// `Optimization`.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn optimization(&mut self, level: OptimizationLevel) -> &mut Self {
        self.replace_int(
            CompilerOptionName::Optimization,
            level.to_raw().0.cast_signed(),
        )
    }

    /// Sets how much debug information Slang puts into the generated code, as
    /// Slang's option `DebugInformation`.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn debug_info(&mut self, level: DebugInfoLevel) -> &mut Self {
        self.replace_int(
            CompilerOptionName::DebugInformation,
            level.to_raw().0.cast_signed(),
        )
    }

    /// Sets how strictly the generated code follows the rules of
    /// floating-point arithmetic, as Slang's option `FloatingPointMode`.
    ///
    /// Slang takes this setting of each target from a field of the target's
    /// descriptor, which overrides the options that the target inherits from
    /// the session. The session builder therefore fills that field from the
    /// options of the target when they set this option, and otherwise from
    /// the options of the session, so the setting applies in both places and
    /// the target's own value wins.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn floating_point_mode(&mut self, mode: FloatingPointMode) -> &mut Self {
        self.replace_int(
            CompilerOptionName::FloatingPointMode,
            mode.to_raw().0.cast_signed(),
        )
    }

    /// Sets whether and how generated source code marks the line of the
    /// Slang source that each part came from, as Slang's option
    /// `LineDirectiveMode`.
    ///
    /// Slang takes this setting of each target from a field of the target's
    /// descriptor, which the session builder fills as it does for
    /// [`CompilerOptions::floating_point_mode`], so the setting applies in the
    /// options of the session and of a target, and the target's own value
    /// wins.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn line_directive_mode(&mut self, mode: LineDirectiveMode) -> &mut Self {
        self.replace_int(
            CompilerOptionName::LineDirectiveMode,
            mode.to_raw().0.cast_signed(),
        )
    }

    /// Sets whether SPIR-V keeps the names of the entry points of the source,
    /// as Slang's option `VulkanUseEntryPointName`.
    ///
    /// An entry point is a function of a shader that a graphics API calls,
    /// like `main` in a program. By default, Slang renames each entry point
    /// in SPIR-V, the binary shader format of Vulkan, to `main`.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn vulkan_use_entry_point_name(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::VulkanUseEntryPointName, toggle)
    }

    /// Sets whether Slang generates SPIR-V directly, as Slang's option
    /// `EmitSpirvDirectly`.
    ///
    /// SPIR-V is the binary shader format of Vulkan. Generating it directly
    /// is Slang's default, so leave the option unset for the default.
    /// [`Toggle::Off`] makes Slang generate source in the OpenGL Shading
    /// Language (GLSL) first and compile that with glslang, the GLSL compiler
    /// that Slang loads as a library. Slang takes the method of each target
    /// from a flag of the target's descriptor, which overrides the method
    /// that the target inherits from the session. The session builder
    /// therefore sets that flag from the options of the target when they set
    /// the method, and otherwise from the options of the session, so the
    /// setting applies in both places and the target's own value wins.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn emit_spirv_directly(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::EmitSpirvDirectly, toggle)
    }

    /// Sets whether Slang skips the validation of the SPIR-V that it
    /// generates, as Slang's option `SkipSPIRVValidation`.
    ///
    /// SPIR-V is the binary shader format of Vulkan. The pinned Slang build
    /// validates the SPIR-V that it generates directly only when the
    /// environment variable `SLANG_RUN_SPIRV_VALIDATION` is `1`, and
    /// [`Toggle::On`] skips that validation even then.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn skip_spirv_validation(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::SkipSPIRVValidation, toggle)
    }

    /// Sets whether every buffer uses scalar layout in SPIR-V and GLSL, as
    /// Slang's option `GLSLForceScalarLayout`.
    ///
    /// In a scalar layout, each value is aligned only to the size of its
    /// scalar elements, which Vulkan allows through an extension. Slang takes
    /// this setting of each target from a field of the target's descriptor,
    /// which the session builder fills as it does for
    /// [`CompilerOptions::floating_point_mode`], so the setting applies in the
    /// options of the session and of a target, and the target's own value
    /// wins. The pinned Slang build reads this option when it lays out types
    /// and generates code for a target, from the options of that target.
    /// In the options of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options),
    /// it changes the generated code but not the layout that reflection
    /// reports, as that method describes.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn glsl_force_scalar_layout(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::GLSLForceScalarLayout, toggle)
    }

    /// Sets whether the fields of buffers follow the layout rules of
    /// DirectX, as Slang's option `ForceDXLayout`.
    ///
    /// The pinned Slang build reads this option when it lays out types for a
    /// target, from the options of that target, which start from the options
    /// of the session.
    /// In the options of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options),
    /// it changes the generated code but not the layout that reflection
    /// reports, as that method describes.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn force_dx_layout(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::ForceDXLayout, toggle)
    }

    /// Sets whether the fields of buffers follow the layout rules of C
    /// structures, as Slang's option `ForceCLayout`.
    ///
    /// The pinned Slang build reads this option when it lays out types for a
    /// target, from the options of that target, which start from the options
    /// of the session.
    /// In the options of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options),
    /// it changes the generated code but not the layout that reflection
    /// reports, as that method describes.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn force_c_layout(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::ForceCLayout, toggle)
    }

    /// Sets whether the generated code keeps every resource parameter, even
    /// one that the code does not use, as Slang's option
    /// `PreserveParameters`.
    ///
    /// A resource is an object that a shader reads or writes through a
    /// binding, such as a buffer or a texture. The pinned Slang build reads
    /// this option from the options of the session when it links the code of
    /// a target, and from the options of the target when it generates the
    /// code, so set it on the session for it to apply to both steps. In the
    /// options of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options),
    /// it changes the generated code as it does on the session.
    ///
    /// # Examples
    ///
    /// See the example of [`CompilerOptions`].
    pub fn preserve_parameters(&mut self, toggle: Toggle) -> &mut Self {
        self.replace_toggle(CompilerOptionName::PreserveParameters, toggle)
    }

    /// Adds an option with two integer values, for an option that has no
    /// typed setter.
    ///
    /// Use it for an option whose value Slang reads as integers. The entry is
    /// added after the entries that the list holds, and for an option that
    /// holds one value, Slang uses the first entry of that option.
    ///
    /// # Examples
    ///
    /// This example turns on the uniformity analysis of Slang. Neither
    /// Slang's header nor its user guide states the kind of the value, and
    /// the pinned Slang build reads the first integer of the entry, as a
    /// Boolean that any value other than 0 turns on:
    ///
    /// ```
    /// use slang_compiler::{CompilerOptions, OptionName};
    /// use slang_compiler_sys::CompilerOptionName;
    ///
    /// let mut options = CompilerOptions::default();
    /// let name = OptionName::new(CompilerOptionName::ValidateUniformity.0)?;
    /// // SAFETY: the pinned Slang build reads this option as an integer, and
    /// // accepts any value.
    /// unsafe { options.set_int(name, 1, 0) };
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees both of these, and breaking either is undefined
    /// behaviour, because Slang checks neither and can index an array or read
    /// a string with a value that it does not expect:
    ///
    /// - Slang reads the option `name` as integers, not as strings.
    /// - `value0` and `value1` are values that Slang accepts for that option,
    ///   such as a value of the enumeration that the option takes.
    pub unsafe fn set_int(&mut self, name: OptionName, value0: i32, value1: i32) -> &mut Self {
        self.entries.push(Entry {
            name: name.0,
            value: Value::Int(value0, value1),
        });
        self
    }

    /// Adds an option with one or two text values, for an option that has no
    /// typed setter.
    ///
    /// Use it for an option whose value Slang reads as text. `value1` is
    /// passed to Slang as empty text when it is `None`. The entry is added
    /// after the entries that the list holds, and for an option that holds
    /// one value, Slang uses the first entry of that option. A NUL byte in a
    /// value is reported when the options are used, by the call that takes
    /// them.
    ///
    /// # Examples
    ///
    /// This example adds a macro, a name that Slang's preprocessor replaces
    /// with a value, through the raw option `MacroDefine`, which
    /// [`SessionBuilder::define`](crate::SessionBuilder::define) also sets:
    ///
    /// ```
    /// use slang_compiler::{CompilerOptions, OptionName};
    /// use slang_compiler_sys::CompilerOptionName;
    ///
    /// let mut options = CompilerOptions::default();
    /// let name = OptionName::new(CompilerOptionName::MacroDefine.0)?;
    /// // SAFETY: Slang reads this option as two strings, the name and the
    /// // value of the macro, and accepts any name and value.
    /// unsafe { options.set_string(name, "BLOCK_SIZE", Some("64")) };
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees both of these, and breaking either is undefined
    /// behaviour, because Slang checks neither and can index an array or read
    /// past a value that it does not expect:
    ///
    /// - Slang reads the option `name` as strings, not as integers.
    /// - `value0` and `value1` are values that Slang accepts for that option.
    pub unsafe fn set_string(
        &mut self,
        name: OptionName,
        value0: &str,
        value1: Option<&str>,
    ) -> &mut Self {
        self.entries.push(Entry {
            name: name.0,
            value: Value::Text(value0.to_owned(), value1.unwrap_or_default().to_owned()),
        });
        self
    }

    fn replace_int(&mut self, name: CompilerOptionName, value: i32) -> &mut Self {
        self.entries.retain(|entry| entry.name != name);
        self.entries.push(Entry {
            name,
            value: Value::Int(value, 0),
        });
        self
    }

    fn replace_toggle(&mut self, name: CompilerOptionName, toggle: Toggle) -> &mut Self {
        self.replace_int(name, i32::from(toggle.is_on()))
    }

    fn push_text(&mut self, name: CompilerOptionName, text: String) -> &mut Self {
        self.entries.push(Entry {
            name,
            value: Value::Text(text, String::new()),
        });
        self
    }

    /// Returns the first integer of the first entry of `name`, which is the
    /// value that Slang reads for an option that holds one value, or `None`
    /// when no entry sets the option. Slang reads 0 from an entry of kind
    /// `String`.
    pub(crate) fn first_int(&self, name: CompilerOptionName) -> Option<i32> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(Entry::int0)
    }

    /// Returns Slang's name of the first option in these options that the
    /// pinned Slang build reads only from the options of a session, or `None`
    /// when they set none.
    pub(crate) fn first_session_only(&self) -> Option<&'static str> {
        self.entries.iter().find_map(|entry| {
            SESSION_ONLY
                .iter()
                .find(|(name, _)| *name == entry.name)
                .map(|(_, text)| *text)
        })
    }

    /// Returns the method of generating SPIR-V that Slang derives from these
    /// options when it loads them, as a value of `SlangEmitSpirvMethod`, or
    /// `None` when no entry sets it.
    pub(crate) fn spirv_method(&self) -> Option<i32> {
        const DEFAULT: i32 = SlangEmitSpirvMethod::SLANG_EMIT_SPIRV_DEFAULT
            .0
            .cast_signed();
        const VIA_GLSL: i32 = SlangEmitSpirvMethod::SLANG_EMIT_SPIRV_VIA_GLSL
            .0
            .cast_signed();
        const DIRECTLY: i32 = SlangEmitSpirvMethod::SLANG_EMIT_SPIRV_DIRECTLY
            .0
            .cast_signed();
        let mut method: Vec<i32> = Vec::new();
        for entry in &self.entries {
            let value = entry.int0();
            match entry.name {
                CompilerOptionName::EmitSpirvMethod => method.push(value),
                CompilerOptionName::EmitSpirvDirectly => {
                    method = vec![if value == 0 { VIA_GLSL } else { DIRECTLY }];
                }
                CompilerOptionName::EmitSpirvViaGLSL
                    if method.first().copied().unwrap_or(DEFAULT) == DEFAULT && value != 0 =>
                {
                    method = vec![VIA_GLSL];
                }
                _ => {}
            }
        }
        method.first().copied()
    }
}

impl Entry {
    fn int0(&self) -> i32 {
        match self.value {
            Value::Int(value0, _) => value0,
            Value::Text(..) => 0,
        }
    }
}

/// Compiler options with their text converted to C strings, which own the
/// memory that the raw entries of [`PreparedOptions::raw`] point to.
#[derive(Debug, Default)]
pub(crate) struct PreparedOptions {
    entries: Vec<PreparedEntry>,
}

#[derive(Debug)]
struct PreparedEntry {
    name: CompilerOptionName,
    value: PreparedValue,
}

#[derive(Debug)]
enum PreparedValue {
    Int(i32, i32),
    Text(CString, CString),
}

impl PreparedOptions {
    /// Adds an option with one integer value.
    pub(crate) fn push_int(&mut self, name: CompilerOptionName, value: i32) {
        self.entries.push(PreparedEntry {
            name,
            value: PreparedValue::Int(value, 0),
        });
    }

    /// Adds the entries of `options` after the entries that this holds, and
    /// reports a NUL byte in a value as an error of `operation`.
    pub(crate) fn append(&mut self, options: &CompilerOptions, operation: Operation) -> Result<()> {
        for entry in &options.entries {
            let value = match &entry.value {
                Value::Int(value0, value1) => PreparedValue::Int(*value0, *value1),
                Value::Text(value0, value1) => {
                    let what = format!("the value of compiler option {}", entry.name.0);
                    PreparedValue::Text(
                        c_string(value0, operation, &what)?,
                        c_string(value1, operation, &what)?,
                    )
                }
            };
            self.entries.push(PreparedEntry {
                name: entry.name,
                value,
            });
        }
        Ok(())
    }

    /// Returns Slang's entries for these options, which point into the C
    /// strings of `self` and so cannot outlive it.
    pub(crate) fn raw(&self, operation: Operation) -> Result<RawOptions<'_>> {
        let count = u32::try_from(self.entries.len()).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                "{} compiler options are more than Slang takes: {error}",
                self.entries.len()
            ))
        })?;
        Ok(RawOptions {
            entries: self.entries.iter().map(PreparedEntry::raw).collect(),
            count,
            prepared: PhantomData,
        })
    }
}

impl PreparedEntry {
    fn raw(&self) -> CompilerOptionEntry {
        let value = match &self.value {
            PreparedValue::Int(value0, value1) => CompilerOptionValue {
                kind: CompilerOptionValueKind::Int,
                intValue0: *value0,
                intValue1: *value1,
                stringValue0: c"".as_ptr(),
                stringValue1: c"".as_ptr(),
            },
            PreparedValue::Text(value0, value1) => CompilerOptionValue {
                kind: CompilerOptionValueKind::String,
                intValue0: 0,
                intValue1: 0,
                stringValue0: value0.as_ptr(),
                stringValue1: value1.as_ptr(),
            },
        };
        CompilerOptionEntry {
            name: self.name,
            value,
        }
    }
}

/// Slang's entries for a list of compiler options, whose strings are owned
/// by the [`PreparedOptions`] that the lifetime borrows, or are static.
#[derive(Debug)]
pub(crate) struct RawOptions<'prepared> {
    entries: Vec<CompilerOptionEntry>,
    count: u32,
    prepared: PhantomData<&'prepared PreparedOptions>,
}

impl RawOptions<'_> {
    /// Returns the pointer to the first entry, or null when there is none,
    /// valid while `self` lives.
    pub(crate) fn as_ptr(&self) -> *const CompilerOptionEntry {
        if self.entries.is_empty() {
            ptr::null()
        } else {
            self.entries.as_ptr()
        }
    }

    /// Returns the number of entries.
    pub(crate) fn count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::CompilerOptionName;

    use super::{CompilerOptions, OptionName, PreparedOptions};
    use crate::fake::{OptionSnapshot, read_options};
    use crate::{
        DebugInfoLevel, ErrorKind, FloatingPointMode, LanguageVersion, LineDirectiveMode,
        Operation, OptimizationLevel, Toggle,
    };

    fn encode(options: &CompilerOptions) -> Vec<OptionSnapshot> {
        let mut prepared = PreparedOptions::default();
        prepared
            .append(options, Operation::CreateSession)
            .expect("the options have no NUL byte");
        let raw = prepared
            .raw(Operation::CreateSession)
            .expect("the count fits");
        // SAFETY: `raw` points to `count` entries whose strings `prepared` or
        // static literals own, and both outlive this call.
        unsafe { read_options(raw.as_ptr(), raw.count()) }
    }

    fn int(name: CompilerOptionName, value: i32) -> OptionSnapshot {
        OptionSnapshot::Int(name.0, value, 0)
    }

    fn text(name: CompilerOptionName, value: &str) -> OptionSnapshot {
        OptionSnapshot::Text(name.0, value.to_owned(), String::new())
    }

    #[test]
    fn every_typed_option_encodes_as_the_entry_that_slang_documents() {
        let mut options = CompilerOptions::default();
        options
            .language_version(LanguageVersion::V2026)
            .warnings_as_errors("all")
            .enable_warning("30081,unused-variable")
            .disable_warning("30081")
            .optimization(OptimizationLevel::Maximal)
            .debug_info(DebugInfoLevel::Standard)
            .floating_point_mode(FloatingPointMode::Precise)
            .line_directive_mode(LineDirectiveMode::SourceMap)
            .vulkan_use_entry_point_name(Toggle::On)
            .emit_spirv_directly(Toggle::Off)
            .skip_spirv_validation(Toggle::On)
            .glsl_force_scalar_layout(Toggle::On)
            .force_dx_layout(Toggle::Off)
            .force_c_layout(Toggle::On)
            .preserve_parameters(Toggle::On);
        assert_eq!(
            encode(&options),
            [
                int(CompilerOptionName(107), 2026),
                text(CompilerOptionName(19), "all"),
                text(CompilerOptionName(21), "30081,unused-variable"),
                text(CompilerOptionName(22), "30081"),
                int(CompilerOptionName(46), 3),
                int(CompilerOptionName(44), 2),
                int(CompilerOptionName(43), 2),
                int(CompilerOptionName(45), 4),
                int(CompilerOptionName(52), 1),
                int(CompilerOptionName(58), 0),
                int(CompilerOptionName(29), 1),
                int(CompilerOptionName(55), 1),
                int(CompilerOptionName(117), 0),
                int(CompilerOptionName(129), 1),
                int(CompilerOptionName(38), 1),
            ]
        );
    }

    #[test]
    fn the_numbers_of_the_typed_options_match_the_pinned_header() {
        let names = [
            (CompilerOptionName::LanguageVersion, 107),
            (CompilerOptionName::WarningsAsErrors, 19),
            (CompilerOptionName::EnableWarning, 21),
            (CompilerOptionName::DisableWarning, 22),
            (CompilerOptionName::Optimization, 46),
            (CompilerOptionName::DebugInformation, 44),
            (CompilerOptionName::FloatingPointMode, 43),
            (CompilerOptionName::LineDirectiveMode, 45),
            (CompilerOptionName::VulkanUseEntryPointName, 52),
            (CompilerOptionName::EmitSpirvDirectly, 58),
            (CompilerOptionName::SkipSPIRVValidation, 29),
            (CompilerOptionName::GLSLForceScalarLayout, 55),
            (CompilerOptionName::ForceDXLayout, 117),
            (CompilerOptionName::ForceCLayout, 129),
            (CompilerOptionName::PreserveParameters, 38),
            (CompilerOptionName::UseUpToDateBinaryModule, 115),
            (CompilerOptionName::Capability, 39),
        ];
        for (name, value) in names {
            assert_eq!(name.0, value);
        }
    }

    #[test]
    fn a_typed_setter_replaces_the_earlier_value_and_warnings_accumulate() {
        let mut options = CompilerOptions::default();
        options
            .optimization(OptimizationLevel::None)
            .warnings_as_errors("30081")
            .optimization(OptimizationLevel::High)
            .warnings_as_errors("unused-variable");
        assert_eq!(
            encode(&options),
            [
                text(CompilerOptionName::WarningsAsErrors, "30081"),
                int(CompilerOptionName::Optimization, 2),
                text(CompilerOptionName::WarningsAsErrors, "unused-variable"),
            ]
        );
    }

    #[test]
    fn a_typed_setter_replaces_a_raw_entry_of_the_same_option() {
        let mut options = CompilerOptions::default();
        let name = OptionName::new(CompilerOptionName::Optimization.0).expect("a stored option");
        // SAFETY: Slang reads `Optimization` as an integer of
        // `SlangOptimizationLevel`, and 0 is `SLANG_OPTIMIZATION_LEVEL_NONE`.
        unsafe { options.set_int(name, 0, 0) };
        options.optimization(OptimizationLevel::Default);
        assert_eq!(encode(&options), [int(CompilerOptionName::Optimization, 1)]);
    }

    #[test]
    fn raw_setters_add_entries_with_non_null_strings() {
        let mut options = CompilerOptions::default();
        let define = OptionName::new(CompilerOptionName::MacroDefine.0).expect("a stored option");
        let include = OptionName::new(CompilerOptionName::Include.0).expect("a stored option");
        let shift =
            OptionName::new(CompilerOptionName::VulkanBindShiftAll.0).expect("a stored option");
        // SAFETY: Slang reads `MacroDefine` and `Include` as strings and
        // `VulkanBindShiftAll` as two integers, and accepts these values.
        unsafe { options.set_string(define, "SIZE", Some("64")) };
        // SAFETY: as above.
        unsafe { options.set_string(include, "shaders", None) };
        // SAFETY: as above.
        unsafe { options.set_int(shift, 2, 10) };
        assert_eq!(
            encode(&options),
            [
                OptionSnapshot::Text(0, "SIZE".to_owned(), "64".to_owned()),
                OptionSnapshot::Text(6, "shaders".to_owned(), String::new()),
                OptionSnapshot::Int(CompilerOptionName::VulkanBindShiftAll.0, 2, 10),
            ]
        );
    }

    #[test]
    fn a_nul_byte_in_a_value_fails_when_the_options_are_prepared() {
        let mut options = CompilerOptions::default();
        options.enable_warning("30081\0");
        let mut prepared = PreparedOptions::default();
        let error = prepared
            .append(&options, Operation::CreateSession)
            .expect_err("a NUL byte fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::CreateSession);
        assert!(error.to_string().contains("compiler option 21"), "{error}");
    }

    #[test]
    fn a_nul_byte_in_the_second_raw_value_fails_too() {
        let mut options = CompilerOptions::default();
        let define = OptionName::new(CompilerOptionName::MacroDefine.0).expect("a stored option");
        // SAFETY: Slang reads `MacroDefine` as two strings. The options are
        // only prepared, never passed to Slang.
        unsafe { options.set_string(define, "SIZE", Some("6\u{0}4")) };
        let error = PreparedOptions::default()
            .append(&options, Operation::CreateSession)
            .expect_err("a NUL byte fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }

    #[test]
    fn empty_options_have_a_null_pointer_and_no_entry() {
        let prepared = PreparedOptions::default();
        let raw = prepared.raw(Operation::CreateSession).expect("no entry");
        assert!(raw.as_ptr().is_null());
        assert_eq!(raw.count(), 0);
    }

    #[test]
    fn option_name_accepts_every_stored_option_below_the_sentinel() {
        for value in [0, 1, 78, 80, 110, 112, 158] {
            assert!(OptionName::new(value).is_ok(), "{value}");
        }
        assert_eq!(CompilerOptionName::CountOf.0, 160);
    }

    #[test]
    fn option_name_rejects_the_sentinel_values_above_and_negatives() {
        for value in [-1, i32::MIN, 79, 111, 159, 160, 161, i32::MAX] {
            let error = OptionName::new(value).expect_err("the number names no stored option");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{value}");
            assert_eq!(error.operation(), Operation::CreateOptionName);
            assert_eq!(error.code(), None);
        }
        assert_eq!(
            OptionName::new(79).map_err(|error| error.to_string()),
            Err(
                "cannot name compiler option `79`: the number names no compiler option that the pinned Slang build stores"
                    .to_owned()
            )
        );
    }

    #[test]
    fn option_name_excludes_exactly_the_three_values_of_the_header() {
        assert_eq!(CompilerOptionName::REMOVED_SerialIR.0, 79);
        assert_eq!(CompilerOptionName::CountOfParsableOptions.0, 111);
        assert_eq!(CompilerOptionName::GetCompilerPath.0, 159);
        let rejected = (0..CompilerOptionName::CountOf.0)
            .filter(|value| OptionName::new(*value).is_err())
            .collect::<Vec<_>>();
        assert_eq!(rejected, [79, 111, 159]);
    }
}
