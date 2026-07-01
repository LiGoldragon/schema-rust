use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{ToTokens, quote};
use schema::{
    Declaration, EnumDeclaration, EnumVariant, FamilyDeclaration, FamilyKey, FieldDeclaration,
    ImplFact, ImplReference, ImportResolver, MethodParameter, MethodSignature, Name,
    NewtypeDeclaration, ReferencedImpl, RelationDeclaration, RelationValue, ResolvedImport,
    RootApplication, RustSurface, Schema, SchemaEngine, SchemaError, SchemaIdentity, SchemaSource,
    SpecifiedDeclaration, SpecifiedDeclarationBody, SpecifiedField, SpecifiedRoot,
    SpecifiedRootApplication, SpecifiedRootEnum, SpecifiedSchema, SpecifiedVariant,
    StreamDeclaration, StructDeclaration, TypeDeclaration, TypeReference, Visibility,
};

pub mod build;
pub mod daemon_emit;
pub mod migration;
pub use daemon_emit::{
    DaemonModule, MetaListenerTier, NexusDaemonShape, SocketModeBits, TcpListenerTier,
    UpgradeListenerTier, WorkingListenerTier,
};
pub use migration::{DefaultRenderer, MigrationEmitter, TypeRenderer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedFile {
    pub path: String,
    pub code: RustCode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCode(String);

impl RustCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) struct RustfmtSkippedItems {
    file: syn::File,
}

impl RustfmtSkippedItems {
    pub(crate) fn new(file: syn::File) -> Self {
        Self { file }
    }

    pub(crate) fn render(self) -> String {
        let mut output = String::new();
        for item in self.file.items {
            let file = syn::File {
                shebang: None,
                attrs: Vec::new(),
                items: vec![item],
            };
            output.push_str("#[rustfmt::skip]\n");
            output.push_str(prettyplease::unparse(&file).trim_end());
            output.push('\n');
        }
        output
    }
}

#[derive(Clone, Debug)]
pub struct RustEmitter {
    generator_name: &'static str,
    options: RustEmissionOptions,
}

impl Default for RustEmitter {
    fn default() -> Self {
        Self {
            generator_name: "schema-rust",
            options: RustEmissionOptions::default(),
        }
    }
}

impl RustEmitter {
    pub fn new(options: RustEmissionOptions) -> Self {
        Self {
            generator_name: "schema-rust",
            options,
        }
    }

    pub fn emit_file_from_schema(&self, schema: &Schema) -> GeneratedFile {
        schema.lower_to_rust_file(self)
    }

    pub fn emit_file_from_specified_schema(&self, schema: &SpecifiedSchema) -> GeneratedFile {
        schema.lower_to_rust_file(self)
    }

    pub fn emit_file_from_schema_source(
        &self,
        source: &SchemaSource,
        identity: SchemaIdentity,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
    ) -> Result<GeneratedFile, SchemaError> {
        source.lower_to_rust_file(identity, engine, resolver, self)
    }

    pub fn emit_code_from_schema(&self, schema: &Schema) -> RustCode {
        schema.lower_to_rust_code(self)
    }

    pub fn emit_code_from_specified_schema(&self, schema: &SpecifiedSchema) -> RustCode {
        schema.lower_to_rust_code(self)
    }

    pub fn emit_module_from_schema(&self, schema: &Schema) -> RustModule {
        schema.lower_to_rust_module(self)
    }

    pub fn emit_module_from_specified_schema(&self, schema: &SpecifiedSchema) -> RustModule {
        schema.lower_to_rust_module(self)
    }

    pub fn emit_module_from_schema_source(
        &self,
        source: &SchemaSource,
        identity: SchemaIdentity,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
    ) -> Result<RustModule, SchemaError> {
        source.lower_to_rust_module(identity, engine, resolver, self)
    }
}

pub trait RustSchemaLowering {
    fn lower_to_rust_file(&self, emitter: &RustEmitter) -> GeneratedFile;
    fn lower_to_rust_code(&self, emitter: &RustEmitter) -> RustCode;
    fn lower_to_rust_module(&self, emitter: &RustEmitter) -> RustModule;
}

impl RustSchemaLowering for Schema {
    fn lower_to_rust_file(&self, emitter: &RustEmitter) -> GeneratedFile {
        SpecifiedSchema::from(self).lower_to_rust_file(emitter)
    }

    fn lower_to_rust_code(&self, emitter: &RustEmitter) -> RustCode {
        SpecifiedSchema::from(self).lower_to_rust_code(emitter)
    }

    fn lower_to_rust_module(&self, emitter: &RustEmitter) -> RustModule {
        SpecifiedSchema::from(self).lower_to_rust_module(emitter)
    }
}

impl RustSchemaLowering for SpecifiedSchema {
    fn lower_to_rust_file(&self, emitter: &RustEmitter) -> GeneratedFile {
        let module = self.lower_to_rust_module(emitter);
        GeneratedFile {
            path: module.file_path().to_owned(),
            code: module.render(),
        }
    }

    fn lower_to_rust_code(&self, emitter: &RustEmitter) -> RustCode {
        self.lower_to_rust_module(emitter).render()
    }

    fn lower_to_rust_module(&self, emitter: &RustEmitter) -> RustModule {
        let context = RustLoweringContext::from_emitter(emitter);
        self.lower_to_rust(&context)
    }
}

pub trait RustSchemaSourceLowering {
    fn lower_to_rust_file(
        &self,
        identity: SchemaIdentity,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
        emitter: &RustEmitter,
    ) -> Result<GeneratedFile, SchemaError>;

    fn lower_to_rust_module(
        &self,
        identity: SchemaIdentity,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
        emitter: &RustEmitter,
    ) -> Result<RustModule, SchemaError>;
}

impl RustSchemaSourceLowering for SchemaSource {
    fn lower_to_rust_file(
        &self,
        identity: SchemaIdentity,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
        emitter: &RustEmitter,
    ) -> Result<GeneratedFile, SchemaError> {
        let module = self.lower_to_rust_module(identity, engine, resolver, emitter)?;
        Ok(GeneratedFile {
            path: module.file_path().to_owned(),
            code: module.render(),
        })
    }

    fn lower_to_rust_module(
        &self,
        identity: SchemaIdentity,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
        emitter: &RustEmitter,
    ) -> Result<RustModule, SchemaError> {
        let schema = engine.lower_schema_source_with_resolver(self, identity, resolver)?;
        let module = schema.lower_to_rust_module(emitter);
        // Emission boundary: a malformed schema name (NOTA accepts symbol atoms
        // Rust rejects as identifiers) becomes a typed error here instead of a
        // panic at `Ident::new`, and the recognized `{| … |}` catalog subset is
        // verified against the surface the module actually emits.
        module.verify_names()?;
        module.verify_catalog(&schema)?;
        Ok(module)
    }
}

#[derive(Clone, Debug)]
pub struct RustLoweringContext {
    generator_name: String,
    options: RustEmissionOptions,
}

impl RustLoweringContext {
    pub fn new(generator_name: impl Into<String>, options: RustEmissionOptions) -> Self {
        Self {
            generator_name: generator_name.into(),
            options,
        }
    }

    pub fn from_emitter(emitter: &RustEmitter) -> Self {
        Self::new(emitter.generator_name, emitter.options.clone())
    }

    fn generator_name(&self) -> &str {
        &self.generator_name
    }

    fn options(&self) -> RustEmissionOptions {
        self.options.clone()
    }

    fn lower_specified_declaration(
        &self,
        schema: &SpecifiedSchema,
        declaration: &SpecifiedDeclaration,
    ) -> RustDeclaration {
        if let Some(expanded) = self.expand_specified_newtype_frame_application(schema, declaration)
        {
            return expanded;
        }
        declaration.lower_to_rust(self)
    }

    fn expand_specified_newtype_frame_application(
        &self,
        schema: &SpecifiedSchema,
        declaration: &SpecifiedDeclaration,
    ) -> Option<RustDeclaration> {
        if !declaration.parameters().is_empty() {
            return None;
        }
        let SpecifiedDeclarationBody::Newtype(reference) = declaration.body() else {
            return None;
        };
        let TypeReference::Application { head, arguments } = reference else {
            return None;
        };
        let variants = if let Some(frame) = schema.declaration_named(head.name().as_str()) {
            let SpecifiedDeclarationBody::Enum(variants) = frame.body() else {
                return None;
            };
            self.expand_specified_frame_variants(schema, frame.parameters(), arguments, variants)
        } else {
            let import = schema
                .resolved_imports()
                .iter()
                .find(|import| import.local_name() == head.name())?;
            self.expand_imported_frame_variants(
                schema,
                import.parameters(),
                arguments,
                import.variants(),
            )
        };
        Some(RustDeclaration {
            visibility: declaration.visibility(),
            name: declaration.name().clone(),
            parameters: Vec::new(),
            value: RustTypeDeclaration::Enum(RustEnum {
                name: declaration.name().clone(),
                parameters: Vec::new(),
                variants,
            }),
        })
    }

    fn expand_specified_frame_variants(
        &self,
        schema: &SpecifiedSchema,
        parameters: &[Name],
        arguments: &[TypeReference],
        variants: &[SpecifiedVariant],
    ) -> Vec<RustEnumVariant> {
        variants
            .iter()
            .map(|variant| RustEnumVariant {
                name: variant.name().clone(),
                payload: variant.payload().map(|payload| {
                    self.reaim_sibling_application(
                        schema,
                        &self.substitute_frame_binder(parameters, arguments, payload.reference()),
                    )
                }),
            })
            .collect()
    }

    fn expand_imported_frame_variants(
        &self,
        schema: &SpecifiedSchema,
        parameters: &[Name],
        arguments: &[TypeReference],
        variants: &[EnumVariant],
    ) -> Vec<RustEnumVariant> {
        variants
            .iter()
            .map(|variant| RustEnumVariant {
                name: variant.name.clone(),
                payload: variant.payload.as_ref().map(|payload| {
                    self.reaim_sibling_application(
                        schema,
                        &self.substitute_frame_binder(parameters, arguments, payload),
                    )
                }),
            })
            .collect()
    }

    fn substitute_frame_binder(
        &self,
        parameters: &[Name],
        arguments: &[TypeReference],
        payload: &TypeReference,
    ) -> TypeReference {
        let TypeReference::Plain(name) = payload else {
            return payload.clone();
        };
        parameters
            .iter()
            .position(|parameter| parameter == name)
            .and_then(|index| arguments.get(index))
            .cloned()
            .unwrap_or_else(|| payload.clone())
    }

    fn reaim_sibling_application(
        &self,
        schema: &SpecifiedSchema,
        payload: &TypeReference,
    ) -> TypeReference {
        for root in [schema.input(), schema.output()] {
            if let SpecifiedRoot::Application(application) = root
                && application.reference() == payload
            {
                return TypeReference::Plain(application.name().clone());
            }
        }
        payload.clone()
    }
}

pub trait LowerToRust<Target> {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> Target;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustModule {
    file_path: String,
    generator_name: String,
    scalar_aliases: Vec<RustScalarAlias>,
    imports: Vec<RustImport>,
    declarations: Vec<RustDeclaration>,
    root_enums: Vec<RustEnum>,
    applied_roots: Vec<RustAppliedRoot>,
    streams: Vec<StreamDeclaration>,
    relations: Vec<RustRelation>,
    versioned_store: RustVersionedStore,
    support: RustSupportModel,
    referenced_impls: Vec<RustImplReference>,
    options: RustEmissionOptions,
}

impl RustModule {
    pub fn from_schema(
        schema: &Schema,
        generator_name: impl Into<String>,
        options: RustEmissionOptions,
    ) -> Self {
        Self::from_specified_schema(&SpecifiedSchema::from(schema), generator_name, options)
    }

    pub fn from_specified_schema(
        schema: &SpecifiedSchema,
        generator_name: impl Into<String>,
        options: RustEmissionOptions,
    ) -> Self {
        let context = RustLoweringContext::new(generator_name, options);
        schema.lower_to_rust(&context)
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn declarations(&self) -> &[RustDeclaration] {
        &self.declarations
    }

    pub fn scalar_aliases(&self) -> &[RustScalarAlias] {
        &self.scalar_aliases
    }

    pub fn imports(&self) -> &[RustImport] {
        &self.imports
    }

    pub fn root_enums(&self) -> &[RustEnum] {
        &self.root_enums
    }

    pub fn streams(&self) -> &[StreamDeclaration] {
        &self.streams
    }

    pub fn relations(&self) -> &[RustRelation] {
        &self.relations
    }

    pub fn versioned_store(&self) -> &RustVersionedStore {
        &self.versioned_store
    }

    pub fn declaration_named(&self, name: &str) -> Option<&RustDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
    }

    pub fn referenced_impls(&self) -> &[RustImplReference] {
        &self.referenced_impls
    }

    /// The type names the `{| … |}` catalog references an ordering-class trait
    /// (`Ord`/`PartialOrd` on a non-integer shape) for — the set that gets the
    /// `PartialOrd, Ord` derive folded in. Integer `PartialOrd` is a body, not
    /// a derive, so it is excluded here (the recipe emits the comparison impl).
    fn ordering_type_names(&self) -> Vec<String> {
        self.referenced_impls
            .iter()
            .filter_map(|reference| {
                let trait_name = reference.entry().trait_name()?;
                let shape = self.target_scalar_shape(reference.target());
                StandardImplRecipe::new(reference.target().clone(), trait_name.clone(), shape)
                    .is_derive_class()
                    .then(|| reference.target().as_str().to_owned())
            })
            .collect()
    }

    /// The backing scalar shape of a referenced impl's target, resolved through
    /// the module's newtype chain. A non-newtype target (struct, enum, absent)
    /// resolves to [`ScalarShape::NonScalar`].
    fn target_scalar_shape(&self, target: &Name) -> ScalarShape {
        match self
            .declarations
            .iter()
            .find(|declaration| declaration.name() == target)
            .map(RustDeclaration::value)
        {
            Some(RustTypeDeclaration::Newtype(newtype)) => {
                ScalarShape::resolve(newtype.reference(), &self.declarations)
            }
            _ => ScalarShape::NonScalar,
        }
    }

    /// Validate every emitted Rust identifier — type names, field names, enum
    /// variants, generic parameters — as a legal Rust identifier BEFORE any
    /// `ToTokens` runs. NOTA accepts a far broader symbol atom than Rust accepts
    /// as an identifier (`Foo-Bar`, `2Things`, `A/B` all parse as schema
    /// names), so a malformed name would otherwise reach `Ident::new` and PANIC.
    /// This boundary turns that panic into a typed [`SchemaError`] naming the
    /// offending identifier.
    pub fn verify_names(&self) -> Result<(), SchemaError> {
        for declaration in &self.declarations {
            RustIdentifier::verify(declaration.name(), "type")?;
            for parameter in declaration.parameters() {
                RustIdentifier::verify(parameter, "type parameter")?;
            }
            match declaration.value() {
                RustTypeDeclaration::Struct(structure) => {
                    for field in structure.fields() {
                        RustIdentifier::verify_field(field.name())?;
                    }
                }
                RustTypeDeclaration::Enum(enumeration) => {
                    for variant in enumeration.variants() {
                        RustIdentifier::verify(variant.name(), "enum variant")?;
                    }
                }
                RustTypeDeclaration::Newtype(_) => {}
            }
        }
        for root_enum in &self.root_enums {
            RustIdentifier::verify(root_enum.name(), "root enum")?;
            for variant in root_enum.variants() {
                RustIdentifier::verify(variant.name(), "enum variant")?;
            }
        }
        for applied_root in &self.applied_roots {
            RustIdentifier::verify(applied_root.name(), "applied root")?;
        }
        Ok(())
    }

    /// Verify the `{| … |}` catalog against the Rust surface this module
    /// ACTUALLY emits. This is the half that turns
    /// [`schema::RustSurface::verify_catalog`] from a test-only check into
    /// a build invariant: the facts come from [`EmittedRustSurface::from`]
    /// walking `self` (the standard impls Move 3 emitted, the intrinsic newtype
    /// inherents, and the ordering-class derives), not a hand-built test vector.
    ///
    /// The two-tier trust boundary: a RECOGNIZED reference (a standard trait
    /// under a scalar shape, or an ordering-class derive) contributes a fact to
    /// the surface only when the generator genuinely emits/derives it — so a
    /// recognized reference whose body is missing fails verification with
    /// [`SchemaError::UnverifiedImplReference`] naming the exact target. An
    /// UNRECOGNIZED reference (a hand-written runtime trait or inherent method)
    /// is passed through as externally-provided-unverified — its facts are
    /// trusted into the surface so the check does not reject the crate-provided
    /// impl. The full real-crate scan that would verify those too is a named
    /// follow-up, not silently claimed here.
    pub fn verify_catalog(&self, schema: &Schema) -> Result<(), SchemaError> {
        EmittedRustSurface::for_schema(self, schema)
            .into_surface()
            .verify_catalog(schema)
    }

    /// Whether the generator OWNS a body (or derive) for this catalog entry on
    /// this target — the recognized closed set under a scalar-backed shape, or
    /// an ordering-class derive. An entry the generator does not recognize is
    /// trusted to the crate and passed through unverified.
    fn recognizes(&self, target: &Name, entry: &ImplReference) -> bool {
        let Some(trait_name) = entry.trait_name() else {
            return false;
        };
        let shape = self.target_scalar_shape(target);
        let recipe = StandardImplRecipe::new(target.clone(), trait_name.clone(), shape);
        recipe.recipe().is_some() || recipe.is_derive_class()
    }

    /// The [`ImplFact`]s the generated Rust surface genuinely exposes for one
    /// target newtype: the standard recipe bodies the catalog drove, the
    /// ordering-class derive when the catalog references it, and the intrinsic
    /// newtype inherents (`new` / `payload` / `into_payload`). Only facts the
    /// generator actually emits are added — a recognized-but-absent reference
    /// therefore has no fact, and verification catches it.
    fn emitted_facts(&self) -> Vec<ImplFact> {
        let mut facts = Vec::new();
        for reference in &self.referenced_impls {
            let Some(trait_name) = reference.entry().trait_name() else {
                continue;
            };
            let shape = self.target_scalar_shape(reference.target());
            let recipe =
                StandardImplRecipe::new(reference.target().clone(), trait_name.clone(), shape);
            if let Some(body) = recipe.recipe() {
                facts.push(body.fact());
            } else if recipe.is_derive_class() {
                facts.push(ImplFact::trait_impl(
                    reference.target().clone(),
                    trait_name.clone(),
                ));
            }
        }
        facts
    }

    pub fn render(&self) -> RustCode {
        let mut writer = RustModuleRenderer::new(self.options.clone());
        writer.note_map_key_types(self.support.map_key_type_names().to_vec());
        writer.note_ordering_types(self.ordering_type_names());
        writer.note_private_type_names(self.support.private_type_names().to_vec());
        writer.line(format!("// @generated by {}", self.generator_name));
        writer.blank();
        for alias in &self.scalar_aliases {
            writer.emit_scalar_alias(alias);
        }
        if self.support.references_bytes() {
            writer.emit_bytes_scalar();
        }
        if self.support.references_fixed_bytes() {
            writer.emit_fixed_bytes_scalar();
        }
        writer.blank();
        writer.emit_imports(&self.imports);
        writer.emit_nota_support();
        if writer.nota_surface().emits_nota() {
            writer.blank();
        }

        for declaration in &self.declarations {
            writer.emit_type(declaration, &self.declarations);
            writer.blank();
        }

        if writer.emits_root_enums() {
            for root_enum in &self.root_enums {
                writer.emit_root_enum(root_enum);
                writer.blank();
            }
        }

        for applied_root in &self.applied_roots {
            writer.emit_applied_root(applied_root);
            writer.blank();
        }

        writer.emit_newtype_inherent_impls(&self.declarations);
        writer.emit_catalog_impls(&self.referenced_impls, &self.declarations);
        writer.emit_enum_variant_constructors(
            &self.declarations,
            writer.emitted_root_enums(&self.root_enums),
        );
        writer.emit_enum_payload_from_impls(
            &self.declarations,
            writer.emitted_root_enums(&self.root_enums),
        );
        if writer.emits_root_enums() {
            for root_enum in &self.root_enums {
                writer.emit_nota_root_enum_support(root_enum);
                writer.blank();
            }
        }
        writer.emit_domain_scope_relation_support(&self.relations, &self.declarations);
        if !self.versioned_store.is_empty() {
            writer.emit_record_family_support(&self.versioned_store);
        }

        if writer.emits_short_headers() {
            writer.emit_short_headers(&self.root_enums);
            writer.blank();
        }
        if writer.emits_wire_frame() {
            writer.emit_signal_frame_codec(&self.root_enums);
            let streaming_event_payload =
                writer.streaming_event_payload(&self.root_enums, &self.streams);
            writer.emit_signal_frame_transport_support(&self.root_enums, streaming_event_payload);
            writer.blank();
            if let Some(event_payload) = streaming_event_payload {
                writer.emit_signal_frame_streaming_support(event_payload);
                writer.blank();
            }
        }
        if writer.emits_runtime_support() {
            writer.emit_plane_route_support(&self.declarations);
            writer.emit_trace_support(&self.declarations, &self.root_enums);
            writer.emit_mail_event_support(&self.root_enums);
            writer.emit_plane_namespaces(&self.declarations, &self.root_enums);
            writer.emit_plane_projection_support(&self.declarations, &self.root_enums);
            writer.emit_runtime_role_trait_impls(&self.declarations, &self.root_enums);
            writer.emit_schema_plane_trait_support(&self.declarations, &self.root_enums);
            writer.emit_upgrade_support();
        }
        RustCode(writer.finish())
    }
}

impl LowerToRust<RustModule> for Schema {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustModule {
        SpecifiedSchema::from(self).lower_to_rust(context)
    }
}

impl LowerToRust<RustModule> for SpecifiedSchema {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustModule {
        let declarations = self
            .declarations()
            .iter()
            .map(|declaration| context.lower_specified_declaration(self, declaration))
            .collect::<Vec<_>>();
        let mut root_enums = Vec::new();
        let mut applied_roots = Vec::new();
        for root in [self.input(), self.output()] {
            match root {
                SpecifiedRoot::Enum(root) => root_enums.push(root.lower_to_rust(context)),
                SpecifiedRoot::Application(application) => match application.expanded() {
                    Some(expanded) => root_enums.push(expanded.lower_to_rust(context)),
                    None => applied_roots.push(application.lower_to_rust(context)),
                },
            }
        }
        RustModule {
            file_path: RustModulePath::new(self.identity().component().clone()).to_file_path(),
            generator_name: context.generator_name().to_owned(),
            scalar_aliases: RustScalarAlias::default_aliases(),
            imports: self
                .resolved_imports()
                .iter()
                .map(|import| RustImport::from_resolved_import(import, self.identity()))
                .collect(),
            declarations,
            root_enums,
            applied_roots,
            streams: self.streams().to_vec(),
            relations: self
                .relations()
                .iter()
                .map(|relation| relation.lower_to_rust(context))
                .collect(),
            versioned_store: <Self as LowerToRust<RustVersionedStore>>::lower_to_rust(
                self, context,
            ),
            support: <Self as LowerToRust<RustSupportModel>>::lower_to_rust(self, context),
            referenced_impls: SpecifiedReferencedImpls::new(self).lower_to_rust(context),
            options: context.options(),
        }
    }
}

/// The emission knobs passed to [`RustEmitter::new`].
///
/// The default is [`NotaSurface::FeatureGated`] with feature name
/// `"nota-text"`. That is the recommended shape per the codec opt-in
/// design: rkyv is the universal base, and NOTA encode/decode are an
/// opt-in surface that text-facing clients (CLIs, launchers, REPLs)
/// enable through a cargo feature. Binary-only consumers (daemons,
/// future binary-only clients) build the contract crate with the
/// default features off and carry no `nota` in their dependency
/// closure. The default target is [`RustEmissionTarget::ComponentRuntime`]
/// so existing all-in-one runtime consumers keep their generated engine traits.
/// New signal and meta-signal contract repos should opt into
/// [`RustEmissionTarget::WireContract`]. Daemon-local signal runtime schemas
/// should opt into [`RustEmissionTarget::SignalRuntime`]. New daemon decision
/// and storage plane schemas should opt into [`RustEmissionTarget::NexusRuntime`]
/// or [`RustEmissionTarget::SemaRuntime`] for per-plane runtime emission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustEmissionOptions {
    pub nota_surface: NotaSurface,
    pub target: RustEmissionTarget,
}

impl Default for RustEmissionOptions {
    fn default() -> Self {
        Self::feature_gated_nota("nota-text")
    }
}

impl RustEmissionOptions {
    /// Always emit `nota::NotaDecode` / `nota::NotaEncode`
    /// derives, the root `FromStr` / `Display` impls, and the `use
    /// nota::*` pull-in — without any cargo-feature gate.
    pub fn always_enabled_nota() -> Self {
        Self {
            nota_surface: NotaSurface::AlwaysEnabled,
            target: RustEmissionTarget::ComponentRuntime,
        }
    }

    /// Emit the NOTA surface guarded by `#[cfg_attr(feature = "<feature>",
    /// derive(...))]` on data types and `#[cfg(feature = "<feature>")]`
    /// on FromStr/Display impls and the `use nota::*` items.
    /// Consumers enable the feature only in text-facing crates (CLI,
    /// launcher) and leave it off in daemon-only crates so `nota`
    /// stays out of the binary-only dependency closure.
    pub fn feature_gated_nota(feature: impl Into<String>) -> Self {
        Self {
            nota_surface: NotaSurface::FeatureGated {
                feature: feature.into(),
            },
            target: RustEmissionTarget::ComponentRuntime,
        }
    }

    /// Emit no NOTA surface at all. The generated source contains no
    /// `nota::*` references, no `FromStr` / `Display` impls
    /// (since both depend on `NotaDecode` / `NotaEncode`). The resulting
    /// Rust file compiles without `nota` in the dependency closure.
    /// This is the daemon-only / binary-only shape.
    pub fn binary_only() -> Self {
        Self {
            nota_surface: NotaSurface::Disabled,
            target: RustEmissionTarget::ComponentRuntime,
        }
    }

    pub fn with_target(mut self, target: RustEmissionTarget) -> Self {
        self.target = target;
        self
    }

    fn nota_surface(&self) -> &NotaSurface {
        &self.nota_surface
    }

    pub fn target(&self) -> RustEmissionTarget {
        self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustEmissionTarget {
    /// Schema declarations plus local inherent/codec support, with no root
    /// input/output enums and no Signal/Nexus/SEMA runtime plane.
    DeclarationModule,
    /// External signal or meta-signal wire vocabulary plus codecs only.
    WireContract,
    /// Bootstrap all-in-one runtime emission for unsplit schemas.
    ComponentRuntime,
    /// Daemon-side Signal plane runtime support over signal roots.
    SignalRuntime,
    /// Daemon-side Nexus plane runtime support only.
    NexusRuntime,
    /// Daemon-side SEMA plane runtime support only.
    SemaRuntime,
}

impl RustEmissionTarget {
    fn emits_runtime_support(self) -> bool {
        self.runtime_planes().emits_any()
    }

    /// Whether this target faces the wire and therefore needs the basic
    /// signal-frame codec (route enums, `short_header`,
    /// `encode_signal_frame` / `decode_signal_frame`, `SignalFrameError`).
    ///
    /// A separately-generated `WireContract` crate IS the wire framing —
    /// peers and the owning daemon import it and call the codec on it — so
    /// it carries the codec even though it emits no daemon-side runtime
    /// planes. `SignalRuntime` and `ComponentRuntime` (bootstrap) also face
    /// the wire. The `NexusRuntime` / `SemaRuntime` internal planes never
    /// touch the wire and must not receive frame codec code.
    fn emits_wire_frame(self) -> bool {
        match self {
            Self::WireContract | Self::SignalRuntime | Self::ComponentRuntime => true,
            Self::DeclarationModule | Self::NexusRuntime | Self::SemaRuntime => false,
        }
    }

    fn runtime_planes(self) -> RuntimePlaneSet {
        match self {
            Self::DeclarationModule => RuntimePlaneSet::none(),
            Self::WireContract => RuntimePlaneSet::none(),
            Self::ComponentRuntime => RuntimePlaneSet::all(),
            Self::SignalRuntime => RuntimePlaneSet::signal_only(),
            Self::NexusRuntime => RuntimePlaneSet::nexus_only(),
            Self::SemaRuntime => RuntimePlaneSet::sema_only(),
        }
    }

    fn emits_root_enums(self) -> bool {
        !matches!(self, Self::DeclarationModule)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimePlaneSet {
    signal: bool,
    nexus: bool,
    sema: bool,
}

impl RuntimePlaneSet {
    fn none() -> Self {
        Self {
            signal: false,
            nexus: false,
            sema: false,
        }
    }

    fn all() -> Self {
        Self {
            signal: true,
            nexus: true,
            sema: true,
        }
    }

    fn signal_only() -> Self {
        Self {
            signal: true,
            nexus: false,
            sema: false,
        }
    }

    fn nexus_only() -> Self {
        Self {
            signal: false,
            nexus: true,
            sema: false,
        }
    }

    fn sema_only() -> Self {
        Self {
            signal: false,
            nexus: false,
            sema: true,
        }
    }

    fn emits_signal(self) -> bool {
        self.signal
    }

    fn emits_nexus(self) -> bool {
        self.nexus
    }

    fn emits_sema(self) -> bool {
        self.sema
    }

    fn emits_any(self) -> bool {
        self.signal || self.nexus || self.sema
    }

    fn emits_all(self) -> bool {
        self.signal && self.nexus && self.sema
    }

    /// The planes this set emits, in canonical signal-nexus-sema order.
    fn active_planes(self) -> Vec<Plane> {
        let mut planes = Vec::new();
        if self.signal {
            planes.push(Plane::Signal);
        }
        if self.nexus {
            planes.push(Plane::Nexus);
        }
        if self.sema {
            planes.push(Plane::Sema);
        }
        planes
    }
}

/// Runtime plane axis. This owns only plane-intrinsic names; target
/// selection and schema-presence checks stay on the emitter/writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Plane {
    Signal,
    Nexus,
    Sema,
}

impl Plane {
    fn module_name(&self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Nexus => "nexus",
            Self::Sema => "sema",
        }
    }

    fn wrapper_name(&self) -> &'static str {
        match self {
            Self::Signal => "Signal",
            Self::Nexus => "Nexus",
            Self::Sema => "Sema",
        }
    }

    fn alias_names(&self) -> &'static [&'static str] {
        match self {
            Self::Signal => &["Input", "Output"],
            Self::Nexus => &["Work", "Action"],
            Self::Sema => &["WriteInput", "WriteOutput", "ReadInput", "ReadOutput"],
        }
    }

    fn canonical_source_type_names(&self) -> &'static [&'static str] {
        match self {
            Self::Signal => &["Input", "Output"],
            Self::Nexus => &["NexusWork", "NexusAction"],
            Self::Sema => &[
                "SemaWriteInput",
                "SemaWriteOutput",
                "SemaReadInput",
                "SemaReadOutput",
            ],
        }
    }

    fn engine_trait_name(&self) -> &'static str {
        match self {
            Self::Signal => "SignalEngine",
            Self::Nexus => "NexusEngine",
            Self::Sema => "SemaEngine",
        }
    }

    fn trace_enum_name(&self) -> &'static str {
        match self {
            Self::Signal => "SignalObjectName",
            Self::Nexus => "NexusObjectName",
            Self::Sema => "SemaObjectName",
        }
    }

    fn trace_activation_method_name(&self) -> &'static str {
        match self {
            Self::Signal => "trace_signal_activation",
            Self::Nexus => "trace_nexus_activation",
            Self::Sema => "trace_sema_activation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotaSurface {
    AlwaysEnabled,
    FeatureGated { feature: String },
    Disabled,
}

impl NotaSurface {
    fn emits_nota(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    fn includes_nota_in_derive(&self) -> bool {
        matches!(self, Self::AlwaysEnabled)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustScalarAlias {
    name: String,
    rust_type: String,
}

impl RustScalarAlias {
    fn new(name: impl Into<String>, rust_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rust_type: rust_type.into(),
        }
    }

    fn default_aliases() -> Vec<Self> {
        vec![
            Self::new("String", "std::string::String"),
            Self::new("Integer", "u64"),
            Self::new("Boolean", "bool"),
            Self::new("Path", "std::string::String"),
        ]
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rust_type(&self) -> &str {
        &self.rust_type
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustImport {
    use_item: String,
}

impl RustImport {
    fn from_resolved_import(import: &ResolvedImport, identity: &SchemaIdentity) -> Self {
        let current_crate = identity
            .component()
            .as_str()
            .split(':')
            .next()
            .expect("schema identity has a component name");
        if import.source().crate_name().as_str() == current_crate {
            let module_path = import
                .source()
                .module()
                .as_str()
                .replace('-', "_")
                .replace(':', "::");
            return Self {
                use_item: format!(
                    "pub use crate::schema::{}::{} as {};",
                    module_path,
                    import.source().type_name().local_part(),
                    import.local_name().local_part()
                ),
            };
        }

        Self {
            use_item: import.use_item(),
        }
    }

    pub fn use_item(&self) -> &str {
        &self.use_item
    }
}

impl LowerToRust<RustImport> for ResolvedImport {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustImport {
        RustImport {
            use_item: self.use_item(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustRelation {
    Equivalence(Vec<RustRelationValue>),
}

impl RustRelation {
    pub fn values(&self) -> &[RustRelationValue] {
        match self {
            Self::Equivalence(values) => values,
        }
    }
}

impl LowerToRust<RustRelation> for RelationDeclaration {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustRelation {
        match self {
            RelationDeclaration::Equivalence(values) => RustRelation::Equivalence(
                values
                    .iter()
                    .map(|value| value.lower_to_rust(context))
                    .collect(),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustRelationValue {
    path: Vec<Name>,
}

impl RustRelationValue {
    pub fn path(&self) -> &[Name] {
        &self.path
    }
}

impl LowerToRust<RustRelationValue> for RelationValue {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustRelationValue {
        RustRelationValue {
            path: self.path().to_vec(),
        }
    }
}

/// The component's versioned store as seen by Rust emission: the store
/// name derived from the schema identity's component name, plus one
/// [`RustRecordFamily`] per declared record family. Empty when the
/// schema declares no families, in which case no families surface is
/// emitted at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustVersionedStore {
    store_name: String,
    families: Vec<RustRecordFamily>,
}

impl RustVersionedStore {
    pub fn store_name(&self) -> &str {
        &self.store_name
    }

    pub fn families(&self) -> &[RustRecordFamily] {
        &self.families
    }

    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }
}

impl LowerToRust<RustVersionedStore> for Schema {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustVersionedStore {
        SpecifiedSchema::from(self).lower_to_rust(_context)
    }
}

impl LowerToRust<RustVersionedStore> for SpecifiedSchema {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustVersionedStore {
        RustVersionedStore {
            store_name: self.identity().component().as_str().to_owned(),
            families: self
                .families()
                .iter()
                .map(|declaration| RustRecordFamily {
                    declaration: declaration.clone(),
                    schema_hash: *self
                        .family_closure(declaration.record.as_str())
                        .expect("family record closure builds for a verified schema")
                        .content_hash()
                        .expect("family closure archives for content hashing")
                        .as_bytes(),
                })
                .collect(),
        }
    }
}

/// One declared record family plus its generation-time content
/// identity: the blake3 hash of the family record's schema closure,
/// computed while the semantic schema is in hand. The emitted artifact
/// pins this hash as a constant, so any schema edit that moves the
/// closure shows up as a generated-code change under the build driver's
/// freshness check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustRecordFamily {
    declaration: FamilyDeclaration,
    schema_hash: [u8; 32],
}

impl RustRecordFamily {
    pub fn name(&self) -> &Name {
        &self.declaration.name
    }

    pub fn record(&self) -> &Name {
        &self.declaration.record
    }

    pub fn table(&self) -> &str {
        self.declaration.table.as_str()
    }

    pub fn key(&self) -> FamilyKey {
        self.declaration.key
    }

    pub fn schema_hash(&self) -> &[u8; 32] {
        &self.schema_hash
    }

    fn constant_identifier(&self) -> Ident {
        Ident::new(
            &ScreamingName::new(self.name()).screaming(),
            Span::call_site(),
        )
    }

    fn constructor_identifier(&self) -> Ident {
        let constructor_name = self.name().field_name();
        if RustKeyword::new(&constructor_name).is_reserved() {
            Ident::new_raw(&constructor_name, Span::call_site())
        } else {
            Ident::new(&constructor_name, Span::call_site())
        }
    }

    fn descriptor_type(&self) -> TokenStream {
        let record = RustIdentifier::new(self.record().as_str());
        match self.key() {
            FamilyKey::Domain => quote! { sema_engine::TableDescriptor<#record> },
            FamilyKey::Identified => quote! { sema_engine::IdentifiedTableDescriptor<#record> },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDeclaration {
    visibility: Visibility,
    name: Name,
    parameters: Vec<Name>,
    value: RustTypeDeclaration,
}

impl RustDeclaration {
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The declaration's generic type parameters, in declared order. Empty
    /// for an ordinary (non-parameterized) declaration; non-empty only for
    /// a parameterized declaration such as the shared reaction frame's
    /// `Work<Event, Write, Read, Effect>`.
    pub fn parameters(&self) -> &[Name] {
        &self.parameters
    }

    pub fn value(&self) -> &RustTypeDeclaration {
        &self.value
    }
}

impl LowerToRust<RustDeclaration> for Declaration {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustDeclaration {
        let parameters = self.parameters().to_vec();
        let mut value = self.value().lower_to_rust(context);
        if let RustTypeDeclaration::Enum(enumeration) = &mut value {
            enumeration.set_parameters(parameters.clone());
        }
        RustDeclaration {
            visibility: self.visibility(),
            name: self.name().clone(),
            parameters,
            value,
        }
    }
}

impl LowerToRust<RustDeclaration> for SpecifiedDeclaration {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustDeclaration {
        let parameters = self.parameters().to_vec();
        let mut value = match self.body() {
            SpecifiedDeclarationBody::Struct(fields) => RustTypeDeclaration::Struct(RustStruct {
                name: self.name().clone(),
                fields: fields
                    .iter()
                    .map(|field| field.lower_to_rust(context))
                    .collect(),
            }),
            SpecifiedDeclarationBody::Enum(variants) => RustTypeDeclaration::Enum(RustEnum {
                name: self.name().clone(),
                parameters: Vec::new(),
                variants: variants
                    .iter()
                    .map(|variant| variant.lower_to_rust(context))
                    .collect(),
            }),
            SpecifiedDeclarationBody::Newtype(reference) => {
                RustTypeDeclaration::Newtype(RustNewtype {
                    name: self.name().clone(),
                    reference: reference.clone(),
                })
            }
        };
        if let RustTypeDeclaration::Enum(enumeration) = &mut value {
            enumeration.set_parameters(parameters.clone());
        }
        RustDeclaration {
            visibility: self.visibility(),
            name: self.name().clone(),
            parameters,
            value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustTypeDeclaration {
    Struct(RustStruct),
    Enum(RustEnum),
    Newtype(RustNewtype),
}

impl LowerToRust<RustTypeDeclaration> for TypeDeclaration {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustTypeDeclaration {
        match self {
            TypeDeclaration::Struct(declaration) => {
                RustTypeDeclaration::Struct(declaration.lower_to_rust(context))
            }
            TypeDeclaration::Enum(declaration) => {
                RustTypeDeclaration::Enum(declaration.lower_to_rust(context))
            }
            TypeDeclaration::Newtype(declaration) => {
                RustTypeDeclaration::Newtype(declaration.lower_to_rust(context))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustNewtype {
    name: Name,
    reference: TypeReference,
}

impl RustNewtype {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn reference(&self) -> &TypeReference {
        &self.reference
    }

    fn scope_root_name(&self) -> Option<&Name> {
        let TypeReference::ScopeOf(reference) = &self.reference else {
            return None;
        };
        reference.plain_name()
    }

    fn is_scope_of(&self) -> bool {
        self.scope_root_name().is_some()
    }

    fn takes_string_constructor(&self) -> bool {
        matches!(self.reference, TypeReference::String | TypeReference::Path)
    }
}

impl LowerToRust<RustNewtype> for NewtypeDeclaration {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustNewtype {
        RustNewtype {
            name: self.name.clone(),
            reference: self.reference.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustStruct {
    name: Name,
    fields: Vec<RustField>,
}

impl RustStruct {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn fields(&self) -> &[RustField] {
        &self.fields
    }
}

impl LowerToRust<RustStruct> for StructDeclaration {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustStruct {
        RustStruct {
            name: self.name.clone(),
            fields: self
                .fields
                .iter()
                .map(|field| field.lower_to_rust(context))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustField {
    name: Name,
    reference: TypeReference,
}

impl RustField {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn reference(&self) -> &TypeReference {
        &self.reference
    }
}

impl LowerToRust<RustField> for FieldDeclaration {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustField {
        RustField {
            name: self.name.clone(),
            reference: self.reference.clone(),
        }
    }
}

impl LowerToRust<RustField> for SpecifiedField {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustField {
        RustField {
            name: self.name().clone(),
            reference: self.reference().clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustEnum {
    name: Name,
    parameters: Vec<Name>,
    variants: Vec<RustEnumVariant>,
}

impl RustEnum {
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The enum's generic type parameters, in declared order. Empty for an
    /// ordinary enum; non-empty for a parameterized frame enum such as
    /// `Work<Event, Write, Read, Effect>`.
    pub fn parameters(&self) -> &[Name] {
        &self.parameters
    }

    /// Attach the owning declaration's generic parameters to this enum.
    /// A bare `EnumDeclaration` carries no parameters — they live on the
    /// outer `Declaration` head — so the declaration lowering injects them.
    fn set_parameters(&mut self, parameters: Vec<Name>) {
        self.parameters = parameters;
    }

    pub fn variants(&self) -> &[RustEnumVariant] {
        &self.variants
    }

    pub fn has_only_unit_variants(&self) -> bool {
        self.variants.iter().all(RustEnumVariant::has_no_payload)
    }
}

impl LowerToRust<RustEnum> for EnumDeclaration {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustEnum {
        RustEnum {
            name: self.name.clone(),
            parameters: Vec::new(),
            variants: self
                .variants
                .iter()
                .map(|variant| variant.lower_to_rust(context))
                .collect(),
        }
    }
}

impl LowerToRust<RustEnum> for SpecifiedRootEnum {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustEnum {
        RustEnum {
            name: self.name().clone(),
            parameters: Vec::new(),
            variants: self
                .variants()
                .iter()
                .map(|variant| variant.lower_to_rust(context))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustEnumVariant {
    name: Name,
    payload: Option<TypeReference>,
}

impl RustEnumVariant {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn payload(&self) -> Option<&TypeReference> {
        self.payload.as_ref()
    }

    pub fn has_no_payload(&self) -> bool {
        self.payload.is_none()
    }
}

impl LowerToRust<RustEnumVariant> for EnumVariant {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustEnumVariant {
        RustEnumVariant {
            name: self.name.clone(),
            payload: self.payload.clone(),
        }
    }
}

impl LowerToRust<RustEnumVariant> for SpecifiedVariant {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustEnumVariant {
        RustEnumVariant {
            name: self.name().clone(),
            payload: self.payload().map(|payload| payload.reference().clone()),
        }
    }
}

/// A component root that applies an imported parameterized frame head at its
/// Input/Output position — `(Work SignalInput SemaWriteOutput …)` — rather
/// than spelling out an enum body. It lowers to a concrete Rust type alias
/// `pub type <position> = <Head><Args>;` so the component refers to the
/// fully-applied frame type by the position name while the imported head
/// owns the generic definition. The applied reference is the field-position
/// projection of the root, so it renders through the same
/// `RustTypeReferenceTokens` path as any other applied type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustAppliedRoot {
    name: Name,
    applied: TypeReference,
}

impl RustAppliedRoot {
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The fully-applied frame type this root aliases to, as an
    /// `Application` reference carrying the head and its arguments.
    pub fn applied(&self) -> &TypeReference {
        &self.applied
    }
}

impl LowerToRust<RustAppliedRoot> for RootApplication {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustAppliedRoot {
        RustAppliedRoot {
            name: self.name().clone(),
            applied: TypeReference::from(self),
        }
    }
}

impl LowerToRust<RustAppliedRoot> for SpecifiedRootApplication {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustAppliedRoot {
        RustAppliedRoot {
            name: self.name().clone(),
            applied: self.reference().clone(),
        }
    }
}

/// An owned mirror of one [`schema::ReferencedImpl`] — an entry from the
/// schema-wide `{| … |}` impl catalog, paired with the type it targets. The
/// borrowed `ReferencedImpl<'schema>` cannot cross into the owned
/// [`RustModule`], so lowering clones it into this noun. The catalog carries
/// no Rust body: this is the *selection* data that drives which standard impls
/// the module emits and the *manifest* the emitted surface is verified against.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustImplReference {
    target: Name,
    entry: RustImplEntry,
}

impl RustImplReference {
    pub fn target(&self) -> &Name {
        &self.target
    }

    pub fn entry(&self) -> &RustImplEntry {
        &self.entry
    }
}

impl LowerToRust<RustImplReference> for ReferencedImpl<'_> {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustImplReference {
        RustImplReference {
            target: self.target().clone(),
            entry: self.entry().lower_to_rust(context),
        }
    }
}

struct SpecifiedReferencedImpls<'schema> {
    schema: &'schema SpecifiedSchema,
}

impl<'schema> SpecifiedReferencedImpls<'schema> {
    fn new(schema: &'schema SpecifiedSchema) -> Self {
        Self { schema }
    }

    fn lower_to_rust(&self, context: &RustLoweringContext) -> Vec<RustImplReference> {
        let mut references = Vec::new();
        for declaration in self.schema.declarations() {
            for entry in declaration.impls().entries() {
                references.push(RustImplReference {
                    target: declaration.name().clone(),
                    entry: entry.lower_to_rust(context),
                });
            }
        }
        for block in self.schema.impl_blocks() {
            for entry in block.catalog().entries() {
                references.push(RustImplReference {
                    target: block.target().clone(),
                    entry: entry.lower_to_rust(context),
                });
            }
        }
        references
    }
}

/// The owned lowering of one [`schema::ImplReference`]: a bare trait
/// marker, a body-bearing trait impl with its required method signatures, or a
/// single inherent method signature. Mirrors the schema enum shape so the
/// module owns its catalog without borrowing the source schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustImplEntry {
    Marker(Name),
    TraitImpl(Name, Vec<RustMethodSignature>),
    InherentMethod(RustMethodSignature),
}

impl RustImplEntry {
    /// The trait this entry names, if any. Inherent methods name no trait.
    pub fn trait_name(&self) -> Option<&Name> {
        match self {
            Self::Marker(trait_name) | Self::TraitImpl(trait_name, _) => Some(trait_name),
            Self::InherentMethod(_) => None,
        }
    }

    /// The method signatures this entry references — none for a marker, the
    /// required methods for a trait impl, exactly itself for an inherent
    /// method. Mirrors [`schema::ImplReference::methods`].
    pub fn methods(&self) -> &[RustMethodSignature] {
        match self {
            Self::Marker(_) => &[],
            Self::TraitImpl(_, methods) => methods,
            Self::InherentMethod(signature) => std::slice::from_ref(signature),
        }
    }
}

impl LowerToRust<RustImplEntry> for ImplReference {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustImplEntry {
        match self {
            ImplReference::Marker(trait_name) => RustImplEntry::Marker(trait_name.clone()),
            ImplReference::TraitImpl(trait_name, methods) => RustImplEntry::TraitImpl(
                trait_name.clone(),
                methods
                    .iter()
                    .map(|signature| signature.lower_to_rust(context))
                    .collect(),
            ),
            ImplReference::InherentMethod(signature) => {
                RustImplEntry::InherentMethod(signature.lower_to_rust(context))
            }
        }
    }
}

/// The owned lowering of one [`schema::MethodSignature`]: a method name,
/// its parameters, and its return type reference. It re-derives the canonical
/// rendering schema uses for duplicate detection and unverified-reference
/// errors so an emitted [`ImplFact`] and the source catalog entry match
/// signature-for-signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMethodSignature {
    name: Name,
    parameters: Vec<RustMethodParameter>,
    return_reference: TypeReference,
}

impl RustMethodSignature {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn parameters(&self) -> &[RustMethodParameter] {
        &self.parameters
    }

    pub fn return_reference(&self) -> &TypeReference {
        &self.return_reference
    }
}

impl LowerToRust<RustMethodSignature> for MethodSignature {
    fn lower_to_rust(&self, context: &RustLoweringContext) -> RustMethodSignature {
        RustMethodSignature {
            name: self.name().clone(),
            parameters: self
                .parameters()
                .iter()
                .map(|parameter| parameter.lower_to_rust(context))
                .collect(),
            return_reference: self.return_reference().clone(),
        }
    }
}

/// The schema [`MethodSignature`] this owned signature corresponds to —
/// the bridge back into a [`schema::ImplFact`] for the emitted surface.
/// Reconstructs the source-side parameter and return types so the surface fact
/// renders the exact canonical signature `RustSurface::verify_catalog` matches.
impl From<&RustMethodSignature> for MethodSignature {
    fn from(signature: &RustMethodSignature) -> Self {
        MethodSignature::new(
            signature.name.clone(),
            signature
                .parameters
                .iter()
                .map(MethodParameter::from)
                .collect(),
            signature.return_reference.clone(),
        )
    }
}

/// The owned lowering of one [`schema::MethodParameter`]: a parameter name
/// and its resolved type reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMethodParameter {
    name: Name,
    reference: TypeReference,
}

impl RustMethodParameter {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn reference(&self) -> &TypeReference {
        &self.reference
    }
}

impl LowerToRust<RustMethodParameter> for MethodParameter {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustMethodParameter {
        RustMethodParameter {
            name: self.name().clone(),
            reference: self.reference().clone(),
        }
    }
}

impl From<&RustMethodParameter> for MethodParameter {
    fn from(parameter: &RustMethodParameter) -> Self {
        MethodParameter::new(parameter.name.clone(), parameter.reference.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustSupportModel {
    map_key_type_names: Vec<String>,
    private_type_names: Vec<String>,
    references_bytes: bool,
    references_fixed_bytes: bool,
}

impl RustSupportModel {
    fn map_key_type_names(&self) -> &[String] {
        &self.map_key_type_names
    }

    fn private_type_names(&self) -> &[String] {
        &self.private_type_names
    }

    fn references_bytes(&self) -> bool {
        self.references_bytes
    }

    fn references_fixed_bytes(&self) -> bool {
        self.references_fixed_bytes
    }
}

impl LowerToRust<RustSupportModel> for Schema {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustSupportModel {
        SpecifiedSchema::from(self).lower_to_rust(_context)
    }
}

impl LowerToRust<RustSupportModel> for SpecifiedSchema {
    fn lower_to_rust(&self, _context: &RustLoweringContext) -> RustSupportModel {
        RustSupportModel {
            map_key_type_names: CollectionScan::new(self).map_key_type_names(),
            references_bytes: CollectionScan::new(self).references_bytes(),
            references_fixed_bytes: CollectionScan::new(self).references_fixed_bytes(),
            private_type_names: self
                .declarations()
                .iter()
                .filter(|declaration| declaration.visibility() == Visibility::Private)
                .map(|declaration| declaration.name().as_str().to_owned())
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct RustRenderContext {
    map_key_type_names: Vec<String>,
    ordering_type_names: Vec<String>,
    private_type_names: Vec<String>,
    nota_surface: NotaSurface,
}

impl RustRenderContext {
    fn new(
        map_key_type_names: Vec<String>,
        ordering_type_names: Vec<String>,
        private_type_names: Vec<String>,
        nota_surface: NotaSurface,
    ) -> Self {
        Self {
            map_key_type_names,
            ordering_type_names,
            private_type_names,
            nota_surface,
        }
    }

    /// Whether the data type gets the `PartialOrd, Ord` derive class — true for
    /// a `BTreeMap` key type (so the map compiles) or a type the `{| … |}`
    /// catalog references `Ord`/`PartialOrd` for (the derive *is* the body for
    /// an ordering-class marker, so the catalog entry is satisfied by it).
    fn is_ordering_class(&self, type_name: &Name) -> bool {
        self.map_key_type_names
            .iter()
            .chain(self.ordering_type_names.iter())
            .any(|name| name == type_name.as_str())
    }

    fn data_type_attributes(&self, type_name: &Name) -> Vec<TokenStream> {
        self.derive_attributes(false, self.is_ordering_class(type_name))
    }

    fn enum_type_attributes(&self, enumeration: &RustEnum) -> Vec<TokenStream> {
        self.derive_attributes(
            enumeration.has_only_unit_variants(),
            self.map_key_type_names
                .iter()
                .any(|name| name == enumeration.name().as_str()),
        )
    }

    fn root_enum_type_attributes(&self, enumeration: &RustEnum) -> Vec<TokenStream> {
        self.derive_attributes(enumeration.has_only_unit_variants(), false)
    }

    fn scope_enum_type_attributes(&self) -> Vec<TokenStream> {
        self.derive_attributes(false, false)
    }

    fn derive_attributes(&self, includes_copy: bool, includes_ordering: bool) -> Vec<TokenStream> {
        let mut attributes = Vec::new();
        if let NotaSurface::FeatureGated { feature } = &self.nota_surface {
            attributes.push(quote! {
                #[cfg_attr(feature = #feature, derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode))]
            });
        }
        let nota_derives = if self.nota_surface.includes_nota_in_derive() {
            quote! { nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode, }
        } else {
            TokenStream::new()
        };
        let copy_derive = if includes_copy {
            quote! { Copy, }
        } else {
            TokenStream::new()
        };
        let ordering_derives = if includes_ordering {
            quote! { PartialOrd, Ord, }
        } else {
            TokenStream::new()
        };
        attributes.push(quote! {
            #[derive(
                #nota_derives
                rkyv::Archive,
                rkyv::Serialize,
                rkyv::Deserialize,
                Clone,
                #copy_derive
                Debug,
                PartialEq,
                Eq,
                #ordering_derives
            )]
        });
        if includes_ordering {
            attributes.push(quote! {
                #[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
            });
        }
        attributes
    }

    fn visibility_tokens(&self, visibility: Visibility) -> TokenStream {
        match visibility {
            Visibility::Public => quote! { pub },
            Visibility::Private => quote! { pub(crate) },
        }
    }

    /// The `#[cfg(feature = "<feature>")]` gate applied to NOTA-only
    /// items, as a token attribute. `None` when the surface is always
    /// enabled or disabled (no gate line).
    fn nota_feature_gate(&self) -> Option<TokenStream> {
        match &self.nota_surface {
            NotaSurface::AlwaysEnabled | NotaSurface::Disabled => None,
            NotaSurface::FeatureGated { feature } => Some(quote! {
                #[cfg(feature = #feature)]
            }),
        }
    }

    fn field_visibility_tokens(
        &self,
        visibility: Visibility,
        reference: &TypeReference,
    ) -> TokenStream {
        if visibility == Visibility::Public && self.references_private_type(reference) {
            quote! { pub(crate) }
        } else {
            self.visibility_tokens(visibility)
        }
    }

    fn references_private_type(&self, reference: &TypeReference) -> bool {
        match reference {
            TypeReference::Plain(name) => self
                .private_type_names
                .iter()
                .any(|private_name| private_name == name.as_str()),
            TypeReference::Vector(inner)
            | TypeReference::Optional(inner)
            | TypeReference::ScopeOf(inner) => self.references_private_type(inner),
            TypeReference::Map(key, value) => {
                self.references_private_type(key) || self.references_private_type(value)
            }
            TypeReference::Application { arguments, .. } => arguments
                .iter()
                .any(|argument| self.references_private_type(argument)),
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::Bytes
            | TypeReference::FixedBytes(_) => false,
        }
    }
}

#[derive(Clone, Copy)]
struct RustIdentifier<'name> {
    name: &'name str,
}

impl<'name> RustIdentifier<'name> {
    fn new(name: &'name str) -> Self {
        Self { name }
    }

    fn ident(&self) -> Ident {
        if RustKeyword::new(self.name).is_reserved() {
            Ident::new_raw(self.name, Span::call_site())
        } else {
            Ident::new(self.name, Span::call_site())
        }
    }

    /// Whether this name is a legal Rust identifier (raw or plain) — the
    /// non-panicking pre-check `ident()` relies on. `Ident::new` PANICS on a
    /// malformed name; this answers the same question without aborting, so a bad
    /// schema name becomes a typed error at the emission boundary instead.
    fn is_legal(&self) -> bool {
        RustKeyword::new(self.name).is_reserved() || syn::parse_str::<syn::Ident>(self.name).is_ok()
    }

    /// Validate a schema-derived name in a type/variant/parameter position as a
    /// legal Rust identifier, yielding a typed [`SchemaError`] (not a panic)
    /// when it is not. NOTA accepts symbol atoms (`Foo-Bar`, `A/B`, `2Things`)
    /// far broader than Rust identifiers, so this gate is what stops a malformed
    /// schema name from reaching `Ident::new` and aborting the generator.
    fn verify(name: &Name, position: &str) -> Result<(), SchemaError> {
        if (RustIdentifier {
            name: name.as_str(),
        })
        .is_legal()
        {
            return Ok(());
        }
        Err(SchemaError::MalformedSchemaNode {
            found: format!(
                "{position} name `{}` is not a legal Rust identifier",
                name.as_str()
            ),
        })
    }

    /// Validate a struct field name. Fields render through the same
    /// `RustIdentifier` path as types, so the same identifier gate applies.
    fn verify_field(name: &Name) -> Result<(), SchemaError> {
        Self::verify(name, "field")
    }
}

impl ToTokens for RustIdentifier<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident().to_tokens(tokens);
    }
}

struct RustTypeReferenceTokens<'reference> {
    reference: &'reference TypeReference,
}

impl<'reference> RustTypeReferenceTokens<'reference> {
    fn new(reference: &'reference TypeReference) -> Self {
        Self { reference }
    }
}

impl ToTokens for RustTypeReferenceTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.reference {
            TypeReference::String => quote! { String }.to_tokens(tokens),
            TypeReference::Integer => quote! { Integer }.to_tokens(tokens),
            TypeReference::Boolean => quote! { Boolean }.to_tokens(tokens),
            TypeReference::Path => quote! { Path }.to_tokens(tokens),
            TypeReference::Bytes => quote! { Bytes }.to_tokens(tokens),
            TypeReference::FixedBytes(width) => {
                let width = proc_macro2::Literal::u64_unsuffixed(*width);
                quote! { FixedBytes<#width> }.to_tokens(tokens);
            }
            TypeReference::Plain(name) => RustIdentifier::new(name.as_str()).to_tokens(tokens),
            TypeReference::Vector(inner) => {
                let inner = Self::new(inner);
                quote! { Vec<#inner> }.to_tokens(tokens);
            }
            TypeReference::Map(key, value) => {
                let key = Self::new(key);
                let value = Self::new(value);
                quote! { std::collections::BTreeMap<#key, #value> }.to_tokens(tokens);
            }
            TypeReference::Optional(inner) => {
                let inner = Self::new(inner);
                quote! { Option<#inner> }.to_tokens(tokens);
            }
            TypeReference::ScopeOf(inner) => match inner.plain_name() {
                Some(name) => {
                    let scope_name = format!("{}Scope", name);
                    let name = RustIdentifier::new(&scope_name);
                    name.to_tokens(tokens);
                }
                None => {
                    let inner = Self::new(inner);
                    quote! { #inner }.to_tokens(tokens);
                }
            },
            TypeReference::Application { head, arguments } => {
                let head = RustIdentifier::new(head.name().as_str());
                let arguments = arguments.iter().map(Self::new);
                quote! { #head<#(#arguments),*> }.to_tokens(tokens);
            }
        }
    }
}

struct RustTypeTokens<'source> {
    source: &'source str,
}

impl<'source> RustTypeTokens<'source> {
    fn new(source: &'source str) -> Self {
        Self { source }
    }
}

impl ToTokens for RustTypeTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        syn::parse_str::<syn::Type>(self.source)
            .expect("generated Rust type token parses")
            .to_tokens(tokens);
    }
}

/// Renders a scalar alias declaration (`pub type Name = path::To::Type;`)
/// as Rust tokens. The alias name is an identifier; the target is an
/// already-resolved Rust type path parsed into a type token.
struct RustScalarAliasTokens<'alias> {
    alias: &'alias RustScalarAlias,
}

impl<'alias> RustScalarAliasTokens<'alias> {
    fn new(alias: &'alias RustScalarAlias) -> Self {
        Self { alias }
    }
}

impl ToTokens for RustScalarAliasTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.alias.name());
        let rust_type = RustTypeTokens::new(self.alias.rust_type());
        quote! {
            pub type #name = #rust_type;
        }
        .to_tokens(tokens);
    }
}

/// The `<Name>Route` enum derived from a source enum: one unit variant per
/// source variant, carrying the copy-data derive set. Owns the source enum
/// and the render context that supplies the derives.
struct RouteEnumTokens<'enum_source, 'context> {
    source: &'enum_source RustEnum,
    context: &'context RustRenderContext,
}

impl<'enum_source, 'context> RouteEnumTokens<'enum_source, 'context> {
    fn new(source: &'enum_source RustEnum, context: &'context RustRenderContext) -> Self {
        Self { source, context }
    }
}

impl ToTokens for RouteEnumTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.context.derive_attributes(true, false);
        let route_name = RouteName::new(self.source.name()).ident();
        let variants = self
            .source
            .variants()
            .iter()
            .map(|variant| RustIdentifier::new(variant.name().as_str()).ident());
        quote! {
            #(#attributes)*
            pub enum #route_name {
                #(#variants,)*
            }
        }
        .to_tokens(tokens);
    }
}

/// The `route(&self) -> <Name>Route` projection on a source enum: a `match`
/// that maps each source variant to its route variant, ignoring any payload.
struct RouteImplTokens<'enum_source> {
    source: &'enum_source RustEnum,
}

impl<'enum_source> RouteImplTokens<'enum_source> {
    fn new(source: &'enum_source RustEnum) -> Self {
        Self { source }
    }
}

impl ToTokens for RouteImplTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.source.name().as_str()).ident();
        let route_name = RouteName::new(self.source.name()).ident();
        let arms = self
            .source
            .variants()
            .iter()
            .map(|variant| RouteMatchArm::new(variant, &route_name));
        quote! {
            impl #name {
                pub fn route(&self) -> #route_name {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// One `Self::Variant(_) => <Route>::Variant,` arm of a `route` match. Owns
/// the source variant and the route enum name; the payload presence decides
/// whether the pattern binds `(_)`.
struct RouteMatchArm<'variant, 'route> {
    variant: &'variant RustEnumVariant,
    route_name: &'route Ident,
}

impl<'variant, 'route> RouteMatchArm<'variant, 'route> {
    fn new(variant: &'variant RustEnumVariant, route_name: &'route Ident) -> Self {
        Self {
            variant,
            route_name,
        }
    }
}

impl ToTokens for RouteMatchArm<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variant = RustIdentifier::new(self.variant.name().as_str()).ident();
        let route_name = self.route_name;
        if self.variant.payload().is_some() {
            quote! { Self::#variant(_) => #route_name::#variant, }.to_tokens(tokens);
        } else {
            quote! { Self::#variant => #route_name::#variant, }.to_tokens(tokens);
        }
    }
}

/// The full signal-frame binary codec impl for one root enum: `route`,
/// `short_header`, `route_from_short_header`, `encode_signal_frame`, and
/// `decode_signal_frame`. Owns the root enum and renders the per-variant
/// triage arms plus the fixed encode/decode bodies. The route and
/// short-header arms are derived from the same `ShortHeader` noun the
/// `short_header` module uses, so the constant names always agree.
struct SignalFrameImplTokens<'enum_source> {
    root_enum: &'enum_source RustEnum,
}

impl<'enum_source> SignalFrameImplTokens<'enum_source> {
    fn new(root_enum: &'enum_source RustEnum) -> Self {
        Self { root_enum }
    }
}

impl ToTokens for SignalFrameImplTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.root_enum.name().as_str()).ident();
        let route_name = RouteName::new(self.root_enum.name()).ident();
        let root_name_literal = self.root_enum.name().as_str();

        let route_arms = self
            .root_enum
            .variants()
            .iter()
            .map(|variant| RouteMatchArm::new(variant, &route_name));

        let short_header_arms =
            self.root_enum
                .variants()
                .iter()
                .enumerate()
                .map(|(variant_index, variant)| {
                    let constant =
                        ShortHeader::new(self.root_enum.name(), variant.name(), 0, variant_index)
                            .constant_identifier();
                    let variant_ident = RustIdentifier::new(variant.name().as_str()).ident();
                    if variant.payload().is_some() {
                        quote! { Self::#variant_ident(_) => short_header::#constant, }
                    } else {
                        quote! { Self::#variant_ident => short_header::#constant, }
                    }
                });

        let route_from_header_arms =
            self.root_enum
                .variants()
                .iter()
                .enumerate()
                .map(|(variant_index, variant)| {
                    let constant =
                        ShortHeader::new(self.root_enum.name(), variant.name(), 0, variant_index)
                            .constant_identifier();
                    let variant_ident = RustIdentifier::new(variant.name().as_str()).ident();
                    quote! { short_header::#constant => Ok(#route_name::#variant_ident), }
                });

        quote! {
            impl #name {
                pub fn route(&self) -> #route_name {
                    match self {
                        #(#route_arms)*
                    }
                }
                pub fn short_header(&self) -> u64 {
                    match self {
                        #(#short_header_arms)*
                    }
                }
                pub fn route_from_short_header(header: u64) -> Result<#route_name, SignalFrameError> {
                    match header {
                        #(#route_from_header_arms)*
                        _ => Err(SignalFrameError::UnknownHeader { root_enum: #root_name_literal, header }),
                    }
                }
                pub fn encode_signal_frame(&self) -> Result<Vec<u8>, SignalFrameError> {
                    let archive = rkyv::to_bytes::<rkyv::rancor::Error>(self)
                        .map_err(|_| SignalFrameError::ArchiveEncode)?;
                    let mut frame = Vec::with_capacity(SIGNAL_SHORT_HEADER_BYTE_COUNT + archive.len());
                    frame.extend_from_slice(&self.short_header().to_le_bytes());
                    frame.extend_from_slice(&archive);
                    Ok(frame)
                }
                pub fn decode_signal_frame(frame: &[u8]) -> Result<(#route_name, Self), SignalFrameError> {
                    if frame.len() < SIGNAL_SHORT_HEADER_BYTE_COUNT {
                        return Err(SignalFrameError::FrameTooShort { found: frame.len() });
                    }
                    let mut header_bytes = [0_u8; SIGNAL_SHORT_HEADER_BYTE_COUNT];
                    header_bytes.copy_from_slice(&frame[..SIGNAL_SHORT_HEADER_BYTE_COUNT]);
                    let header = u64::from_le_bytes(header_bytes);
                    let route = Self::route_from_short_header(header)?;
                    let value = rkyv::from_bytes::<Self, rkyv::rancor::Error>(&frame[SIGNAL_SHORT_HEADER_BYTE_COUNT..])
                        .map_err(|_| SignalFrameError::ArchiveDecode)?;
                    let expected = value.short_header();
                    if expected != header {
                        return Err(SignalFrameError::HeaderMismatch { expected, found: header });
                    }
                    Ok((route, value))
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// The `<Name>Route` type identifier derived from a schema enum name.
struct RouteName<'name> {
    name: &'name Name,
}

impl<'name> RouteName<'name> {
    fn new(name: &'name Name) -> Self {
        Self { name }
    }

    fn ident(&self) -> Ident {
        Ident::new(&format!("{}Route", self.name.as_str()), Span::call_site())
    }
}

/// A single short-header constant: the per-root-per-variant route triage
/// key. It owns the root name, the variant name, and the two positional
/// indices, and renders its own SCREAMING_SNAKE constant identifier plus
/// the packed `u64` value. Shared by the `short_header` module emission and
/// the per-root frame codec so the constant name and value are computed in
/// exactly one place.
struct ShortHeader<'enums> {
    root_name: &'enums Name,
    variant_name: &'enums Name,
    root_index: usize,
    variant_index: usize,
}

impl<'enums> ShortHeader<'enums> {
    fn new(
        root_name: &'enums Name,
        variant_name: &'enums Name,
        root_index: usize,
        variant_index: usize,
    ) -> Self {
        Self {
            root_name,
            variant_name,
            root_index,
            variant_index,
        }
    }

    fn constant_identifier(&self) -> Ident {
        let name = format!(
            "{}_{}",
            ScreamingName::new(self.root_name).screaming(),
            ScreamingName::new(self.variant_name).screaming()
        );
        Ident::new(&name, Span::call_site())
    }

    fn value(&self) -> u64 {
        ((self.root_index as u64) << 56) | ((self.variant_index as u64) << 48)
    }

    fn value_literal(&self) -> syn::LitInt {
        syn::LitInt::new(&format!("0x{:016X}", self.value()), Span::call_site())
    }
}

/// Renders the `pub mod short_header { ... }` module: one `pub const NAME:
/// u64 = 0x...;` per root-enum variant across every root, with values packed
/// by declaration order.
struct ShortHeaderModuleTokens<'enums> {
    root_enums: &'enums [RustEnum],
}

impl<'enums> ShortHeaderModuleTokens<'enums> {
    fn new(root_enums: &'enums [RustEnum]) -> Self {
        Self { root_enums }
    }
}

impl ToTokens for ShortHeaderModuleTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let constants = self
            .root_enums
            .iter()
            .enumerate()
            .flat_map(|(root_index, root_enum)| {
                root_enum
                    .variants()
                    .iter()
                    .enumerate()
                    .map(move |(variant_index, variant)| {
                        let header = ShortHeader::new(
                            root_enum.name(),
                            variant.name(),
                            root_index,
                            variant_index,
                        );
                        let constant = header.constant_identifier();
                        let value = header.value_literal();
                        quote! { pub const #constant: u64 = #value; }
                    })
            });
        quote! {
            pub mod short_header {
                #(#constants)*
            }
        }
        .to_tokens(tokens);
    }
}

/// Projects a schema `Name` into its SCREAMING_SNAKE_CASE constant form:
/// PascalCase word boundaries become underscores and lowercase runs are
/// uppercased. Owns the borrowed name and renders the screaming string.
struct ScreamingName<'name> {
    name: &'name Name,
}

impl<'name> ScreamingName<'name> {
    fn new(name: &'name Name) -> Self {
        Self { name }
    }

    fn screaming(&self) -> String {
        let mut output = String::new();
        for (index, character) in self.name.as_str().chars().enumerate() {
            if character.is_ascii_uppercase() {
                if index > 0 {
                    output.push('_');
                }
                output.push(character);
            } else if character == '-' {
                output.push('_');
            } else {
                output.push(character.to_ascii_uppercase());
            }
        }
        output
    }
}

/// How the generated `to_nota` bridge takes its receiver. A `Copy` noun
/// takes `self` by value and borrows for the encode call; every other noun
/// borrows `&self`.
#[derive(Clone, Copy)]
/// Renders the root-enum `FromStr` + `Display` NOTA surface, each gated by
/// the context's NOTA feature gate.
struct NotaRootEnumStringSupportTokens<'name, 'context> {
    name: &'name str,
    context: &'context RustRenderContext,
}

impl<'name, 'context> NotaRootEnumStringSupportTokens<'name, 'context> {
    fn new(name: &'name str, context: &'context RustRenderContext) -> Self {
        Self { name, context }
    }
}

impl ToTokens for NotaRootEnumStringSupportTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let gate = self.context.nota_feature_gate();
        let name = RustIdentifier::new(self.name);
        quote! {
            #gate
            impl std::str::FromStr for #name {
                type Err = NotaDecodeError;
                fn from_str(source: &str) -> Result<Self, Self::Err> {
                    NotaSource::new(source).parse::<Self>()
                }
            }
            #gate
            impl std::fmt::Display for #name {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str(&<Self as NotaEncode>::to_nota(self))
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// Renders the inherent `new` / `payload` / `into_payload` accessors plus
/// the `From<Payload>` impl for a tuple newtype. The newtype owns its name
/// and the wrapped payload type, so it renders its own construction surface.
struct NewtypeInherentImplTokens<'newtype> {
    newtype: &'newtype RustNewtype,
}

impl<'newtype> NewtypeInherentImplTokens<'newtype> {
    fn new(newtype: &'newtype RustNewtype) -> Self {
        Self { newtype }
    }
}

impl ToTokens for NewtypeInherentImplTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.newtype.name().as_str());
        let payload_type = RustTypeReferenceTokens::new(self.newtype.reference());
        // String-backed newtypes take `impl Into<String>` so call sites pass
        // `&str` literals without `.to_string()`; other payloads keep the exact
        // type (integer literals would not infer through `impl Into`).
        let constructor = if self.newtype.takes_string_constructor() {
            quote! {
                pub fn new(payload: impl Into<String>) -> Self {
                    Self(payload.into())
                }
            }
        } else {
            quote! {
                pub fn new(payload: #payload_type) -> Self {
                    Self(payload)
                }
            }
        };
        quote! {
            impl #name {
                #constructor
                pub fn payload(&self) -> &#payload_type {
                    &self.0
                }
                pub fn into_payload(self) -> #payload_type {
                    self.0
                }
            }
            impl From<#payload_type> for #name {
                fn from(payload: #payload_type) -> Self {
                    Self::new(payload)
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// The backing scalar shape a recipe's payload-delegating body is valid for.
/// A standard impl body delegates to `self.payload()`, so the body is only
/// sound when the target newtype is ultimately backed by a primitive scalar.
/// [`Self::resolve`] follows a chain of `Plain` newtype references through the
/// module's declarations to the underlying scalar — so a transitive newtype
/// (`Statement(StatementText(String))`) resolves to `String`, closing the
/// blind spot the old direct-`TypeReference`-match `scalar_like()` left open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarShape {
    String,
    Integer,
    Boolean,
    NonScalar,
}

impl ScalarShape {
    /// Classify a type reference into its backing scalar, following `Plain`
    /// newtype references through the supplied declarations. A reference cycle
    /// or an unresolved plain name terminates as [`Self::NonScalar`].
    fn resolve(reference: &TypeReference, declarations: &[RustDeclaration]) -> Self {
        let mut current = reference.clone();
        let mut seen: Vec<String> = Vec::new();
        loop {
            match &current {
                TypeReference::String | TypeReference::Path => return Self::String,
                TypeReference::Integer => return Self::Integer,
                TypeReference::Boolean => return Self::Boolean,
                TypeReference::Plain(name) => {
                    let name = name.as_str().to_owned();
                    if seen.contains(&name) {
                        return Self::NonScalar;
                    }
                    seen.push(name.clone());
                    match declarations
                        .iter()
                        .find(|declaration| declaration.name().as_str() == name)
                        .map(RustDeclaration::value)
                    {
                        Some(RustTypeDeclaration::Newtype(newtype)) => {
                            current = newtype.reference().clone();
                        }
                        _ => return Self::NonScalar,
                    }
                }
                _ => return Self::NonScalar,
            }
        }
    }

    fn is_string(self) -> bool {
        matches!(self, Self::String)
    }

    fn is_integer(self) -> bool {
        matches!(self, Self::Integer)
    }

    fn is_boolean(self) -> bool {
        matches!(self, Self::Boolean)
    }

    fn is_scalar(self) -> bool {
        !matches!(self, Self::NonScalar)
    }
}

/// One recognized standard-impl recipe body, addressable per trait. Each body
/// is the payload-delegating one-liner the catalog trait names and the
/// generator owns — `Display` delegates to the scalar payload's own `Display`,
/// scalar comparisons compare the payload directly. The body carries the target
/// type name so it renders a complete `impl` block. This is the body library
/// the `{| … |}` catalog *selects from*; the catalog carries no Rust body.
#[derive(Clone, Debug, Eq, PartialEq)]
enum StandardImplBody {
    Display(Name),
    AsRefStr(Name),
    PartialEqStr(Name),
    PartialEqU64(Name),
    PartialOrdU64(Name),
    PartialEqBool(Name),
}

impl StandardImplBody {
    /// The [`ImplFact`] this emitted body attests to on the generated Rust
    /// surface — a trait impl of the corresponding `std` (or comparison) trait
    /// for the target type, keyed by the catalog trait atom the surface
    /// verifies against. The surface speaks the catalog's trait vocabulary
    /// (`Display`, `AsRef`, `PartialEq`, `PartialOrd`), not the spelled-out
    /// generic argument, so an emitted `PartialEq<&str>` attests the `PartialEq`
    /// reference the catalog carries.
    fn fact(&self) -> ImplFact {
        let (target, trait_atom) = match self {
            Self::Display(target) => (target, "Display"),
            Self::AsRefStr(target) => (target, "AsRef"),
            Self::PartialEqStr(target)
            | Self::PartialEqU64(target)
            | Self::PartialEqBool(target) => (target, "PartialEq"),
            Self::PartialOrdU64(target) => (target, "PartialOrd"),
        };
        ImplFact::trait_impl(target.clone(), Name::new(trait_atom))
    }
}

impl ToTokens for StandardImplBody {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Display(target) => {
                let name = RustIdentifier::new(target.as_str());
                quote! {
                    impl std::fmt::Display for #name {
                        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                            self.payload().fmt(formatter)
                        }
                    }
                }
            }
            Self::AsRefStr(target) => {
                let name = RustIdentifier::new(target.as_str());
                quote! {
                    impl AsRef<str> for #name {
                        fn as_ref(&self) -> &str {
                            self.payload().as_str()
                        }
                    }
                }
            }
            Self::PartialEqStr(target) => {
                let name = RustIdentifier::new(target.as_str());
                quote! {
                    impl PartialEq<&str> for #name {
                        fn eq(&self, other: &&str) -> bool {
                            self.payload() == other
                        }
                    }
                }
            }
            Self::PartialEqU64(target) => {
                let name = RustIdentifier::new(target.as_str());
                quote! {
                    impl PartialEq<u64> for #name {
                        fn eq(&self, other: &u64) -> bool {
                            self.payload() == other
                        }
                    }
                }
            }
            Self::PartialOrdU64(target) => {
                let name = RustIdentifier::new(target.as_str());
                quote! {
                    impl PartialOrd<u64> for #name {
                        fn partial_cmp(&self, other: &u64) -> Option<std::cmp::Ordering> {
                            self.payload().partial_cmp(other)
                        }
                    }
                }
            }
            Self::PartialEqBool(target) => {
                let name = RustIdentifier::new(target.as_str());
                quote! {
                    impl PartialEq<bool> for #name {
                        fn eq(&self, other: &bool) -> bool {
                            self.payload() == other
                        }
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// Resolves one `(trait, target-shape)` pair from the `{| … |}` catalog to the
/// standard-impl body the generator owns for it — the selection half of
/// catalog consumption. It holds the target type name, the catalog trait atom,
/// and the resolved backing [`ScalarShape`]; [`Self::recipe`] answers `Some`
/// for the recognized closed set under a scalar-backed shape, `None` for an
/// unrecognized trait, a derive-class trait (`Ord`), or a non-scalar shape —
/// `None` meaning "emit nothing, verify only." The scalar-shape predicates are
/// a GUARD here, not the trigger: the catalog says *whether* a type gets the
/// impl; the shape says *whether* the payload-delegating body is valid for it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct StandardImplRecipe {
    target: Name,
    trait_name: Name,
    shape: ScalarShape,
}

impl StandardImplRecipe {
    fn new(target: Name, trait_name: Name, shape: ScalarShape) -> Self {
        Self {
            target,
            trait_name,
            shape,
        }
    }

    /// Whether this entry is the derive-class `Ord` marker — recognized, but
    /// folded into the derive set rather than emitted as an `impl` body. The
    /// data types already derive `PartialOrd, Ord` when they are ordering-class,
    /// so an `Ord` reference is satisfied by the derive, not a hand body.
    fn is_derive_class(&self) -> bool {
        matches!(self.trait_name.as_str(), "Ord" | "PartialOrd") && !self.shape.is_integer()
    }

    /// The standard body this `(trait, shape)` resolves to, or `None` when the
    /// generator owns no body for it under this shape (verify-only).
    fn recipe(&self) -> Option<StandardImplBody> {
        if !self.shape.is_scalar() {
            return None;
        }
        match self.trait_name.as_str() {
            "Display" => Some(StandardImplBody::Display(self.target.clone())),
            "AsRef" if self.shape.is_string() => {
                Some(StandardImplBody::AsRefStr(self.target.clone()))
            }
            "PartialEq" if self.shape.is_string() => {
                Some(StandardImplBody::PartialEqStr(self.target.clone()))
            }
            "PartialEq" if self.shape.is_integer() => {
                Some(StandardImplBody::PartialEqU64(self.target.clone()))
            }
            "PartialEq" if self.shape.is_boolean() => {
                Some(StandardImplBody::PartialEqBool(self.target.clone()))
            }
            "PartialOrd" if self.shape.is_integer() => {
                Some(StandardImplBody::PartialOrdU64(self.target.clone()))
            }
            _ => None,
        }
    }
}

/// The Rust impl surface a [`RustModule`] genuinely emits, expressed as a
/// [`schema::ImplFact`] set — the producer that makes
/// [`schema::RustSurface::verify_catalog`] meaningful on a GENERATED
/// surface rather than the hand-built facts in schema's tests. The
/// [`From<&RustModule>`] form carries only what the generator emits/derives;
/// [`Self::for_schema`] additionally trusts the unrecognized references through,
/// so the trust boundary verifies the recognized subset and passes the rest.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EmittedRustSurface {
    facts: Vec<ImplFact>,
}

impl EmittedRustSurface {
    /// The emitted surface for verifying `schema` against `module`: the facts
    /// the generator genuinely emits, plus a trusted passthrough fact for every
    /// reference the generator does not recognize (externally-provided). A
    /// recognized reference contributes a fact only when genuinely emitted, so a
    /// missing recognized body still fails verification.
    fn for_schema(module: &RustModule, schema: &Schema) -> Self {
        let mut surface = Self::from(module);
        for reference in schema.referenced_impls() {
            if module.recognizes(reference.target(), reference.entry()) {
                continue;
            }
            Self::push_trusted_reference(&mut surface.facts, reference.target(), reference.entry());
        }
        surface
    }

    /// Add the facts an unrecognized reference is TRUSTED to provide: its trait
    /// impl (if any) and each of its method signatures. This is the
    /// externally-provided-unverified passthrough — the crate is trusted to
    /// carry these, so the boundary does not reject them.
    fn push_trusted_reference(facts: &mut Vec<ImplFact>, target: &Name, entry: &ImplReference) {
        if let Some(trait_name) = entry.trait_name() {
            facts.push(ImplFact::trait_impl(target.clone(), trait_name.clone()));
        }
        for signature in entry.methods() {
            facts.push(ImplFact::method(target.clone(), signature.clone()));
        }
    }

    fn into_surface(self) -> RustSurface {
        RustSurface::new(self.facts)
    }
}

/// The bare emitted surface — every [`ImplFact`] the module's own emission and
/// derives produce, with no trusted passthrough. This is the honest record of
/// what schema-rust ITSELF generates.
impl From<&RustModule> for EmittedRustSurface {
    fn from(module: &RustModule) -> Self {
        Self {
            facts: module.emitted_facts(),
        }
    }
}

/// Renders a cross-crate `use` import as Rust tokens. The dependency crate's
/// `ResolvedImport` already produced the canonical `pub use path as Alias;`
/// item text; this parses it into an item token so it pretty-prints with the
/// rest of the generated module.
struct RustImportTokens<'import> {
    import: &'import RustImport,
}

impl<'import> RustImportTokens<'import> {
    fn new(import: &'import RustImport) -> Self {
        Self { import }
    }
}

impl ToTokens for RustImportTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        syn::parse_str::<syn::ItemUse>(self.import.use_item())
            .expect("generated use item parses")
            .to_tokens(tokens);
    }
}

struct SignalFrameStreamingSupportTokens<'event> {
    event_payload: &'event TypeReference,
}

impl<'event> SignalFrameStreamingSupportTokens<'event> {
    fn new(event_payload: &'event TypeReference) -> Self {
        Self { event_payload }
    }
}

impl ToTokens for SignalFrameStreamingSupportTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let event_type = RustTypeReferenceTokens::new(self.event_payload);
        quote! {
            impl #event_type {
                pub fn into_subscription_frame(
                    self,
                    event_identifier: signal_frame::StreamEventIdentifier,
                    token: signal_frame::SubscriptionTokenInner,
                ) -> Frame {
                    Frame::with_short_header(
                        signal_frame::ShortHeader::new(short_header::OUTPUT_EVENT),
                        FrameBody::SubscriptionEvent {
                            event_identifier,
                            token,
                            event: self,
                        },
                    )
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct SignalFrameTransportSupportTokens<'schema> {
    input: &'schema RustEnum,
    event_payload: Option<&'schema TypeReference>,
}

impl<'schema> SignalFrameTransportSupportTokens<'schema> {
    fn new(input: &'schema RustEnum, event_payload: Option<&'schema TypeReference>) -> Self {
        Self {
            input,
            event_payload,
        }
    }
}

impl ToTokens for SignalFrameTransportSupportTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let heads = self
            .input
            .variants()
            .iter()
            .map(|variant| Literal::string(variant.name().as_str()));
        let frame_alias = match self.event_payload {
            Some(event_payload) => {
                let event_type = RustTypeReferenceTokens::new(event_payload);
                quote! {
                    pub type Frame = signal_frame::StreamingFrame<Input, Output, #event_type>;
                    pub type FrameBody = signal_frame::StreamingFrameBody<Input, Output, #event_type>;
                }
            }
            None => quote! {
                pub type Frame = signal_frame::ExchangeFrame<Input, Output>;
                pub type FrameBody = signal_frame::ExchangeFrameBody<Input, Output>;
            },
        };
        quote! {
            impl signal_frame::RequestPayload for Input {}

            impl signal_frame::SignalOperationHeads for Input {
                const HEADS: &'static [&'static str] = &[#(#heads),*];
            }

            impl signal_frame::LogVariant for Input {
                fn log_variant(&self) -> u64 {
                    self.short_header()
                }
            }

            #frame_alias
            pub type Request = signal_frame::Request<Input>;
            pub type ReplyEnvelope = signal_frame::Reply<Output>;
            pub type RequestBuilder = signal_frame::RequestBuilder<Input>;

            impl Input {
                pub fn into_frame(self, exchange: signal_frame::ExchangeIdentifier) -> Frame {
                    let short_header = signal_frame::ShortHeader::new(self.short_header());
                    let request = signal_frame::Request::from_payload(self);
                    Frame::with_short_header(
                        short_header,
                        FrameBody::Request { exchange, request },
                    )
                }
            }

            impl Output {
                pub fn into_reply_frame(
                    self,
                    exchange: signal_frame::ExchangeIdentifier,
                ) -> Frame {
                    let short_header = signal_frame::ShortHeader::new(self.short_header());
                    let reply = signal_frame::Reply::committed(signal_frame::NonEmpty::single(
                        signal_frame::SubReply::Ok(self),
                    ));
                    Frame::with_short_header(
                        short_header,
                        FrameBody::Reply { exchange, reply },
                    )
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct NexusRunnerNextStepProjectionTokens<'shape> {
    shape: &'shape NexusRunnerShape,
}

impl<'shape> NexusRunnerNextStepProjectionTokens<'shape> {
    fn new(shape: &'shape NexusRunnerShape) -> Self {
        Self { shape }
    }
}

impl ToTokens for NexusRunnerNextStepProjectionTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let reply_type = RustTypeTokens::new(&self.shape.reply_type);
        let sema_write_input_type = RustTypeTokens::new(self.shape.sema_write_input_type());
        let sema_read_input_type = RustTypeTokens::new(self.shape.sema_read_input_type());
        let effect_command_type = RustTypeTokens::new(self.shape.effect_command_type());
        let sema_write_arm = self.shape.emits_sema_write().then(|| {
            quote! {
                Self::CommandSemaWrite(input) => triad_runtime::NextStep::SemaWrite(input),
            }
        });
        let sema_read_arm = self.shape.emits_sema_read().then(|| {
            quote! {
                Self::CommandSemaRead(input) => triad_runtime::NextStep::SemaRead(input),
            }
        });
        let effect_arm = self.shape.emits_effect().then(|| {
            quote! {
                Self::CommandEffect(effect) => triad_runtime::NextStep::RunEffect(effect),
            }
        });
        let continue_arm = self.shape.has_continue.then(|| {
            quote! {
                Self::Continue(work) => triad_runtime::NextStep::Continue(work),
            }
        });

        quote! {
            pub type NexusRunnerNextStep = triad_runtime::NextStep<
                #reply_type,
                #sema_write_input_type,
                #sema_read_input_type,
                #effect_command_type,
                NexusWork,
            >;

            impl triad_runtime::NexusAction for NexusAction {
                type Reply = #reply_type;
                type SemaWrite = #sema_write_input_type;
                type SemaRead = #sema_read_input_type;
                type Effect = #effect_command_type;
                type Work = NexusWork;

                fn into_next_step(self) -> NexusRunnerNextStep {
                    match self {
                        #sema_write_arm
                        #sema_read_arm
                        Self::ReplyToSignal(output) => triad_runtime::NextStep::Reply(output),
                        #effect_arm
                        #continue_arm
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct NexusRunnerAdapterTokens<'shape> {
    shape: &'shape NexusRunnerShape,
}

impl<'shape> NexusRunnerAdapterTokens<'shape> {
    fn new(shape: &'shape NexusRunnerShape) -> Self {
        Self { shape }
    }
}

impl ToTokens for NexusRunnerAdapterTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let reply_type = RustTypeTokens::new(&self.shape.reply_type);
        let sema_write_input_type = RustTypeTokens::new(self.shape.sema_write_input_type());
        let sema_read_input_type = RustTypeTokens::new(self.shape.sema_read_input_type());
        let effect_command_type = RustTypeTokens::new(self.shape.effect_command_type());
        let apply_sema_write_body =
            if let Some(output_type) = self.shape.sema_write_output_type.as_deref() {
                let output_type = RustTypeTokens::new(output_type);
                quote! {
                    let output: #output_type = NexusEngine::apply_sema_write(
                        self.engine,
                        self.origin_route,
                        write,
                    )
                    .await;
                    NexusWork::sema_write_completed(output)
                }
            } else {
                quote! {
                    match write {}
                }
            };
        let observe_sema_read_body =
            if let Some(output_type) = self.shape.sema_read_output_type.as_deref() {
                let output_type = RustTypeTokens::new(output_type);
                quote! {
                    let output: #output_type = NexusEngine::observe_sema_read(
                        self.engine,
                        self.origin_route,
                        read,
                    )
                    .await;
                    NexusWork::sema_read_completed(output)
                }
            } else {
                quote! {
                    match read {}
                }
            };
        let run_effect_body = if let Some(output_type) = self.shape.effect_result_type.as_deref() {
            let output_type = RustTypeTokens::new(output_type);
            quote! {
                let output: #output_type = NexusEngine::run_effect(self.engine, effect).await;
                NexusWork::effect_completed(output)
            }
        } else {
            quote! {
                match effect {}
            }
        };

        quote! {
            struct NexusRunnerAdapter<'engine, Engine> {
                engine: &'engine mut Engine,
                origin_route: OriginRoute,
            }

            impl<'engine, Engine> NexusRunnerAdapter<'engine, Engine> {
                fn new(engine: &'engine mut Engine, origin_route: OriginRoute) -> Self {
                    Self {
                        engine,
                        origin_route,
                    }
                }
            }

            impl<'engine, Engine> triad_runtime::RunnerEngines
                for NexusRunnerAdapter<'engine, Engine>
            where
                Engine: NexusEngine,
            {
                type Reply = #reply_type;
                type SemaWrite = #sema_write_input_type;
                type SemaRead = #sema_read_input_type;
                type Effect = #effect_command_type;
                type Work = NexusWork;

                fn decide_next_step(
                    &mut self,
                    work: Self::Work,
                ) -> triad_runtime::runner::RunnerNextStep<Self> {
                    let action = NexusEngine::decide(
                        self.engine,
                        work.with_origin_route(self.origin_route),
                    )
                    .into_root();
                    triad_runtime::NexusAction::into_next_step(action)
                }

                async fn apply_sema_write(
                    &mut self,
                    write: Self::SemaWrite,
                ) -> Self::Work {
                    #apply_sema_write_body
                }

                async fn observe_sema_read(
                    &mut self,
                    read: Self::SemaRead,
                ) -> Self::Work {
                    #observe_sema_read_body
                }

                async fn run_effect(
                    &mut self,
                    effect: Self::Effect,
                ) -> Self::Work {
                    #run_effect_body
                }

                fn budget_exhausted_reply(
                    &self,
                    exhausted: triad_runtime::ContinuationExhausted,
                ) -> Self::Reply {
                    NexusEngine::budget_exhausted_reply(self.engine, exhausted)
                }
            }
        }
        .to_tokens(tokens);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeSupportSection {
    EngineLifecycle,
    SchemaPlane,
}

struct EngineLifecycleSupportTokens {
    section: RuntimeSupportSection,
}

impl EngineLifecycleSupportTokens {
    fn new() -> Self {
        Self {
            section: RuntimeSupportSection::EngineLifecycle,
        }
    }
}

impl ToTokens for EngineLifecycleSupportTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, RuntimeSupportSection::EngineLifecycle);
        quote! {
            #[derive(Clone, Debug, PartialEq, Eq)]
            pub enum EngineStartFailure {
                ResourceBusy(String),
                ConfigurationInvalid(String),
            }

            impl std::fmt::Display for EngineStartFailure {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        Self::ResourceBusy(message) => {
                            write!(formatter, "engine resource busy: {message}")
                        }
                        Self::ConfigurationInvalid(message) => {
                            write!(formatter, "engine configuration invalid: {message}")
                        }
                    }
                }
            }

            impl std::error::Error for EngineStartFailure {}

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub enum EngineStopFailure {
                ResourceLocked(String),
                ChildStillRunning(String),
            }

            impl std::fmt::Display for EngineStopFailure {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        Self::ResourceLocked(message) => {
                            write!(formatter, "engine resource locked: {message}")
                        }
                        Self::ChildStillRunning(message) => {
                            write!(formatter, "engine child still running: {message}")
                        }
                    }
                }
            }

            impl std::error::Error for EngineStopFailure {}
        }
        .to_tokens(tokens);
    }
}

struct SignalEngineTraitTokens {
    plane: Plane,
    emits_concrete_signal_engine: bool,
}

impl SignalEngineTraitTokens {
    fn new(emits_concrete_signal_engine: bool) -> Self {
        Self {
            plane: Plane::Signal,
            emits_concrete_signal_engine,
        }
    }
}

impl ToTokens for SignalEngineTraitTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let engine_trait = RustIdentifier::new(self.plane.engine_trait_name());
        let trace_enum = RustIdentifier::new(self.plane.trace_enum_name());
        let trace_activation = RustIdentifier::new(self.plane.trace_activation_method_name());
        let associated_nexus_types = (!self.emits_concrete_signal_engine).then(|| {
            quote! {
                type NexusInput;
                type NexusOutput;
            }
        });
        let triage_inner = if self.emits_concrete_signal_engine {
            quote! {
                fn triage_inner(
                    &self,
                    input: signal::Signal<signal::Input>,
                ) -> nexus::Nexus<nexus::Work>;
            }
        } else {
            quote! {
                fn triage_inner(&self, input: signal::Signal<signal::Input>) -> Self::NexusInput;
            }
        };
        let reply_inner = if self.emits_concrete_signal_engine {
            quote! {
                fn reply_inner(
                    &self,
                    output: nexus::Nexus<nexus::Action>,
                ) -> signal::Signal<signal::Output>;
            }
        } else {
            quote! {
                fn reply_inner(
                    &self,
                    output: Self::NexusOutput,
                ) -> signal::Signal<signal::Output>;
            }
        };
        let triage_output = if self.emits_concrete_signal_engine {
            quote! { nexus::Nexus<nexus::Work> }
        } else {
            quote! { Self::NexusInput }
        };
        let reply_input = if self.emits_concrete_signal_engine {
            quote! { nexus::Nexus<nexus::Action> }
        } else {
            quote! { Self::NexusOutput }
        };

        quote! {
            pub trait #engine_trait: Send {
                #associated_nexus_types

                fn on_start(&mut self) -> Result<(), EngineStartFailure> {
                    Ok(())
                }

                fn on_stop(&mut self) -> Result<(), EngineStopFailure> {
                    Ok(())
                }

                fn #trace_activation(&self, _object_name: #trace_enum) {}

                fn trace_signal_admitted(&self) {
                    self.#trace_activation(#trace_enum::Admitted);
                }

                fn trace_signal_rejected(&self) {
                    self.#trace_activation(#trace_enum::Rejected);
                }

                fn trace_signal_triaged(&self) {
                    self.#trace_activation(#trace_enum::Triaged);
                }

                fn trace_signal_replied(&self) {
                    self.#trace_activation(#trace_enum::Replied);
                }

                #triage_inner

                #reply_inner

                fn triage(&self, input: signal::Signal<signal::Input>) -> #triage_output {
                    let output = self.triage_inner(input);
                    self.trace_signal_triaged();
                    output
                }

                fn reply(&self, output: #reply_input) -> signal::Signal<signal::Output> {
                    let signal_output = self.reply_inner(output);
                    self.trace_signal_replied();
                    signal_output
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct NexusEngineTraitTokens<'shape> {
    plane: Plane,
    runner_shape: Option<&'shape NexusRunnerShape>,
}

impl<'shape> NexusEngineTraitTokens<'shape> {
    fn new(runner_shape: Option<&'shape NexusRunnerShape>) -> Self {
        Self {
            plane: Plane::Nexus,
            runner_shape,
        }
    }
}

impl ToTokens for NexusEngineTraitTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let engine_trait = RustIdentifier::new(self.plane.engine_trait_name());
        let trace_enum = RustIdentifier::new(self.plane.trace_enum_name());
        let trace_activation = RustIdentifier::new(self.plane.trace_activation_method_name());
        let runner_hooks = self.runner_shape.map(|shape| {
            let sema_write_hook = match (
                shape.sema_write_input_type.as_deref(),
                shape.sema_write_output_type.as_deref(),
            ) {
                (Some(input_type), Some(output_type)) => {
                    let input_type = RustTypeTokens::new(input_type);
                    let output_type = RustTypeTokens::new(output_type);
                    quote! {
                        fn apply_sema_write(
                            &mut self,
                            origin_route: OriginRoute,
                            input: #input_type,
                        ) -> impl std::future::Future<Output = #output_type> + Send + '_;
                    }
                }
                _ => quote! {},
            };
            let sema_read_hook = match (
                shape.sema_read_input_type.as_deref(),
                shape.sema_read_output_type.as_deref(),
            ) {
                (Some(input_type), Some(output_type)) => {
                    let input_type = RustTypeTokens::new(input_type);
                    let output_type = RustTypeTokens::new(output_type);
                    quote! {
                        fn observe_sema_read(
                            &mut self,
                            origin_route: OriginRoute,
                            input: #input_type,
                        ) -> impl std::future::Future<Output = #output_type> + Send + '_;
                    }
                }
                _ => quote! {},
            };
            let effect_hook = match (
                shape.effect_command_type.as_deref(),
                shape.effect_result_type.as_deref(),
            ) {
                (Some(input_type), Some(output_type)) => {
                    let input_type = RustTypeTokens::new(input_type);
                    let output_type = RustTypeTokens::new(output_type);
                    quote! {
                        fn run_effect(
                            &mut self,
                            input: #input_type,
                        ) -> impl std::future::Future<Output = #output_type> + Send + '_;
                    }
                }
                _ => quote! {},
            };
            let reply_type = RustTypeTokens::new(&shape.reply_type);
            quote! {
                fn continuation_limit(&self) -> triad_runtime::ContinuationLimit {
                    triad_runtime::ContinuationLimit::default()
                }

                #sema_write_hook

                #sema_read_hook

                #effect_hook

                fn budget_exhausted_reply(
                    &self,
                    exhausted: triad_runtime::ContinuationExhausted,
                ) -> #reply_type;
            }
        });
        let sized_where = self.runner_shape.is_some().then(|| {
            quote! {
                where
                    Self: Sized,
            }
        });
        let execute_body = if self.runner_shape.is_some() {
            quote! {
                let origin_route = input.origin_route();
                let first_work = input.into_root();
                let runner = triad_runtime::Runner::new(self.continuation_limit());
                let mut runner_adapter = NexusRunnerAdapter::new(self, origin_route);
                let reply = runner.drive(&mut runner_adapter, first_work).await;
                let output = NexusAction::reply_to_signal(reply).with_origin_route(origin_route);
            }
        } else {
            quote! {
                let output = self.decide(input);
            }
        };

        quote! {
            pub trait #engine_trait: Send {
                fn on_start(&mut self) -> Result<(), EngineStartFailure> {
                    Ok(())
                }

                fn on_stop(&mut self) -> Result<(), EngineStopFailure> {
                    Ok(())
                }

                fn #trace_activation(&self, _object_name: #trace_enum) {}

                fn trace_nexus_entered(&self) {
                    self.#trace_activation(#trace_enum::Entered);
                }

                fn trace_nexus_decided(&self) {
                    self.#trace_activation(#trace_enum::Decided);
                }

                #runner_hooks

                fn decide(
                    &mut self,
                    input: nexus::Nexus<nexus::Work>,
                ) -> nexus::Nexus<nexus::Action>;

                fn execute(
                    &mut self,
                    input: nexus::Nexus<nexus::Work>,
                ) -> impl std::future::Future<Output = nexus::Nexus<nexus::Action>> + Send + '_
                #sized_where
                {
                    async move {
                        self.trace_nexus_entered();
                        #execute_body
                        self.trace_nexus_decided();
                        output
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct SemaEngineTraitTokens {
    plane: Plane,
    emits_apply: bool,
    emits_observe: bool,
}

impl SemaEngineTraitTokens {
    fn new(emits_apply: bool, emits_observe: bool) -> Self {
        Self {
            plane: Plane::Sema,
            emits_apply,
            emits_observe,
        }
    }
}

impl ToTokens for SemaEngineTraitTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let engine_trait = RustIdentifier::new(self.plane.engine_trait_name());
        let trace_enum = RustIdentifier::new(self.plane.trace_enum_name());
        let trace_activation = RustIdentifier::new(self.plane.trace_activation_method_name());
        let write_trace = self.emits_apply.then(|| {
            quote! {
                fn trace_sema_write_applied(&self) {
                    self.#trace_activation(#trace_enum::WriteApplied);
                }
            }
        });
        let read_trace = self.emits_observe.then(|| {
            quote! {
                fn trace_sema_read_observed(&self) {
                    self.#trace_activation(#trace_enum::ReadObserved);
                }
            }
        });
        let apply_inner = self.emits_apply.then(|| {
            quote! {
                fn apply_inner(
                    &mut self,
                    input: sema::Sema<sema::WriteInput>,
                ) -> sema::Sema<sema::WriteOutput>;
            }
        });
        let observe_inner = self.emits_observe.then(|| {
            quote! {
                fn observe_inner(
                    &self,
                    input: sema::Sema<sema::ReadInput>,
                ) -> sema::Sema<sema::ReadOutput>;
            }
        });
        let apply = self.emits_apply.then(|| {
            quote! {
                fn apply(
                    &mut self,
                    input: sema::Sema<sema::WriteInput>,
                ) -> sema::Sema<sema::WriteOutput> {
                    let output = self.apply_inner(input);
                    self.trace_sema_write_applied();
                    output
                }
            }
        });
        let observe = self.emits_observe.then(|| {
            quote! {
                fn observe(&self, input: sema::Sema<sema::ReadInput>) -> sema::Sema<sema::ReadOutput> {
                    let output = self.observe_inner(input);
                    self.trace_sema_read_observed();
                    output
                }
            }
        });

        quote! {
            pub trait #engine_trait: Send {
                fn on_start(&mut self) -> Result<(), EngineStartFailure> {
                    Ok(())
                }

                fn on_stop(&mut self) -> Result<(), EngineStopFailure> {
                    Ok(())
                }

                fn #trace_activation(&self, _object_name: #trace_enum) {}

                #write_trace

                #read_trace

                #apply_inner

                #observe_inner

                #apply

                #observe
            }
        }
        .to_tokens(tokens);
    }
}

struct RuntimeCopyNewtypeTokens<'context> {
    name: &'static str,
    context: &'context RustRenderContext,
}

impl<'context> RuntimeCopyNewtypeTokens<'context> {
    fn new(name: &'static str, context: &'context RustRenderContext) -> Self {
        Self { name, context }
    }
}

impl ToTokens for RuntimeCopyNewtypeTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.name);
        let attributes = self.context.derive_attributes(true, false);
        quote! {
            #( #attributes )*
            pub struct #name(Integer);

            impl #name {
                pub fn new(payload: Integer) -> Self {
                    Self(payload)
                }
                pub fn payload(&self) -> Integer {
                    self.0
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct MessageRootTokens<'schema, 'context> {
    root_enums: &'schema [RustEnum],
    context: &'context RustRenderContext,
}

impl<'schema, 'context> MessageRootTokens<'schema, 'context> {
    fn new(root_enums: &'schema [RustEnum], context: &'context RustRenderContext) -> Self {
        Self {
            root_enums,
            context,
        }
    }
}

impl ToTokens for MessageRootTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.context.derive_attributes(true, false);
        let variants = self.root_enums.iter().map(|root_enum| {
            let name = RustIdentifier::new(root_enum.name().as_str());
            quote! { #name, }
        });
        quote! {
            #( #attributes )*
            pub enum MessageRoot {
                #( #variants )*
            }
        }
        .to_tokens(tokens);
    }
}

struct SignalMailLifecycleSupportTokens<'schema> {
    root_enums: &'schema [RustEnum],
}

impl<'schema> SignalMailLifecycleSupportTokens<'schema> {
    fn new(root_enums: &'schema [RustEnum]) -> Self {
        Self { root_enums }
    }
}

impl ToTokens for SignalMailLifecycleSupportTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let root_impls = self.root_enums.iter().map(|root_enum| {
            let root_name = RustIdentifier::new(root_enum.name().as_str());
            quote! {
                impl #root_name {
                    pub fn with_origin_route(self, origin_route: OriginRoute) -> Signal<Self> {
                        Signal::new(origin_route, self)
                    }
                }

                impl signal::Signal<#root_name> {
                    pub fn message_sent(&self, identifier: MessageIdentifier) -> MessageSent {
                        MessageSent {
                            identifier,
                            origin_route: self.origin_route(),
                            root: MessageRoot::#root_name,
                            short_header: self.root().short_header(),
                        }
                    }
                }
            }
        });

        quote! {
            #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
            pub struct MessageSent {
                pub identifier: MessageIdentifier,
                pub origin_route: OriginRoute,
                pub root: MessageRoot,
                pub short_header: Integer,
            }

            #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
            pub struct MessageProcessed<Reply> {
                pub identifier: MessageIdentifier,
                pub origin_route: OriginRoute,
                pub reply: Reply,
            }

            pub trait MessageSentHook {
                type Error;

                fn message_sent(&mut self, event: MessageSent) -> Result<(), Self::Error>;
            }

            pub trait MessageProcessedHook<Reply> {
                type Error;

                fn message_processed(
                    &mut self,
                    event: MessageProcessed<Reply>,
                ) -> Result<(), Self::Error>;
            }

            impl MessageSent {
                pub fn origin_route(&self) -> OriginRoute {
                    self.origin_route
                }

                pub fn push_to<Hook>(&self, hook: &mut Hook) -> Result<(), Hook::Error>
                where
                    Hook: MessageSentHook,
                {
                    hook.message_sent(self.clone())
                }
            }

            impl<Reply> MessageProcessed<Reply> {
                pub fn new(
                    identifier: MessageIdentifier,
                    origin_route: OriginRoute,
                    reply: Reply,
                ) -> Self {
                    Self {
                        identifier,
                        origin_route,
                        reply,
                    }
                }

                pub fn identifier(&self) -> MessageIdentifier {
                    self.identifier
                }

                pub fn origin_route(&self) -> OriginRoute {
                    self.origin_route
                }

                pub fn into_reply(self) -> Reply {
                    self.reply
                }

                pub fn push_to<Hook>(&self, hook: &mut Hook) -> Result<(), Hook::Error>
                where
                    Hook: MessageProcessedHook<Reply>,
                    Reply: Clone,
                {
                    hook.message_processed(self.clone())
                }
            }

            #( #root_impls )*
        }
        .to_tokens(tokens);
    }
}

struct SchemaPlaneSupportTokens {
    section: RuntimeSupportSection,
}

impl SchemaPlaneSupportTokens {
    fn new() -> Self {
        Self {
            section: RuntimeSupportSection::SchemaPlane,
        }
    }
}

impl ToTokens for SchemaPlaneSupportTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, RuntimeSupportSection::SchemaPlane);
        quote! {
            pub mod schema {
                #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
                pub enum Plane<SignalRoot, NexusRoot, SemaRoot> {
                    Signal(super::Signal<SignalRoot>),
                    Nexus(super::Nexus<NexusRoot>),
                    Sema(super::Sema<SemaRoot>),
                }

                impl<SignalRoot, NexusRoot, SemaRoot> Plane<SignalRoot, NexusRoot, SemaRoot> {
                    pub fn origin_route(&self) -> super::OriginRoute {
                        match self {
                            Self::Signal(message) => message.origin_route(),
                            Self::Nexus(message) => message.origin_route(),
                            Self::Sema(message) => message.origin_route(),
                        }
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct PlaneEnvelopeTokens<'name> {
    name: &'name str,
}

impl<'name> PlaneEnvelopeTokens<'name> {
    fn new(name: &'name str) -> Self {
        Self { name }
    }
}

impl ToTokens for PlaneEnvelopeTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.name);
        quote! {
            #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
            pub struct #name<Root> {
                pub origin_route: OriginRoute,
                pub root: Root,
            }

            impl<Root> #name<Root> {
                pub fn new(origin_route: OriginRoute, root: Root) -> Self {
                    Self { origin_route, root }
                }

                pub fn origin_route(&self) -> OriginRoute {
                    self.origin_route
                }

                pub fn root(&self) -> &Root {
                    &self.root
                }

                pub fn into_root(self) -> Root {
                    self.root
                }

                pub fn map_root<NextRoot>(self, map: impl FnOnce(Root) -> NextRoot) -> #name<NextRoot> {
                    #name::new(self.origin_route, map(self.root))
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct PlaneNamespaceAlias<'source> {
    export_name: &'static str,
    source_type_name: &'source str,
}

impl<'source> PlaneNamespaceAlias<'source> {
    fn new(export_name: &'static str, source_type_name: &'source str) -> Self {
        Self {
            export_name,
            source_type_name,
        }
    }
}

impl ToTokens for PlaneNamespaceAlias<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let export = RustIdentifier::new(self.export_name);
        let source = RustIdentifier::new(self.source_type_name);
        quote! {
            pub type #export = super::#source;
        }
        .to_tokens(tokens);
    }
}

struct PlaneNamespaceTokens<'source> {
    plane: Plane,
    aliases: Vec<PlaneNamespaceAlias<'source>>,
}

impl<'source> PlaneNamespaceTokens<'source> {
    fn new(plane: Plane, aliases: Vec<PlaneNamespaceAlias<'source>>) -> Self {
        Self { plane, aliases }
    }
}

impl ToTokens for PlaneNamespaceTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let module = RustIdentifier::new(self.plane.module_name());
        let wrapper = RustIdentifier::new(self.plane.wrapper_name());
        let aliases = &self.aliases;
        quote! {
            #[allow(clippy::module_inception)]
            pub mod #module {
                #(#aliases)*
                pub type #wrapper<Root> = super::#wrapper<Root>;
            }
        }
        .to_tokens(tokens);
    }
}

struct PlaneOriginRouteConstructorTokens<'source> {
    plane: Plane,
    type_name: &'source str,
}

impl<'source> PlaneOriginRouteConstructorTokens<'source> {
    fn new(plane: Plane, type_name: &'source str) -> Self {
        Self { plane, type_name }
    }
}

impl ToTokens for PlaneOriginRouteConstructorTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let type_name = RustIdentifier::new(self.type_name);
        let wrapper_path = PlaneWrapperPath::new(self.plane);
        quote! {
            impl #type_name {
                pub fn with_origin_route(self, origin_route: OriginRoute) -> #wrapper_path<Self> {
                    #wrapper_path::new(origin_route, self)
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct PlaneWrapperPath {
    plane: Plane,
}

impl PlaneWrapperPath {
    fn new(plane: Plane) -> Self {
        Self { plane }
    }
}

impl ToTokens for PlaneWrapperPath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper = RustIdentifier::new(self.plane.wrapper_name());
        match self.plane {
            Plane::Signal => quote! { #wrapper },
            Plane::Nexus | Plane::Sema => {
                let module = RustIdentifier::new(self.plane.module_name());
                quote! { #module::#wrapper }
            }
        }
        .to_tokens(tokens);
    }
}

struct TraceObjectNameEnumTokens<'schema, 'context> {
    plane: Plane,
    interface_roots: &'schema [TraceInterfaceRoot<'schema>],
    actor_variants: &'schema [&'static str],
    context: &'context RustRenderContext,
}

impl<'schema, 'context> TraceObjectNameEnumTokens<'schema, 'context> {
    fn new(
        plane: Plane,
        interface_roots: &'schema [TraceInterfaceRoot<'schema>],
        actor_variants: &'schema [&'static str],
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            plane,
            interface_roots,
            actor_variants,
            context,
        }
    }
}

impl ToTokens for TraceObjectNameEnumTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let enum_name = RustIdentifier::new(self.plane.trace_enum_name());
        let attributes = self.context.derive_attributes(true, false);
        let interface_variants = self.interface_roots.iter().map(|root| {
            let object_variant = RustIdentifier::new(root.object_variant);
            let route_name = format!("{}Route", root.type_name.as_str());
            let route_type = RustIdentifier::new(&route_name);
            quote! { #object_variant(#route_type), }
        });
        let actor_variants = self.actor_variants.iter().map(|variant| {
            let variant = RustIdentifier::new(variant);
            quote! { #variant, }
        });
        let interface_match_arms = self.interface_roots.iter().map(|root| {
            let object_variant = RustIdentifier::new(root.object_variant);
            let route_name = format!("{}Route", root.type_name.as_str());
            let route_type = RustIdentifier::new(&route_name);
            let route_arms = root.enum_declaration.variants().iter().map(|variant| {
                let variant_name = RustIdentifier::new(variant.name().as_str());
                let rendered_name = format!("{}{}", root.name_prefix, variant.name());
                let rendered_name = Literal::string(&rendered_name);
                quote! { #route_type::#variant_name => #rendered_name, }
            });
            quote! {
                Self::#object_variant(route) => match route {
                    #( #route_arms )*
                },
            }
        });
        let actor_match_arms = self.actor_variants.iter().map(|variant| {
            let variant_name = RustIdentifier::new(variant);
            let rendered_name = format!("{}{}", self.plane.wrapper_name(), variant);
            let rendered_name = Literal::string(&rendered_name);
            quote! { Self::#variant_name => #rendered_name, }
        });
        quote! {
            #( #attributes )*
            pub enum #enum_name {
                #( #interface_variants )*
                #( #actor_variants )*
            }

            impl #enum_name {
                pub fn name(self) -> &'static str {
                    match self {
                        #( #interface_match_arms )*
                        #( #actor_match_arms )*
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct TraceSupportTokens<'context> {
    planes: Vec<Plane>,
    context: &'context RustRenderContext,
}

impl<'context> TraceSupportTokens<'context> {
    fn new(planes: Vec<Plane>, context: &'context RustRenderContext) -> Self {
        Self { planes, context }
    }
}

impl ToTokens for TraceSupportTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.context.derive_attributes(true, false);
        let variants = self.planes.iter().map(|plane| {
            let variant = RustIdentifier::new(plane.wrapper_name());
            let trace_enum = RustIdentifier::new(plane.trace_enum_name());
            quote! { #variant(#trace_enum), }
        });
        let match_arms = self.planes.iter().map(|plane| {
            let variant = RustIdentifier::new(plane.wrapper_name());
            quote! { Self::#variant(object_name) => object_name.name(), }
        });
        quote! {
            #( #attributes )*
            pub enum ObjectName {
                #( #variants )*
            }

            #( #attributes )*
            pub struct TraceEvent(pub ObjectName);

            impl ObjectName {
                pub fn name(self) -> &'static str {
                    match self {
                        #( #match_arms )*
                    }
                }
            }

            impl TraceEvent {
                pub fn new(object_name: ObjectName) -> Self {
                    Self(object_name)
                }

                pub fn object_name(&self) -> ObjectName {
                    self.0
                }

                pub fn name(&self) -> &'static str {
                    self.0.name()
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct RustDeclarationTokens<'declaration, 'context> {
    declaration: &'declaration RustDeclaration,
    context: &'context RustRenderContext,
}

impl<'declaration, 'context> RustDeclarationTokens<'declaration, 'context> {
    fn new(
        declaration: &'declaration RustDeclaration,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            declaration,
            context,
        }
    }
}

impl ToTokens for RustDeclarationTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.declaration.value() {
            RustTypeDeclaration::Struct(value) => {
                RustStructTokens::new(value, self.declaration.visibility(), self.context)
                    .to_tokens(tokens)
            }
            RustTypeDeclaration::Enum(value) => {
                RustEnumTokens::new(value, self.declaration.visibility(), self.context)
                    .to_tokens(tokens)
            }
            RustTypeDeclaration::Newtype(value) => {
                RustNewtypeTokens::new(value, self.declaration.visibility(), self.context)
                    .to_tokens(tokens)
            }
        }
    }
}

struct RustNewtypeTokens<'newtype, 'context> {
    newtype: &'newtype RustNewtype,
    visibility: Visibility,
    context: &'context RustRenderContext,
}

impl<'newtype, 'context> RustNewtypeTokens<'newtype, 'context> {
    fn new(
        newtype: &'newtype RustNewtype,
        visibility: Visibility,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            newtype,
            visibility,
            context,
        }
    }
}

impl ToTokens for RustNewtypeTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.context.data_type_attributes(self.newtype.name());
        let visibility = self.context.visibility_tokens(self.visibility);
        let name = RustIdentifier::new(self.newtype.name().as_str());
        let reference = RustTypeReferenceTokens::new(self.newtype.reference());
        quote! {
            #(#attributes)*
            #visibility struct #name(#reference);
        }
        .to_tokens(tokens);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopeEnumModel {
    source_name: String,
    emitted_name: String,
    root: bool,
    variants: Vec<ScopeEnumVariantModel>,
}

impl ScopeEnumModel {
    fn from_scope_newtype(
        newtype: &RustNewtype,
        declarations: &[RustDeclaration],
    ) -> Option<Vec<Self>> {
        let root_name = newtype.scope_root_name()?;
        let root = Self::enum_named(declarations, root_name.as_str())?;
        let mut models = Vec::new();
        Self::push_model(
            root,
            newtype.name().as_str().to_owned(),
            true,
            declarations,
            &mut models,
        );
        Some(models)
    }

    fn push_model(
        source: &RustEnum,
        emitted_name: String,
        root: bool,
        declarations: &[RustDeclaration],
        models: &mut Vec<Self>,
    ) {
        if models
            .iter()
            .any(|model: &Self| model.source_name == source.name().as_str())
        {
            return;
        }
        let variants = source
            .variants()
            .iter()
            .map(|variant| {
                let payload_source = variant.payload().and_then(Self::scope_payload_source_name);
                let payload_scope = variant
                    .payload()
                    .and_then(Self::scope_payload_source_name)
                    .and_then(|name| Self::enum_named(declarations, name.as_str()))
                    .map(|payload| {
                        let emitted = Self::child_scope_name(payload.name());
                        Self::push_model(payload, emitted.clone(), false, declarations, models);
                        emitted
                    });
                ScopeEnumVariantModel::new(
                    variant.name().as_str().to_owned(),
                    payload_source,
                    payload_scope,
                )
            })
            .collect();
        models.push(Self {
            source_name: source.name().as_str().to_owned(),
            emitted_name,
            root,
            variants,
        });
    }

    fn enum_named<'declaration>(
        declarations: &'declaration [RustDeclaration],
        name: &str,
    ) -> Option<&'declaration RustEnum> {
        declarations
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
            .and_then(|declaration| match declaration.value() {
                RustTypeDeclaration::Enum(value) => Some(value),
                _ => None,
            })
    }

    fn child_scope_name(name: &Name) -> String {
        format!("{}Scope", name.as_str())
    }

    fn scope_payload_source_name(reference: &TypeReference) -> Option<String> {
        match reference {
            TypeReference::Plain(name) => Some(name.as_str().to_owned()),
            _ => None,
        }
    }

    fn model_named<'model>(
        models: &'model [Self],
        emitted_name: &str,
    ) -> Option<&'model ScopeEnumModel> {
        models
            .iter()
            .find(|model| model.emitted_name.as_str() == emitted_name)
    }

    fn constructor_tokens(&self, models: &[Self], path: &[Name]) -> TokenStream {
        let (head, tail) = path
            .split_first()
            .unwrap_or_else(|| panic!("empty scope relation path for {}", self.emitted_name));
        let variant = self
            .variants
            .iter()
            .find(|variant| variant.name.as_str() == head.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "scope relation path segment {} is not a variant of {}",
                    head.as_str(),
                    self.emitted_name
                )
            });
        let scope_name = RustIdentifier::new(&self.emitted_name);
        let variant_name = RustIdentifier::new(&variant.name);
        match &variant.payload_scope {
            Some(payload_scope) => {
                let payload_name = RustIdentifier::new(payload_scope);
                let payload_model = Self::model_named(models, payload_scope)
                    .unwrap_or_else(|| panic!("missing scope model for payload {}", payload_scope));
                let payload = if tail.is_empty() {
                    quote! { #payload_name::All }
                } else {
                    payload_model.constructor_tokens(models, tail)
                };
                quote! { #scope_name::#variant_name(#payload) }
            }
            None if tail.is_empty() => quote! { #scope_name::#variant_name },
            None => {
                panic!(
                    "scope relation path continues past leaf {}::{}",
                    self.emitted_name, variant.name
                )
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopeEnumVariantModel {
    name: String,
    payload_source: Option<String>,
    payload_scope: Option<String>,
}

impl ScopeEnumVariantModel {
    /// The scope-universal variant name. A non-root scope enum always carries
    /// this catch-all (`Self::All`), and a source enum whose own leaf-unspecified
    /// member is named `All` collapses into that same catch-all rather than
    /// producing a second variant.
    const UNIVERSAL: &'static str = "All";

    fn new(name: String, payload_source: Option<String>, payload_scope: Option<String>) -> Self {
        Self {
            name,
            payload_source,
            payload_scope,
        }
    }

    /// Whether this source variant is the leaf-unspecified `All` member that a
    /// non-root scope enum represents with its auto-injected `Self::All`.
    fn is_universal(&self) -> bool {
        self.name == Self::UNIVERSAL
    }
}

struct ScopeFamilyTokens<'newtype, 'declarations, 'context> {
    newtype: &'newtype RustNewtype,
    declarations: &'declarations [RustDeclaration],
    visibility: Visibility,
    context: &'context RustRenderContext,
}

impl<'newtype, 'declarations, 'context> ScopeFamilyTokens<'newtype, 'declarations, 'context> {
    fn new(
        newtype: &'newtype RustNewtype,
        declarations: &'declarations [RustDeclaration],
        visibility: Visibility,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            newtype,
            declarations,
            visibility,
            context,
        }
    }
}

impl ToTokens for ScopeFamilyTokens<'_, '_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Some(models) = ScopeEnumModel::from_scope_newtype(self.newtype, self.declarations)
        else {
            RustNewtypeTokens::new(self.newtype, self.visibility, self.context).to_tokens(tokens);
            return;
        };
        let enum_tokens = models
            .iter()
            .rev()
            .map(|model| ScopeEnumTokens::new(model, self.visibility, self.context));
        let operation_tokens = models.iter().map(ScopeOperationImplTokens::new);
        let nota_tokens = if self.context.nota_surface.emits_nota() {
            let string_support =
                NotaRootEnumStringSupportTokens::new(self.newtype.name().as_str(), self.context);
            quote! {
                #string_support
            }
        } else {
            TokenStream::new()
        };
        quote! {
            #(#enum_tokens)*
            #(#operation_tokens)*
            #nota_tokens
        }
        .to_tokens(tokens);
    }
}

struct ScopeEnumTokens<'model, 'context> {
    model: &'model ScopeEnumModel,
    visibility: Visibility,
    context: &'context RustRenderContext,
}

impl<'model, 'context> ScopeEnumTokens<'model, 'context> {
    fn new(
        model: &'model ScopeEnumModel,
        visibility: Visibility,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            model,
            visibility,
            context,
        }
    }
}

impl ToTokens for ScopeEnumTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.context.scope_enum_type_attributes();
        let visibility = self.context.visibility_tokens(self.visibility);
        let name = RustIdentifier::new(&self.model.emitted_name);
        let all = (!self.model.root).then(|| {
            quote! {
                All,
            }
        });
        let variants = self
            .model
            .variants
            .iter()
            .filter(|variant| self.model.root || !variant.is_universal())
            .map(|variant| {
                let variant_name = RustIdentifier::new(&variant.name);
                match &variant.payload_scope {
                    Some(payload_scope) => {
                        let payload = RustIdentifier::new(payload_scope);
                        quote! { #variant_name(#payload), }
                    }
                    None => quote! { #variant_name, },
                }
            });
        quote! {
            #(#attributes)*
            #visibility enum #name {
                #all
                #(#variants)*
            }
        }
        .to_tokens(tokens);
    }
}

struct ScopeOperationImplTokens<'model> {
    model: &'model ScopeEnumModel,
}

impl<'model> ScopeOperationImplTokens<'model> {
    fn new(model: &'model ScopeEnumModel) -> Self {
        Self { model }
    }
}

impl ToTokens for ScopeOperationImplTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(&self.model.emitted_name);
        let source = RustIdentifier::new(&self.model.source_name);
        let from_arms = self.model.variants.iter().map(|variant| {
            let variant_name = RustIdentifier::new(&variant.name);
            match &variant.payload_source {
                Some(_) => quote! {
                    #source::#variant_name(payload) => Self::#variant_name(payload.into()),
                },
                None => quote! {
                    #source::#variant_name => Self::#variant_name,
                },
            }
        });
        let contains_arms = self
            .model
            .variants
            .iter()
            .filter(|variant| self.model.root || !variant.is_universal())
            .map(|variant| {
                let variant_name = RustIdentifier::new(&variant.name);
                match &variant.payload_scope {
                    Some(_) => quote! {
                        (Self::#variant_name(left), Self::#variant_name(right)) => {
                            left.contains_scope(right)
                        }
                    },
                    None => quote! {
                        (Self::#variant_name, Self::#variant_name) => true,
                    },
                }
            });
        let all_contains_arm = (!self.model.root).then(|| {
            quote! {
                (Self::All, _) => true,
            }
        });
        let contains_scope_body = if self
            .model
            .variants
            .iter()
            .all(|variant| variant.payload_scope.is_none())
        {
            let all_pattern = (!self.model.root).then(|| {
                quote! {
                    (Self::All, _)
                }
            });
            let leaf_patterns = self
                .model
                .variants
                .iter()
                .filter(|variant| self.model.root || !variant.is_universal())
                .map(|variant| {
                    let variant_name = RustIdentifier::new(&variant.name);
                    quote! {
                        (Self::#variant_name, Self::#variant_name)
                    }
                });
            let patterns = all_pattern.into_iter().chain(leaf_patterns);
            quote! {
                matches!((self, scope), #(#patterns)|*)
            }
        } else {
            quote! {
                match (self, scope) {
                    #all_contains_arm
                    #(#contains_arms)*
                    _ => false,
                }
            }
        };
        let contains_domain = self.model.root.then(|| {
            quote! {
                pub fn contains_domain(&self, domain: &#source) -> bool {
                    self.contains_scope(&domain.clone().into())
                }
            }
        });
        quote! {
            impl From<#source> for #name {
                fn from(value: #source) -> Self {
                    match value {
                        #(#from_arms)*
                    }
                }
            }

            impl #name {
                pub fn contains_scope(&self, scope: &Self) -> bool {
                    #contains_scope_body
                }

                #contains_domain
            }
        }
        .to_tokens(tokens);
    }
}

struct RustStructTokens<'structure, 'context> {
    structure: &'structure RustStruct,
    visibility: Visibility,
    context: &'context RustRenderContext,
}

impl<'structure, 'context> RustStructTokens<'structure, 'context> {
    fn new(
        structure: &'structure RustStruct,
        visibility: Visibility,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            structure,
            visibility,
            context,
        }
    }
}

impl ToTokens for RustStructTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.context.data_type_attributes(self.structure.name());
        let visibility = self.context.visibility_tokens(self.visibility);
        let name = RustIdentifier::new(self.structure.name().as_str());
        let fields = self
            .structure
            .fields()
            .iter()
            .map(|field| RustFieldTokens::new(field, self.visibility, self.context))
            .collect::<Vec<_>>();
        quote! {
            #(#attributes)*
            #visibility struct #name {
                #(#fields)*
            }
        }
        .to_tokens(tokens);
    }
}

struct RustFieldTokens<'field, 'context> {
    field: &'field RustField,
    visibility: Visibility,
    context: &'context RustRenderContext,
}

impl<'field, 'context> RustFieldTokens<'field, 'context> {
    fn new(
        field: &'field RustField,
        visibility: Visibility,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            field,
            visibility,
            context,
        }
    }
}

impl ToTokens for RustFieldTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let visibility = self
            .context
            .field_visibility_tokens(self.visibility, self.field.reference());
        let name = RustIdentifier::new(self.field.name().as_str());
        let reference = RustTypeReferenceTokens::new(self.field.reference());
        quote! {
            #visibility #name: #reference,
        }
        .to_tokens(tokens);
    }
}

struct RustEnumTokens<'enumeration, 'context> {
    enumeration: &'enumeration RustEnum,
    visibility: Visibility,
    context: &'context RustRenderContext,
    root: bool,
}

impl<'enumeration, 'context> RustEnumTokens<'enumeration, 'context> {
    fn new(
        enumeration: &'enumeration RustEnum,
        visibility: Visibility,
        context: &'context RustRenderContext,
    ) -> Self {
        Self {
            enumeration,
            visibility,
            context,
            root: false,
        }
    }

    fn root(enumeration: &'enumeration RustEnum, context: &'context RustRenderContext) -> Self {
        Self {
            enumeration,
            visibility: Visibility::Public,
            context,
            root: true,
        }
    }

    fn attributes(&self) -> Vec<TokenStream> {
        if self.root {
            self.context.root_enum_type_attributes(self.enumeration)
        } else {
            self.context.enum_type_attributes(self.enumeration)
        }
    }
}

impl ToTokens for RustEnumTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let attributes = self.attributes();
        let visibility = self.context.visibility_tokens(self.visibility);
        let name = RustIdentifier::new(self.enumeration.name().as_str());
        let generics = RustGenericParameterTokens::new(self.enumeration.parameters());
        let variants = self
            .enumeration
            .variants()
            .iter()
            .map(RustEnumVariantTokens::new)
            .collect::<Vec<_>>();
        quote! {
            #(#attributes)*
            #visibility enum #name #generics {
                #(#variants)*
            }
        }
        .to_tokens(tokens);
    }
}

/// The `<P1, P2, …>` generic-parameter list emitted after a type name. An
/// empty parameter slice emits nothing, so an ordinary enum stays a bare
/// `enum Name { … }` while a parameterized frame enum becomes
/// `enum Work<Event, Write, Read, Effect> { … }`.
struct RustGenericParameterTokens<'parameter> {
    parameters: &'parameter [Name],
}

impl<'parameter> RustGenericParameterTokens<'parameter> {
    fn new(parameters: &'parameter [Name]) -> Self {
        Self { parameters }
    }
}

impl ToTokens for RustGenericParameterTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.parameters.is_empty() {
            return;
        }
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| RustIdentifier::new(parameter.as_str()).ident());
        quote! { <#(#parameters),*> }.to_tokens(tokens);
    }
}

/// `pub type <position> = <Head><Args>;` for a frame-applying root. The
/// applied type renders through [`RustTypeReferenceTokens`], so the head
/// name resolves to the imported frame alias and each argument to its
/// component-local payload type.
struct AppliedRootTokens<'root> {
    root: &'root RustAppliedRoot,
}

impl<'root> AppliedRootTokens<'root> {
    fn new(root: &'root RustAppliedRoot) -> Self {
        Self { root }
    }
}

impl ToTokens for AppliedRootTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.root.name().as_str());
        let applied = RustTypeReferenceTokens::new(self.root.applied());
        quote! {
            pub type #name = #applied;
        }
        .to_tokens(tokens);
    }
}

struct RustEnumVariantTokens<'variant> {
    variant: &'variant RustEnumVariant,
}

impl<'variant> RustEnumVariantTokens<'variant> {
    fn new(variant: &'variant RustEnumVariant) -> Self {
        Self { variant }
    }
}

impl ToTokens for RustEnumVariantTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = RustIdentifier::new(self.variant.name().as_str());
        match self.variant.payload() {
            Some(reference) => {
                let reference = RustTypeReferenceTokens::new(reference);
                quote! { #name(#reference), }.to_tokens(tokens);
            }
            None => quote! { #name, }.to_tokens(tokens),
        }
    }
}

struct DomainScopeRelationSupportTokens<'relation, 'model> {
    relations: &'relation [RustRelation],
    root: &'model ScopeEnumModel,
    models: &'model [ScopeEnumModel],
}

impl<'relation, 'model> DomainScopeRelationSupportTokens<'relation, 'model> {
    fn new(
        relations: &'relation [RustRelation],
        root: &'model ScopeEnumModel,
        models: &'model [ScopeEnumModel],
    ) -> Self {
        Self {
            relations,
            root,
            models,
        }
    }
}

impl ToTokens for DomainScopeRelationSupportTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let classes = self.relations.iter().map(|relation| {
            let scopes = relation
                .values()
                .iter()
                .map(|value| DomainScopeValueTokens::new(value, self.root, self.models));
            quote! { vec![#(#scopes),*] }
        });
        quote! {
            impl DomainScope {
                pub fn expand(&self) -> ScopeSet {
                    let mut scopes = vec![self.clone()];
                    for relation in Self::equivalence_relations() {
                        if relation.iter().any(|scope| scope == self) {
                            for scope in relation {
                                if !scopes.iter().any(|existing| existing == &scope) {
                                    scopes.push(scope);
                                }
                            }
                        }
                    }
                    ScopeSet::new(scopes)
                }

                pub fn equivalence_relations() -> Vec<Vec<DomainScope>> {
                    vec![#(#classes),*]
                }
            }
        }
        .to_tokens(tokens);
    }
}

struct DomainScopeValueTokens<'value, 'model> {
    value: &'value RustRelationValue,
    root: &'model ScopeEnumModel,
    models: &'model [ScopeEnumModel],
}

impl<'value, 'model> DomainScopeValueTokens<'value, 'model> {
    fn new(
        value: &'value RustRelationValue,
        root: &'model ScopeEnumModel,
        models: &'model [ScopeEnumModel],
    ) -> Self {
        Self {
            value,
            root,
            models,
        }
    }
}

impl ToTokens for DomainScopeValueTokens<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.root
            .constructor_tokens(self.models, self.value.path())
            .to_tokens(tokens);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustModulePath {
    schema_name: Name,
}

impl RustModulePath {
    fn new(schema_name: Name) -> Self {
        Self { schema_name }
    }

    fn to_file_path(&self) -> String {
        let mut segments = self.module_segments();
        let file = segments.pop().unwrap_or_else(|| "lib".to_owned());
        if segments.is_empty() {
            format!("src/schema/{file}.rs")
        } else {
            format!("src/schema/{}/{}.rs", segments.join("/"), file)
        }
    }

    fn module_segments(&self) -> Vec<String> {
        let segments = self.schema_name.namespace_segments();
        let module_segments = if segments.len() > 1 {
            &segments[1..]
        } else {
            segments.as_slice()
        };
        module_segments
            .iter()
            .map(|segment| Name::new(*segment).field_name())
            .collect()
    }
}

/// The `family_identity` module: one 32-byte schema-hash constant per
/// declared record family, SCREAMING_SNAKE-named after the family, on
/// the `short_header` module precedent. The values are computed at
/// generation time from each family record's schema closure, so the
/// generated artifact pins the per-family version identity and a schema
/// edit surfaces as a constant move under the freshness check.
struct FamilyIdentityModuleTokens<'store> {
    families: &'store [RustRecordFamily],
}

impl<'store> FamilyIdentityModuleTokens<'store> {
    fn new(families: &'store [RustRecordFamily]) -> Self {
        Self { families }
    }
}

impl ToTokens for FamilyIdentityModuleTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let constants = self.families.iter().map(|family| {
            let constant = family.constant_identifier();
            let bytes = family
                .schema_hash()
                .iter()
                .map(|byte| Literal::u8_unsuffixed(*byte));
            quote! { pub const #constant: [u8; 32] = [#(#bytes),*]; }
        });
        quote! {
            pub mod family_identity {
                #(#constants)*
            }
        }
        .to_tokens(tokens);
    }
}

/// The closed `RecordFamily` sum over the component's declared record
/// families, with the whole version-control surface attached: the store
/// name and `versioning_policy` constructor, one descriptor constructor
/// per family returning the matching `sema_engine` table descriptor,
/// and the `decode` dispatch from a stored `FamilyIdentity` plus rkyv
/// bytes into the typed sum. Unknown families and schema-hash drift are
/// typed [`RecordFamilyError`] values, never a fallback.
struct RecordFamilyEnumTokens<'store> {
    store: &'store RustVersionedStore,
}

impl<'store> RecordFamilyEnumTokens<'store> {
    fn new(store: &'store RustVersionedStore) -> Self {
        Self { store }
    }
}

impl ToTokens for RecordFamilyEnumTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variants = self.store.families().iter().map(|family| {
            let variant = RustIdentifier::new(family.name().as_str());
            let record = RustIdentifier::new(family.record().as_str());
            quote! { #variant(#record), }
        });
        let descriptor_constructors = self.store.families().iter().map(|family| {
            let constructor = family.constructor_identifier();
            let descriptor_type = family.descriptor_type();
            let descriptor_head = match family.key() {
                FamilyKey::Domain => quote! { sema_engine::TableDescriptor::new },
                FamilyKey::Identified => quote! { sema_engine::IdentifiedTableDescriptor::new },
            };
            let table = Literal::string(family.table());
            let family_name = Literal::string(family.name().as_str());
            let constant = family.constant_identifier();
            quote! {
                pub fn #constructor() -> #descriptor_type {
                    #descriptor_head(
                        sema_engine::TableName::new(#table),
                        sema_engine::FamilyName::new(#family_name),
                        sema_engine::SchemaHash::new(family_identity::#constant),
                    )
                }
            }
        });
        let decode_arms = self.store.families().iter().map(|family| {
            let variant = RustIdentifier::new(family.name().as_str());
            let record = RustIdentifier::new(family.record().as_str());
            let family_name = Literal::string(family.name().as_str());
            let constant = family.constant_identifier();
            quote! {
                #family_name => {
                    let generated = sema_engine::SchemaHash::new(family_identity::#constant);
                    if identity.schema_hash() != generated {
                        return Err(RecordFamilyError::SchemaHashMismatch {
                            family: sema_engine::FamilyName::new(#family_name),
                            stored: identity.schema_hash(),
                            generated,
                        });
                    }
                    let record = rkyv::from_bytes::<#record, rkyv::rancor::Error>(bytes)
                        .map_err(|_| RecordFamilyError::RecordDecode {
                            family: sema_engine::FamilyName::new(#family_name),
                        })?;
                    Ok(Self::#variant(record))
                }
            }
        });
        let store_name = Literal::string(self.store.store_name());
        quote! {
            #[derive(Clone, Debug, PartialEq)]
            pub enum RecordFamily {
                #(#variants)*
            }

            impl RecordFamily {
                pub const STORE_NAME: &'static str = #store_name;

                pub fn versioning_policy() -> sema_engine::VersioningPolicy {
                    sema_engine::VersioningPolicy::new(
                        sema_engine::VersionedStoreName::new(Self::STORE_NAME),
                    )
                }

                #(#descriptor_constructors)*

                pub fn decode(
                    identity: &sema_engine::FamilyIdentity,
                    bytes: &[u8],
                ) -> Result<Self, RecordFamilyError> {
                    match identity.family().as_str() {
                        #(#decode_arms)*
                        _ => Err(RecordFamilyError::UnknownFamily {
                            family: identity.family().clone(),
                        }),
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// Decides which fully specified schema type names appear as map keys.
#[derive(Clone, Copy, Debug)]
struct CollectionScan<'schema> {
    schema: &'schema SpecifiedSchema,
}

impl<'schema> CollectionScan<'schema> {
    fn new(schema: &'schema SpecifiedSchema) -> Self {
        Self { schema }
    }

    /// The plain type names that appear as a `BTreeMap` key anywhere in
    /// the schema (field references, variant payloads, and nested
    /// collection positions). These types need the ordering derives.
    fn map_key_type_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for declaration in self.schema.declarations() {
            Self::collect_declaration_map_keys(declaration.body(), &mut names);
        }
        for root in [self.schema.input(), self.schema.output()] {
            Self::collect_root_map_keys(root, &mut names);
        }
        names
    }

    /// Collect map-key type names from either root shape: an enum root walks
    /// its variant payloads; an application root walks its applied reference.
    fn collect_root_map_keys(root: &SpecifiedRoot, names: &mut Vec<String>) {
        match root {
            SpecifiedRoot::Enum(declaration) => {
                for variant in declaration.variants() {
                    if let Some(payload) = variant.payload() {
                        Self::collect_map_keys(payload.reference(), names);
                    }
                }
            }
            SpecifiedRoot::Application(application) => {
                Self::collect_map_keys(application.reference(), names);
            }
        }
    }

    /// Whether the schema references the `Bytes` scalar anywhere (directly or
    /// nested in a collection), so the renderer only emits the `Bytes`
    /// newtype-prelude + hex codec when a module actually uses it.
    fn references_bytes(&self) -> bool {
        self.schema
            .declarations()
            .iter()
            .any(|declaration| Self::declaration_uses_bytes(declaration.body()))
            || [self.schema.input(), self.schema.output()]
                .into_iter()
                .any(Self::root_uses_bytes)
    }

    fn root_uses_bytes(root: &SpecifiedRoot) -> bool {
        match root {
            SpecifiedRoot::Enum(declaration) => declaration.variants().iter().any(|variant| {
                variant
                    .payload()
                    .is_some_and(|payload| Self::reference_uses_bytes(payload.reference()))
            }),
            SpecifiedRoot::Application(application) => {
                Self::reference_uses_bytes(application.reference())
            }
        }
    }

    fn declaration_uses_bytes(declaration: &SpecifiedDeclarationBody) -> bool {
        match declaration {
            SpecifiedDeclarationBody::Struct(fields) => fields
                .iter()
                .any(|field| Self::reference_uses_bytes(field.reference())),
            SpecifiedDeclarationBody::Newtype(reference) => Self::reference_uses_bytes(reference),
            SpecifiedDeclarationBody::Enum(variants) => variants.iter().any(|variant| {
                variant
                    .payload()
                    .is_some_and(|payload| Self::reference_uses_bytes(payload.reference()))
            }),
        }
    }

    fn reference_uses_bytes(reference: &TypeReference) -> bool {
        match reference {
            TypeReference::Bytes => true,
            TypeReference::Vector(inner)
            | TypeReference::Optional(inner)
            | TypeReference::ScopeOf(inner) => Self::reference_uses_bytes(inner),
            TypeReference::Map(key, value) => {
                Self::reference_uses_bytes(key) || Self::reference_uses_bytes(value)
            }
            TypeReference::Application { arguments, .. } => {
                arguments.iter().any(Self::reference_uses_bytes)
            }
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::FixedBytes(_)
            | TypeReference::Plain(_) => false,
        }
    }

    /// Whether the schema references a fixed-size `(Bytes N)` anywhere, so the
    /// renderer only emits the generic `FixedBytes<N>` newtype-prelude when used.
    fn references_fixed_bytes(&self) -> bool {
        self.schema
            .declarations()
            .iter()
            .any(|declaration| Self::declaration_uses_fixed_bytes(declaration.body()))
            || [self.schema.input(), self.schema.output()]
                .into_iter()
                .any(Self::root_uses_fixed_bytes)
    }

    fn root_uses_fixed_bytes(root: &SpecifiedRoot) -> bool {
        match root {
            SpecifiedRoot::Enum(declaration) => declaration.variants().iter().any(|variant| {
                variant
                    .payload()
                    .is_some_and(|payload| Self::reference_uses_fixed_bytes(payload.reference()))
            }),
            SpecifiedRoot::Application(application) => {
                Self::reference_uses_fixed_bytes(application.reference())
            }
        }
    }

    fn declaration_uses_fixed_bytes(declaration: &SpecifiedDeclarationBody) -> bool {
        match declaration {
            SpecifiedDeclarationBody::Struct(fields) => fields
                .iter()
                .any(|field| Self::reference_uses_fixed_bytes(field.reference())),
            SpecifiedDeclarationBody::Newtype(reference) => {
                Self::reference_uses_fixed_bytes(reference)
            }
            SpecifiedDeclarationBody::Enum(variants) => variants.iter().any(|variant| {
                variant
                    .payload()
                    .is_some_and(|payload| Self::reference_uses_fixed_bytes(payload.reference()))
            }),
        }
    }

    fn reference_uses_fixed_bytes(reference: &TypeReference) -> bool {
        match reference {
            TypeReference::FixedBytes(_) => true,
            TypeReference::Vector(inner)
            | TypeReference::Optional(inner)
            | TypeReference::ScopeOf(inner) => Self::reference_uses_fixed_bytes(inner),
            TypeReference::Map(key, value) => {
                Self::reference_uses_fixed_bytes(key) || Self::reference_uses_fixed_bytes(value)
            }
            TypeReference::Application { arguments, .. } => {
                arguments.iter().any(Self::reference_uses_fixed_bytes)
            }
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::Bytes
            | TypeReference::Plain(_) => false,
        }
    }

    fn collect_declaration_map_keys(
        declaration: &SpecifiedDeclarationBody,
        names: &mut Vec<String>,
    ) {
        match declaration {
            SpecifiedDeclarationBody::Struct(fields) => {
                for field in fields {
                    Self::collect_map_keys(field.reference(), names);
                }
            }
            SpecifiedDeclarationBody::Newtype(reference) => {
                Self::collect_map_keys(reference, names)
            }
            SpecifiedDeclarationBody::Enum(variants) => {
                for variant in variants {
                    if let Some(payload) = variant.payload() {
                        Self::collect_map_keys(payload.reference(), names);
                    }
                }
            }
        }
    }

    fn collect_map_keys(reference: &TypeReference, names: &mut Vec<String>) {
        match reference {
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::Bytes
            | TypeReference::FixedBytes(_)
            | TypeReference::Plain(_) => {}
            TypeReference::Vector(inner)
            | TypeReference::Optional(inner)
            | TypeReference::ScopeOf(inner) => {
                Self::collect_map_keys(inner, names);
            }
            TypeReference::Map(key, value) => {
                if let TypeReference::Plain(name) = key.as_ref() {
                    let name = name.as_str().to_owned();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                Self::collect_map_keys(key, names);
                Self::collect_map_keys(value, names);
            }
            TypeReference::Application { arguments, .. } => {
                for argument in arguments {
                    Self::collect_map_keys(argument, names);
                }
            }
        }
    }
}

/// The render driver for one generated module. It owns the emission
/// context (the map-key and private-type name sets, the NOTA surface, and
/// the emission target) and accumulates the pretty-printed source text as
/// each section's token-wrapper noun renders itself through
/// [`RustModuleRenderer::emit_item_tokens`]. It builds **no** Rust as
/// strings — the only direct text it writes is the leading `// @generated`
/// header comment, which cannot pass through `prettyplease` because that
/// drops non-doc comments. Its remaining `emits_*` / `*_root` / `trace_*`
/// methods are schema-analysis predicates that decide which sections emit;
/// the syntax of every section lives on its owning `*Tokens` noun.
struct RustModuleRenderer {
    output: String,
    map_key_types: Vec<String>,
    ordering_types: Vec<String>,
    private_type_names: Vec<String>,
    nota_surface: NotaSurface,
    target: RustEmissionTarget,
}

struct EnumConstructorPayload {
    argument_type: String,
    expression: String,
}

impl EnumConstructorPayload {
    fn new(argument_type: String, expression: String) -> Self {
        Self {
            argument_type,
            expression,
        }
    }

    fn argument_type(&self) -> &str {
        &self.argument_type
    }

    fn expression(&self) -> &str {
        &self.expression
    }
}

/// Renders `impl From<Payload> for Enum { fn from(payload) { Self::Variant(payload) } }`
/// for an enum variant whose payload type is unique within the enum. Owns
/// the enum name, the variant name, and the payload type name.
struct EnumPayloadFromImplTokens<'name> {
    enum_name: &'name Name,
    variant_name: &'name Name,
    payload: &'name str,
}

impl<'name> EnumPayloadFromImplTokens<'name> {
    fn new(enum_name: &'name Name, variant_name: &'name Name, payload: &'name str) -> Self {
        Self {
            enum_name,
            variant_name,
            payload,
        }
    }
}

impl ToTokens for EnumPayloadFromImplTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let enum_name = RustIdentifier::new(self.enum_name.as_str());
        let variant = RustIdentifier::new(self.variant_name.as_str());
        let payload = RustIdentifier::new(self.payload);
        quote! {
            impl From<#payload> for #enum_name {
                fn from(payload: #payload) -> Self {
                    Self::#variant(payload)
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// One generated variant constructor: the method name, the source variant
/// name, and the resolved payload (argument type + wrap expression).
struct EnumVariantConstructor {
    method_name: String,
    variant_name: String,
    payload: EnumConstructorPayload,
}

impl EnumVariantConstructor {
    fn new(method_name: String, variant_name: String, payload: EnumConstructorPayload) -> Self {
        Self {
            method_name,
            variant_name,
            payload,
        }
    }

    fn method_tokens(&self) -> TokenStream {
        let method_name = RustIdentifier::new(&self.method_name).ident();
        let variant = RustIdentifier::new(&self.variant_name);
        let argument_type = RustTypeTokens::new(self.payload.argument_type());
        let expression = syn::parse_str::<syn::Expr>(self.payload.expression())
            .expect("generated constructor expression parses");
        quote! {
            pub fn #method_name(payload: #argument_type) -> Self {
                Self::#variant(#expression)
            }
        }
    }
}

/// Renders the `impl Enum { pub fn variant(payload) -> Self { ... } ... }`
/// block carrying one associated constructor per payload-bearing variant.
struct EnumVariantConstructorsTokens<'data> {
    enum_name: &'data Name,
    constructors: &'data [EnumVariantConstructor],
}

impl<'data> EnumVariantConstructorsTokens<'data> {
    fn new(enum_name: &'data Name, constructors: &'data [EnumVariantConstructor]) -> Self {
        Self {
            enum_name,
            constructors,
        }
    }
}

impl ToTokens for EnumVariantConstructorsTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let enum_name = RustIdentifier::new(self.enum_name.as_str());
        let methods = self
            .constructors
            .iter()
            .map(EnumVariantConstructor::method_tokens);
        quote! {
            impl #enum_name {
                #(#methods)*
            }
        }
        .to_tokens(tokens);
    }
}

struct SplitSemaProjection<'schema> {
    signal_input: &'schema RustEnum,
    signal_output: &'schema RustEnum,
    nexus_work: &'schema RustEnum,
    nexus_action: &'schema RustEnum,
    sema_write_input: &'schema RustEnum,
    sema_write_output: &'schema RustEnum,
    sema_read_input: &'schema RustEnum,
    sema_read_output: &'schema RustEnum,
}

struct NexusRunnerShape {
    reply_type: String,
    sema_write_input_type: Option<String>,
    sema_write_output_type: Option<String>,
    sema_read_input_type: Option<String>,
    sema_read_output_type: Option<String>,
    effect_command_type: Option<String>,
    effect_result_type: Option<String>,
    has_continue: bool,
}

struct RuntimeRoleTraitImpl {
    type_name: String,
    trait_name: &'static str,
    canonical_type_name: String,
}

impl NexusRunnerShape {
    fn sema_write_input_type(&self) -> &str {
        self.sema_write_input_type
            .as_deref()
            .unwrap_or("std::convert::Infallible")
    }

    fn sema_read_input_type(&self) -> &str {
        self.sema_read_input_type
            .as_deref()
            .unwrap_or("std::convert::Infallible")
    }

    fn effect_command_type(&self) -> &str {
        self.effect_command_type
            .as_deref()
            .unwrap_or("std::convert::Infallible")
    }

    fn emits_sema_write(&self) -> bool {
        self.sema_write_input_type.is_some()
    }

    fn emits_sema_read(&self) -> bool {
        self.sema_read_input_type.is_some()
    }

    fn emits_effect(&self) -> bool {
        self.effect_command_type.is_some()
    }
}

impl RuntimeRoleTraitImpl {
    fn new(type_name: String, trait_name: &'static str, canonical_type_name: String) -> Self {
        Self {
            type_name,
            trait_name,
            canonical_type_name,
        }
    }

    fn matches(&self, type_name: &str, trait_name: &'static str) -> bool {
        self.canonical_type_name == type_name && self.trait_name == trait_name
    }
}

struct TraceInterfaceRoot<'schema> {
    object_variant: &'static str,
    name_prefix: &'static str,
    type_name: &'schema Name,
    enum_declaration: &'schema RustEnum,
}

impl RustModuleRenderer {
    fn new(options: RustEmissionOptions) -> Self {
        Self {
            output: String::new(),
            map_key_types: Vec::new(),
            ordering_types: Vec::new(),
            private_type_names: Vec::new(),
            nota_surface: options.nota_surface().clone(),
            target: options.target(),
        }
    }

    fn nota_surface(&self) -> &NotaSurface {
        &self.nota_surface
    }

    fn emits_runtime_support(&self) -> bool {
        self.target.emits_runtime_support()
    }

    fn emits_wire_frame(&self) -> bool {
        self.target.emits_wire_frame()
    }

    fn emits_short_headers(&self) -> bool {
        self.emits_wire_frame()
    }

    fn emits_root_enums(&self) -> bool {
        self.target.emits_root_enums()
    }

    fn emitted_root_enums<'root>(&self, root_enums: &'root [RustEnum]) -> &'root [RustEnum] {
        if self.emits_root_enums() {
            root_enums
        } else {
            &[]
        }
    }

    fn runtime_planes(&self) -> RuntimePlaneSet {
        self.target.runtime_planes()
    }

    fn render_context(&self) -> RustRenderContext {
        RustRenderContext::new(
            self.map_key_types.clone(),
            self.ordering_types.clone(),
            self.private_type_names.clone(),
            self.nota_surface.clone(),
        )
    }

    fn emits_all_runtime_planes(&self) -> bool {
        self.runtime_planes().emits_all()
    }

    /// Record the set of type names used as a `BTreeMap` key anywhere
    /// in the schema. A map key type additionally derives `PartialOrd,
    /// Ord` (and the archived form does too) so `BTreeMap<Key, _>`
    /// compiles. Non-key types keep the original derive set, so a
    /// collection-free schema's emission stays byte-identical.
    fn note_map_key_types(&mut self, key_types: Vec<String>) {
        self.map_key_types = key_types;
    }

    /// Record the type names the `{| … |}` catalog references an ordering-class
    /// trait (`Ord`/`PartialOrd`) for. Like a map-key type, an ordering-class
    /// type additionally derives `PartialOrd, Ord`; the derive *is* the body for
    /// an ordering marker, so the catalog entry is satisfied by it and the
    /// emitted surface attests the trait.
    fn note_ordering_types(&mut self, ordering_types: Vec<String>) {
        self.ordering_types = ordering_types;
    }

    fn note_private_type_names(&mut self, names: Vec<String>) {
        self.private_type_names = names;
    }

    fn line(&mut self, line: impl AsRef<str>) {
        self.output.push_str(line.as_ref());
        self.output.push('\n');
    }

    fn blank(&mut self) {
        if self.output.ends_with("\n\n") {
            return;
        }
        self.output.push('\n');
    }

    fn finish(self) -> String {
        let mut output = self.output.trim_end().to_owned();
        output.push('\n');
        output
    }

    fn emit_item_tokens(&mut self, tokens: TokenStream) {
        let file = syn::parse2::<syn::File>(tokens).expect("generated Rust item tokens parse");
        self.output
            .push_str(&RustfmtSkippedItems::new(file).render());
    }

    /// Emit a `pub use` alias for each cross-crate import.
    ///
    /// The dependency crate emits its own definition of the type; the
    /// consumer references that type through the local alias instead of
    /// re-declaring it. Later fields or variants that name the imported
    /// type therefore use the dependency crate's type identity.
    fn emit_scalar_alias(&mut self, alias: &RustScalarAlias) {
        self.emit_item_tokens(RustScalarAliasTokens::new(alias).into_token_stream());
    }

    /// Emits the `Bytes` scalar as a local storage newtype over `Vec<u8>`.
    /// The inner value is nota's byte scalar, so the generated type can
    /// derive NOTA codecs instead of emitting a local implementation.
    fn emit_bytes_scalar(&mut self) {
        let nota_gate = match &self.nota_surface {
            NotaSurface::AlwaysEnabled | NotaSurface::Disabled => quote! {},
            NotaSurface::FeatureGated { feature } => quote! {
                #[cfg_attr(feature = #feature, derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode))]
            },
        };
        let nota_derives = if self.nota_surface.includes_nota_in_derive() {
            quote! { nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode, }
        } else {
            quote! {}
        };
        self.emit_item_tokens(quote! {
            #nota_gate
            #[derive(
                #nota_derives
                rkyv::Archive,
                rkyv::Serialize,
                rkyv::Deserialize,
                Clone,
                Debug,
                PartialEq,
                Eq,
                PartialOrd,
                Ord,
                Hash,
            )]
            pub struct Bytes(nota::ByteSequence);

            impl Bytes {
                pub fn new(payload: Vec<u8>) -> Self {
                    Self(nota::ByteSequence::new(payload))
                }

                pub fn payload(&self) -> &[u8] {
                    self.0.payload()
                }

                pub fn into_payload(self) -> Vec<u8> {
                    self.0.into_payload()
                }
            }
        });
    }

    /// Emits the generic fixed-size `FixedBytes<const WIDTH: usize>([u8; WIDTH])`
    /// that `(Bytes N)` references lower to (`FixedBytes<N>`). One generic type
    /// serves every width; the inner value is nota's fixed byte scalar, so
    /// the generated type can derive NOTA codecs instead of emitting a local
    /// implementation.
    fn emit_fixed_bytes_scalar(&mut self) {
        let nota_gate = match &self.nota_surface {
            NotaSurface::AlwaysEnabled | NotaSurface::Disabled => quote! {},
            NotaSurface::FeatureGated { feature } => quote! {
                #[cfg_attr(feature = #feature, derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode))]
            },
        };
        let nota_derives = if self.nota_surface.includes_nota_in_derive() {
            quote! { nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode, }
        } else {
            quote! {}
        };
        self.emit_item_tokens(quote! {
            #nota_gate
            #[derive(
                #nota_derives
                rkyv::Archive,
                rkyv::Serialize,
                rkyv::Deserialize,
                Clone,
                Debug,
                PartialEq,
                Eq,
                PartialOrd,
                Ord,
                Hash,
            )]
            pub struct FixedBytes<const WIDTH: usize>(nota::FixedByteSequence<WIDTH>);

            impl<const WIDTH: usize> FixedBytes<WIDTH> {
                pub fn new(payload: [u8; WIDTH]) -> Self {
                    Self(nota::FixedByteSequence::new(payload))
                }

                pub fn payload(&self) -> &[u8; WIDTH] {
                    self.0.payload()
                }

                pub fn into_payload(self) -> [u8; WIDTH] {
                    self.0.into_payload()
                }
            }
        });
    }

    fn emit_imports(&mut self, imports: &[RustImport]) {
        if imports.is_empty() {
            return;
        }
        for import in imports {
            self.emit_item_tokens(RustImportTokens::new(import).into_token_stream());
        }
        self.blank();
    }

    fn emit_type(&mut self, declaration: &RustDeclaration, declarations: &[RustDeclaration]) {
        let context = self.render_context();
        if let RustTypeDeclaration::Newtype(newtype) = declaration.value()
            && newtype.is_scope_of()
        {
            self.emit_item_tokens(
                ScopeFamilyTokens::new(newtype, declarations, declaration.visibility(), &context)
                    .into_token_stream(),
            );
            return;
        }
        self.emit_item_tokens(
            RustDeclarationTokens::new(declaration, &context).into_token_stream(),
        );
    }

    fn emit_nota_support(&mut self) {
        if !self.nota_surface.emits_nota() {
            return;
        }
        let context = self.render_context();
        let gate = context.nota_feature_gate();
        self.emit_item_tokens(quote! {
            #gate
            pub use nota::{
                NotaDecodeError, NotaEncode, NotaSource,
            };
        });
    }

    fn emit_newtype_inherent_impls(&mut self, declarations: &[RustDeclaration]) {
        let newtypes: Vec<_> = declarations
            .iter()
            .filter_map(|declaration| match declaration.value() {
                RustTypeDeclaration::Newtype(value) if !value.is_scope_of() => Some(value),
                _ => None,
            })
            .collect();
        for newtype in newtypes {
            self.emit_newtype_inherent_impl(newtype);
            self.blank();
        }
    }

    fn emit_newtype_inherent_impl(&mut self, declaration: &RustNewtype) {
        // The intrinsic Bucket-1 surface (`new` / `payload` / `into_payload` /
        // `From`) is unconditional — it is what being a newtype MEANS, not a
        // catalog entry. Standard payload-delegating impls (Display, AsRef,
        // scalar comparisons) are no longer emitted here on a flag; they are
        // driven by the `{| … |}` catalog through `emit_catalog_impls`.
        self.emit_item_tokens(NewtypeInherentImplTokens::new(declaration).into_token_stream());
    }

    /// Drive standard-impl emission from the `{| … |}` catalog rather than
    /// `scalar_like()` shape inference. For each referenced impl, resolve the
    /// target's backing scalar shape, build a [`StandardImplRecipe`], and:
    /// `Some(body)` → emit the body the generator owns; derive-class (`Ord`) →
    /// already folded into the derive set by `note_ordering_types`, emit no
    /// body; unrecognized trait / hand-written method → emit nothing (the
    /// crate is trusted to provide it; the verify loop is the trust boundary).
    fn emit_catalog_impls(
        &mut self,
        referenced_impls: &[RustImplReference],
        declarations: &[RustDeclaration],
    ) {
        for reference in referenced_impls {
            let Some(trait_name) = reference.entry().trait_name() else {
                // An inherent-method reference names no trait; the generator
                // owns no body recipe for it, so it is verify-only.
                continue;
            };
            let shape = match declarations
                .iter()
                .find(|declaration| declaration.name() == reference.target())
                .map(RustDeclaration::value)
            {
                Some(RustTypeDeclaration::Newtype(newtype)) => {
                    ScalarShape::resolve(newtype.reference(), declarations)
                }
                _ => ScalarShape::NonScalar,
            };
            let recipe =
                StandardImplRecipe::new(reference.target().clone(), trait_name.clone(), shape);
            if let Some(body) = recipe.recipe() {
                self.emit_item_tokens(body.into_token_stream());
                self.blank();
            }
        }
    }

    fn emit_root_enum(&mut self, root_enum: &RustEnum) {
        let context = self.render_context();
        self.emit_item_tokens(RustEnumTokens::root(root_enum, &context).into_token_stream());
    }

    fn emit_applied_root(&mut self, applied_root: &RustAppliedRoot) {
        self.emit_item_tokens(AppliedRootTokens::new(applied_root).into_token_stream());
    }

    fn emit_enum_payload_from_impls(
        &mut self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) {
        for declaration in declarations {
            if let RustTypeDeclaration::Enum(value) = declaration.value() {
                self.emit_enum_payload_from_impls_for(value);
            }
        }
        for root_enum in root_enums {
            self.emit_enum_payload_from_impls_for(root_enum);
        }
    }

    fn emit_enum_payload_from_impls_for(&mut self, declaration: &RustEnum) -> bool {
        // A parameterized frame enum's variant payloads are generic binders,
        // not concrete payload types, so a `From<Binder> for Work` impl is
        // meaningless and would not name its generics. Skip it — the frame's
        // conversions are hand-written in the runtime crate.
        if !declaration.parameters().is_empty() {
            return false;
        }
        let mut emitted = false;
        for variant in self.unique_plain_payload_variants(declaration) {
            let Some(payload) = self.plain_payload_name(variant) else {
                continue;
            };
            self.emit_item_tokens(
                EnumPayloadFromImplTokens::new(declaration.name(), variant.name(), payload)
                    .into_token_stream(),
            );
            self.blank();
            emitted = true;
        }
        emitted
    }

    fn emit_enum_variant_constructors(
        &mut self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) {
        let newtypes: Vec<_> = declarations
            .iter()
            .filter_map(|declaration| match declaration.value() {
                RustTypeDeclaration::Newtype(value) if !value.is_scope_of() => Some(value),
                _ => None,
            })
            .collect();
        for declaration in declarations {
            if let RustTypeDeclaration::Enum(value) = declaration.value() {
                self.emit_enum_variant_constructors_for(value, &newtypes);
            }
        }
        for root_enum in root_enums {
            self.emit_enum_variant_constructors_for(root_enum, &newtypes);
        }
    }

    fn emit_enum_variant_constructors_for(
        &mut self,
        declaration: &RustEnum,
        newtypes: &[&RustNewtype],
    ) {
        // A parameterized frame enum (`Work<Event, …>`) carries its
        // construction behaviour by hand in the runtime crate, not as
        // schema-emitted inherent constructors: an emitted `impl Work { … }`
        // would reference the undeclared generic binders. Schema emits the
        // generic DATA enum only.
        if !declaration.parameters().is_empty() {
            return;
        }
        let payload_variants: Vec<_> = declaration
            .variants()
            .iter()
            .filter(|variant| variant.payload().is_some())
            .collect();
        if payload_variants.is_empty() {
            return;
        }
        let constructors: Vec<EnumVariantConstructor> = payload_variants
            .iter()
            .filter_map(|variant| {
                let payload = variant.payload()?;
                let constructor = self.enum_variant_constructor_payload(payload, newtypes);
                Some(EnumVariantConstructor::new(
                    variant.name().field_name(),
                    variant.name().as_str().to_owned(),
                    constructor,
                ))
            })
            .collect();
        self.emit_item_tokens(
            EnumVariantConstructorsTokens::new(declaration.name(), &constructors)
                .into_token_stream(),
        );
        self.blank();
    }

    fn emit_domain_scope_relation_support(
        &mut self,
        relations: &[RustRelation],
        declarations: &[RustDeclaration],
    ) {
        if relations.is_empty() {
            return;
        }
        let Some((root, models)) = declarations
            .iter()
            .filter_map(|declaration| match declaration.value() {
                RustTypeDeclaration::Newtype(value) if value.is_scope_of() => Some(value),
                _ => None,
            })
            .find_map(|newtype| {
                let models = ScopeEnumModel::from_scope_newtype(newtype, declarations)?;
                let root = models.iter().find(|model| model.root)?.clone();
                Some((root, models))
            })
        else {
            return;
        };
        self.emit_item_tokens(
            DomainScopeRelationSupportTokens::new(relations, &root, &models).into_token_stream(),
        );
        self.blank();
    }

    /// Emit the record-family version-control surface for a schema that
    /// declares families: the `family_identity` constant module pinning
    /// each family's generation-time closure hash, the typed
    /// `RecordFamilyError` enum, and the closed `RecordFamily` sum
    /// carrying the store name, `versioning_policy`, the per-family
    /// `sema_engine` table-descriptor constructors, and the `decode`
    /// dispatch. Generated paths reference the real `sema_engine` crate
    /// on the signal-frame precedent, so a consumer that declares
    /// families depends on `sema-engine`.
    fn emit_record_family_support(&mut self, store: &RustVersionedStore) {
        self.emit_item_tokens(
            FamilyIdentityModuleTokens::new(store.families()).into_token_stream(),
        );
        self.blank();
        self.emit_item_tokens(quote! {
            #[derive(Clone, Debug, PartialEq, Eq)]
            pub enum RecordFamilyError {
                UnknownFamily { family: sema_engine::FamilyName },
                SchemaHashMismatch {
                    family: sema_engine::FamilyName,
                    stored: sema_engine::SchemaHash,
                    generated: sema_engine::SchemaHash,
                },
                RecordDecode { family: sema_engine::FamilyName },
            }
            impl std::fmt::Display for RecordFamilyError {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        Self::UnknownFamily { family } => write!(formatter, "unknown record family {family}"),
                        Self::SchemaHashMismatch { family, stored, generated } => write!(
                            formatter,
                            "schema hash mismatch for record family {family}: stored {stored}, generated {generated}",
                        ),
                        Self::RecordDecode { family } => write!(formatter, "failed to decode {family} record archive"),
                    }
                }
            }
            impl std::error::Error for RecordFamilyError {}
        });
        self.blank();
        self.emit_item_tokens(RecordFamilyEnumTokens::new(store).into_token_stream());
        self.blank();
    }

    fn enum_variant_constructor_payload(
        &self,
        payload: &TypeReference,
        newtypes: &[&RustNewtype],
    ) -> EnumConstructorPayload {
        match payload {
            TypeReference::Plain(name) => newtypes
                .iter()
                .find(|newtype| newtype.name() == name)
                .map(|newtype| {
                    EnumConstructorPayload::new(
                        self.rust_type(newtype.reference()),
                        format!("{}::new(payload)", newtype.name()),
                    )
                })
                .unwrap_or_else(|| {
                    EnumConstructorPayload::new(self.rust_type(payload), "payload".to_owned())
                }),
            _ => EnumConstructorPayload::new(self.rust_type(payload), "payload".to_owned()),
        }
    }

    fn unique_plain_payload_variants<'declaration>(
        &self,
        declaration: &'declaration RustEnum,
    ) -> Vec<&'declaration RustEnumVariant> {
        declaration
            .variants()
            .iter()
            .filter(|variant| {
                let Some(payload) = self.plain_payload_name(variant) else {
                    return false;
                };
                declaration
                    .variants()
                    .iter()
                    .filter(|other| self.plain_payload_name(other) == Some(payload))
                    .count()
                    == 1
            })
            .collect()
    }

    fn plain_payload_name<'variant>(
        &self,
        variant: &'variant RustEnumVariant,
    ) -> Option<&'variant str> {
        match variant.payload() {
            Some(TypeReference::Plain(name)) => Some(name.as_str()),
            _ => None,
        }
    }

    fn emit_nota_root_enum_support(&mut self, root_enum: &RustEnum) {
        if !self.nota_surface.emits_nota() {
            return;
        }
        let context = self.render_context();
        self.emit_item_tokens(
            NotaRootEnumStringSupportTokens::new(root_enum.name().as_str(), &context)
                .into_token_stream(),
        );
    }

    fn emit_short_headers(&mut self, root_enums: &[RustEnum]) {
        self.emit_item_tokens(ShortHeaderModuleTokens::new(root_enums).into_token_stream());
    }

    /// Emit the basic signal-frame codec: the short-header byte count,
    /// the [`SignalFrameError`] type, the per-root route enums, and the
    /// per-root frame impls (`route` / `short_header` /
    /// `route_from_short_header` / `encode_signal_frame` /
    /// `decode_signal_frame`).
    ///
    /// This is the wire framing every wire-facing target needs — a
    /// separately-generated `WireContract` crate IS the framing that peers
    /// and the owning daemon import and call the codec on. It is gated by
    /// [`RustModuleRenderer::emits_wire_frame`] in [`RustModule::render`], so it
    /// reaches `WireContract`, `SignalRuntime`, and `ComponentRuntime` but
    /// never the internal `NexusRuntime` / `SemaRuntime` planes. The
    /// streaming / observable surface is gated separately by
    /// [`RustModuleRenderer::emits_signal`] plus a declared stream — see
    /// [`RustModuleRenderer::emit_signal_frame_streaming_support`].
    fn emit_signal_frame_codec(&mut self, root_enums: &[RustEnum]) {
        self.emit_item_tokens(quote! {
            const SIGNAL_SHORT_HEADER_BYTE_COUNT: usize = 8;
            #[derive(Clone, Debug, PartialEq, Eq)]
            pub enum SignalFrameError {
                ArchiveEncode,
                ArchiveDecode,
                FrameTooShort { found: usize },
                UnknownHeader { root_enum: &'static str, header: u64 },
                HeaderMismatch { expected: u64, found: u64 },
            }
            impl std::fmt::Display for SignalFrameError {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        Self::ArchiveEncode => formatter.write_str("failed to encode rkyv archive"),
                        Self::ArchiveDecode => formatter.write_str("failed to decode rkyv archive"),
                        Self::FrameTooShort { found } => write!(formatter, "signal frame too short: {found} bytes"),
                        Self::UnknownHeader { root_enum, header } => write!(formatter, "unknown {root_enum} short header 0x{header:016X}"),
                        Self::HeaderMismatch { expected, found } => write!(formatter, "decoded payload header mismatch: expected 0x{expected:016X}, found 0x{found:016X}"),
                    }
                }
            }
            impl std::error::Error for SignalFrameError {}
        });
        self.blank();

        for root_enum in root_enums {
            self.emit_route_enum(root_enum);
            self.blank();
        }

        for root_enum in root_enums {
            self.emit_signal_frame_impl(root_enum);
            self.blank();
        }
    }

    fn emit_route_enum(&mut self, root_enum: &RustEnum) {
        let context = self.render_context();
        self.emit_item_tokens(RouteEnumTokens::new(root_enum, &context).into_token_stream());
    }

    fn emit_plane_route_support(&mut self, declarations: &[RustDeclaration]) {
        for declaration in self.plane_route_enums(declarations) {
            self.emit_route_enum(declaration);
            self.blank();
            self.emit_route_impl(declaration);
            self.blank();
        }
    }

    fn emit_route_impl(&mut self, declaration: &RustEnum) {
        self.emit_item_tokens(RouteImplTokens::new(declaration).into_token_stream());
    }

    fn emit_signal_frame_impl(&mut self, root_enum: &RustEnum) {
        self.emit_item_tokens(SignalFrameImplTokens::new(root_enum).into_token_stream());
    }

    fn emit_signal_frame_transport_support(
        &mut self,
        root_enums: &[RustEnum],
        event_payload: Option<&TypeReference>,
    ) {
        let Some(input) = self.root_enum_named(root_enums, "Input") else {
            return;
        };
        if self.root_enum_named(root_enums, "Output").is_none() {
            return;
        }
        self.emit_item_tokens(
            SignalFrameTransportSupportTokens::new(input, event_payload).into_token_stream(),
        );
    }

    fn streaming_event_payload<'schema>(
        &self,
        root_enums: &'schema [RustEnum],
        streams: &'schema [StreamDeclaration],
    ) -> Option<&'schema TypeReference> {
        let stream = streams.first()?;
        let output = self.root_enum_named(root_enums, "Output")?;
        let output_event_payload = output
            .variants()
            .iter()
            .find(|variant| variant.name().as_str() == "Event")
            .and_then(RustEnumVariant::payload)?;
        if &stream.event == output_event_payload {
            Some(output_event_payload)
        } else {
            None
        }
    }

    fn emit_signal_frame_streaming_support(&mut self, event_payload: &TypeReference) {
        self.emit_item_tokens(
            SignalFrameStreamingSupportTokens::new(event_payload).into_token_stream(),
        );
    }

    fn emit_trace_support(&mut self, declarations: &[RustDeclaration], root_enums: &[RustEnum]) {
        let signal_roots = if self.runtime_planes().emits_signal() {
            self.trace_signal_roots(root_enums)
        } else {
            Vec::new()
        };
        let nexus_roots = if self.runtime_planes().emits_nexus() {
            self.trace_nexus_roots(declarations)
        } else {
            Vec::new()
        };
        let sema_roots = if self.runtime_planes().emits_sema() {
            self.trace_sema_roots(declarations, root_enums)
        } else {
            Vec::new()
        };
        let signal_actor_variants = self.trace_signal_actor_variants(declarations, root_enums);
        let nexus_actor_variants = self.trace_nexus_actor_variants(declarations);
        let sema_actor_variants = self.trace_sema_actor_variants(declarations, root_enums);
        let has_signal = !signal_roots.is_empty() || !signal_actor_variants.is_empty();
        let has_nexus = !nexus_roots.is_empty() || !nexus_actor_variants.is_empty();
        let has_sema = !sema_roots.is_empty() || !sema_actor_variants.is_empty();
        if !has_signal && !has_nexus && !has_sema {
            return;
        }
        let mut planes = Vec::new();
        let context = self.render_context();
        if has_signal {
            planes.push(Plane::Signal);
        }
        self.emit_object_name_enum(
            Plane::Signal,
            &signal_roots,
            &signal_actor_variants,
            &context,
        );
        if has_nexus {
            planes.push(Plane::Nexus);
        }
        self.emit_object_name_enum(Plane::Nexus, &nexus_roots, &nexus_actor_variants, &context);
        if has_sema {
            planes.push(Plane::Sema);
        }
        self.emit_object_name_enum(Plane::Sema, &sema_roots, &sema_actor_variants, &context);
        self.emit_item_tokens(TraceSupportTokens::new(planes, &context).into_token_stream());
        self.blank();
    }

    fn emit_object_name_enum(
        &mut self,
        plane: Plane,
        interface_roots: &[TraceInterfaceRoot<'_>],
        actor_variants: &[&'static str],
        context: &RustRenderContext,
    ) {
        if interface_roots.is_empty() && actor_variants.is_empty() {
            return;
        }
        self.emit_item_tokens(
            TraceObjectNameEnumTokens::new(plane, interface_roots, actor_variants, context)
                .into_token_stream(),
        );
        self.blank();
    }

    fn plane_route_enums<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
    ) -> Vec<&'schema RustEnum> {
        declarations
            .iter()
            .filter_map(|declaration| match declaration.value() {
                RustTypeDeclaration::Enum(value)
                    if self.emits_plane_route_type(declaration.name().as_str()) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .collect()
    }

    fn trace_signal_roots<'schema>(
        &self,
        root_enums: &'schema [RustEnum],
    ) -> Vec<TraceInterfaceRoot<'schema>> {
        let mut roots = Vec::new();
        if let Some(input) = self.root_enum_named(root_enums, "Input") {
            roots.push(TraceInterfaceRoot {
                object_variant: "Input",
                name_prefix: "SignalInput",
                type_name: input.name(),
                enum_declaration: input,
            });
        }
        if let Some(output) = self.root_enum_named(root_enums, "Output") {
            roots.push(TraceInterfaceRoot {
                object_variant: "Output",
                name_prefix: "SignalOutput",
                type_name: output.name(),
                enum_declaration: output,
            });
        }
        roots
    }

    fn trace_nexus_roots<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
    ) -> Vec<TraceInterfaceRoot<'schema>> {
        let mut roots = Vec::new();
        if let Some(input) = self.declaration_enum_named(declarations, "NexusWork") {
            roots.push(TraceInterfaceRoot {
                object_variant: "Work",
                name_prefix: "NexusWork",
                type_name: input.name(),
                enum_declaration: input,
            });
        }
        if let Some(output) = self.declaration_enum_named(declarations, "NexusAction") {
            roots.push(TraceInterfaceRoot {
                object_variant: "Action",
                name_prefix: "NexusAction",
                type_name: output.name(),
                enum_declaration: output,
            });
        }
        roots
    }

    fn trace_sema_roots<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
        root_enums: &'schema [RustEnum],
    ) -> Vec<TraceInterfaceRoot<'schema>> {
        let mut roots = Vec::new();
        for root in [
            self.sema_write_input_root(declarations, root_enums)
                .map(|root| (root, "WriteInput", "SemaWriteInput")),
            self.sema_read_input_root(declarations, root_enums)
                .map(|root| (root, "ReadInput", "SemaReadInput")),
            self.sema_write_output_root(declarations, root_enums)
                .map(|root| (root, "WriteOutput", "SemaWriteOutput")),
            self.sema_read_output_root(declarations, root_enums)
                .map(|root| (root, "ReadOutput", "SemaReadOutput")),
        ]
        .into_iter()
        .flatten()
        {
            let (declaration, object_variant, name_prefix) = root;
            roots.push(TraceInterfaceRoot {
                object_variant,
                name_prefix,
                type_name: declaration.name(),
                enum_declaration: declaration,
            });
        }
        roots
    }

    fn trace_signal_actor_variants(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> Vec<&'static str> {
        let mut variants = Vec::new();
        let has_signal_roots =
            self.has_root_enum(root_enums, "Input") && self.has_root_enum(root_enums, "Output");
        let has_concrete_nexus =
            self.has_type(declarations, "NexusWork") && self.has_type(declarations, "NexusAction");
        if has_signal_roots
            && (matches!(self.target, RustEmissionTarget::SignalRuntime)
                || (self.runtime_planes().emits_signal() && has_concrete_nexus))
        {
            variants.extend([
                "Started", "Stopped", "Admitted", "Rejected", "Triaged", "Replied",
            ]);
        }
        variants
    }

    fn trace_nexus_actor_variants(&self, declarations: &[RustDeclaration]) -> Vec<&'static str> {
        let mut variants = Vec::new();
        if self.runtime_planes().emits_nexus()
            && self.has_type(declarations, "NexusWork")
            && self.has_type(declarations, "NexusAction")
        {
            variants.extend(["Started", "Stopped", "Entered", "Decided"]);
        }
        variants
    }

    fn trace_sema_actor_variants(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> Vec<&'static str> {
        let mut variants = Vec::new();
        if !self.runtime_planes().emits_sema() {
            return variants;
        }
        let has_write = self.has_sema_write_roots(declarations, root_enums);
        let has_read = self.has_sema_read_roots(declarations, root_enums);
        if has_write || has_read {
            variants.extend(["Started", "Stopped"]);
        }
        if has_write {
            variants.push("WriteApplied");
        }
        if has_read {
            variants.push("ReadObserved");
        }
        variants
    }

    fn is_plane_route_type(&self, type_name: &str) -> bool {
        matches!(
            type_name,
            "NexusWork"
                | "NexusAction"
                | "WriteInput"
                | "WriteOutput"
                | "ReadInput"
                | "ReadOutput"
                | "SemaWriteInput"
                | "SemaWriteOutput"
                | "SemaReadInput"
                | "SemaReadOutput"
        )
    }

    fn emits_plane_route_type(&self, type_name: &str) -> bool {
        self.is_plane_route_type(type_name)
            && ((self.runtime_planes().emits_nexus()
                && matches!(type_name, "NexusWork" | "NexusAction"))
                || (self.runtime_planes().emits_sema()
                    && matches!(
                        type_name,
                        "WriteInput"
                            | "WriteOutput"
                            | "ReadInput"
                            | "ReadOutput"
                            | "SemaWriteInput"
                            | "SemaWriteOutput"
                            | "SemaReadInput"
                            | "SemaReadOutput"
                    )))
    }

    fn emit_mail_event_support(&mut self, root_enums: &[RustEnum]) {
        let context = self.render_context();
        if self.runtime_planes().emits_signal() {
            self.emit_item_tokens(
                RuntimeCopyNewtypeTokens::new("MessageIdentifier", &context).into_token_stream(),
            );
            self.blank();
        }
        self.emit_item_tokens(
            RuntimeCopyNewtypeTokens::new("OriginRoute", &context).into_token_stream(),
        );
        self.blank();
        if self.emits_all_runtime_planes() {
            self.emit_signal_message_root_support(root_enums);
            self.blank();
            self.emit_schema_plane_support();
            self.blank();
        }
        if self.runtime_planes().emits_signal() {
            self.emit_plane_envelope("Signal");
            self.blank();
        }
        if self.runtime_planes().emits_nexus() {
            self.emit_plane_envelope("Nexus");
            self.blank();
        }
        if self.runtime_planes().emits_sema() {
            self.emit_plane_envelope("Sema");
            self.blank();
        }
        if self.runtime_planes().emits_signal() {
            if !self.emits_all_runtime_planes() {
                self.emit_signal_message_root_support(root_enums);
                self.blank();
            }
            self.emit_signal_mail_lifecycle_support(root_enums);
        }
    }

    fn emit_signal_message_root_support(&mut self, root_enums: &[RustEnum]) {
        let context = self.render_context();
        self.emit_item_tokens(MessageRootTokens::new(root_enums, &context).into_token_stream());
    }

    fn emit_signal_mail_lifecycle_support(&mut self, root_enums: &[RustEnum]) {
        self.emit_item_tokens(
            SignalMailLifecycleSupportTokens::new(root_enums).into_token_stream(),
        );
        self.blank();
    }

    fn emit_schema_plane_support(&mut self) {
        self.emit_item_tokens(SchemaPlaneSupportTokens::new().into_token_stream());
    }

    fn emit_plane_envelope(&mut self, name: &str) {
        self.emit_item_tokens(PlaneEnvelopeTokens::new(name).into_token_stream());
    }

    fn emit_plane_namespaces(&mut self, declarations: &[RustDeclaration], root_enums: &[RustEnum]) {
        let active_planes = self.runtime_planes().active_planes();
        for plane in &active_planes {
            let aliases = self.plane_namespace_aliases(*plane, declarations, root_enums);
            if aliases.is_empty() {
                continue;
            }
            self.emit_item_tokens(PlaneNamespaceTokens::new(*plane, aliases).into_token_stream());
            self.blank();
        }
        for plane in &active_planes {
            let source_type_names =
                self.plane_origin_route_source_type_names(*plane, declarations, root_enums);
            for source_type_name in source_type_names {
                self.emit_item_tokens(
                    PlaneOriginRouteConstructorTokens::new(*plane, source_type_name)
                        .into_token_stream(),
                );
                self.blank();
            }
        }
    }

    fn plane_namespace_aliases<'declaration>(
        &self,
        plane: Plane,
        declarations: &'declaration [RustDeclaration],
        root_enums: &'declaration [RustEnum],
    ) -> Vec<PlaneNamespaceAlias<'declaration>> {
        match plane {
            Plane::Signal => plane
                .alias_names()
                .iter()
                .zip(plane.canonical_source_type_names())
                .filter(|(_, source)| self.has_root_enum(root_enums, source))
                .map(|(export, source)| PlaneNamespaceAlias::new(export, source))
                .collect(),
            Plane::Nexus => plane
                .alias_names()
                .iter()
                .zip(plane.canonical_source_type_names())
                .filter(|(_, source)| self.has_type(declarations, source))
                .map(|(export, source)| PlaneNamespaceAlias::new(export, source))
                .collect(),
            Plane::Sema => plane
                .alias_names()
                .iter()
                .zip(self.sema_source_type_names(declarations, root_enums))
                .filter_map(|(export, source)| {
                    source.map(|source| PlaneNamespaceAlias::new(export, source))
                })
                .collect(),
        }
    }

    fn plane_origin_route_source_type_names<'declaration>(
        &self,
        plane: Plane,
        declarations: &'declaration [RustDeclaration],
        root_enums: &'declaration [RustEnum],
    ) -> Vec<&'declaration str> {
        match plane {
            Plane::Signal => Vec::new(),
            Plane::Nexus => plane
                .canonical_source_type_names()
                .iter()
                .copied()
                .filter(|source| self.has_type(declarations, source))
                .collect(),
            Plane::Sema => self
                .sema_source_type_names(declarations, root_enums)
                .into_iter()
                .flatten()
                .collect(),
        }
    }

    fn sema_source_type_names(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> [Option<&'static str>; 4] {
        [
            self.sema_write_input_type_name(declarations, root_enums),
            self.sema_write_output_type_name(declarations, root_enums),
            self.sema_read_input_type_name(declarations, root_enums),
            self.sema_read_output_type_name(declarations, root_enums),
        ]
    }

    fn emit_plane_projection_support(
        &mut self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) {
        if !self.emits_all_runtime_planes() {
            return;
        }
        let signal_input = self.root_enum_named(root_enums, "Input");
        let signal_output = self.root_enum_named(root_enums, "Output");
        let nexus_work = self.declaration_enum_named(declarations, "NexusWork");
        let nexus_action = self.declaration_enum_named(declarations, "NexusAction");
        let sema_write_input = self.declaration_enum_named(declarations, "SemaWriteInput");
        let sema_write_output = self.declaration_enum_named(declarations, "SemaWriteOutput");
        let sema_read_input = self.declaration_enum_named(declarations, "SemaReadInput");
        let sema_read_output = self.declaration_enum_named(declarations, "SemaReadOutput");

        if let (
            Some(signal_input),
            Some(signal_output),
            Some(nexus_work),
            Some(nexus_action),
            Some(sema_write_input),
            Some(sema_write_output),
            Some(sema_read_input),
            Some(sema_read_output),
        ) = (
            signal_input,
            signal_output,
            nexus_work,
            nexus_action,
            sema_write_input,
            sema_write_output,
            sema_read_input,
            sema_read_output,
        ) {
            let projection = SplitSemaProjection {
                signal_input,
                signal_output,
                nexus_work,
                nexus_action,
                sema_write_input,
                sema_write_output,
                sema_read_input,
                sema_read_output,
            };
            if self.can_emit_split_nexus_work_projection(&projection) {
                self.emit_split_nexus_work_projection(&projection);
            }
            self.emit_nexus_action_projection(nexus_action);
            if self.enum_has_unique_payload_variant(
                nexus_work,
                "SemaWriteCompleted",
                "SemaWriteOutput",
            ) {
                self.emit_split_sema_output_projection("WriteOutput", "SemaWriteOutput");
            }
            if self.enum_has_unique_payload_variant(
                nexus_work,
                "SemaReadCompleted",
                "SemaReadOutput",
            ) {
                self.emit_split_sema_output_projection("ReadOutput", "SemaReadOutput");
            }
        }
    }

    fn emit_runtime_role_trait_impls(
        &mut self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) {
        let mut role_impls = Vec::<RuntimeRoleTraitImpl>::new();

        if self.runtime_planes().emits_nexus() {
            self.push_role_trait_impl_if_local_role_type(
                &mut role_impls,
                declarations,
                root_enums,
                "NexusWork",
                "triad_runtime::NexusWork",
            );
            if let Some(shape) = self.nexus_runner_shape(declarations) {
                self.push_role_trait_impl_if_local_role_type(
                    &mut role_impls,
                    declarations,
                    root_enums,
                    shape.sema_write_input_type(),
                    "triad_runtime::SemaWriteInput",
                );
                self.push_role_trait_impl_if_local_role_type(
                    &mut role_impls,
                    declarations,
                    root_enums,
                    shape.sema_read_input_type(),
                    "triad_runtime::SemaReadInput",
                );
                self.push_role_trait_impl_if_local_role_type(
                    &mut role_impls,
                    declarations,
                    root_enums,
                    shape.effect_command_type(),
                    "triad_runtime::NexusEffectCommand",
                );
                if let Some(effect_result_type) = shape.effect_result_type.as_deref() {
                    self.push_role_trait_impl_if_local_role_type(
                        &mut role_impls,
                        declarations,
                        root_enums,
                        effect_result_type,
                        "triad_runtime::NexusEffectResult",
                    );
                }
            }
        }

        if self.runtime_planes().emits_sema() {
            if let Some(root) = self.sema_write_input_root(declarations, root_enums) {
                self.push_role_trait_impl(
                    &mut role_impls,
                    root.name().as_str(),
                    "triad_runtime::SemaWriteInput",
                );
            }
            if let Some(root) = self.sema_write_output_root(declarations, root_enums) {
                self.push_role_trait_impl(
                    &mut role_impls,
                    root.name().as_str(),
                    "triad_runtime::SemaWriteOutput",
                );
            }
            if let Some(root) = self.sema_read_input_root(declarations, root_enums) {
                self.push_role_trait_impl(
                    &mut role_impls,
                    root.name().as_str(),
                    "triad_runtime::SemaReadInput",
                );
            }
            if let Some(root) = self.sema_read_output_root(declarations, root_enums) {
                self.push_role_trait_impl(
                    &mut role_impls,
                    root.name().as_str(),
                    "triad_runtime::SemaReadOutput",
                );
            }
        }

        for role_impl in role_impls {
            let type_name = RustTypeTokens::new(&role_impl.type_name);
            let trait_name = RustTypeTokens::new(role_impl.trait_name);
            self.emit_item_tokens(quote! {
                impl #trait_name for #type_name {}
            });
            self.blank();
        }
    }

    fn push_role_trait_impl_if_local_role_type(
        &self,
        role_impls: &mut Vec<RuntimeRoleTraitImpl>,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
        type_name: &str,
        trait_name: &'static str,
    ) {
        if type_name == "std::convert::Infallible" {
            return;
        }
        if self.local_runtime_role_type_exists(declarations, root_enums, type_name) {
            self.push_role_trait_impl(role_impls, type_name, trait_name);
        }
    }

    fn push_role_trait_impl(
        &self,
        role_impls: &mut Vec<RuntimeRoleTraitImpl>,
        type_name: &str,
        trait_name: &'static str,
    ) {
        // Newtypes are distinct, so a type is its own canonical role type —
        // there is no transparent alias to resolve through.
        let canonical_type_name = type_name.to_owned();
        if !role_impls
            .iter()
            .any(|role_impl| role_impl.matches(&canonical_type_name, trait_name))
        {
            role_impls.push(RuntimeRoleTraitImpl::new(
                type_name.to_owned(),
                trait_name,
                canonical_type_name,
            ));
        }
    }

    fn local_runtime_role_type_exists(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
        type_name: &str,
    ) -> bool {
        self.declaration_enum_named(declarations, type_name)
            .is_some()
            || self.root_enum_named(root_enums, type_name).is_some()
    }

    fn emit_nexus_runner_next_step_projection(&mut self, shape: &NexusRunnerShape) {
        self.emit_item_tokens(NexusRunnerNextStepProjectionTokens::new(shape).into_token_stream());
        self.blank();
    }

    fn emit_nexus_runner_adapter(&mut self, shape: &NexusRunnerShape) {
        self.emit_item_tokens(NexusRunnerAdapterTokens::new(shape).into_token_stream());
        self.blank();
    }

    fn emit_split_nexus_work_projection(&mut self, projection: &SplitSemaProjection<'_>) {
        let signal_arrived_arms = self.split_signal_arrived_arms(projection);
        let sema_write_arms = self.split_output_arms(
            projection.sema_write_output,
            projection.signal_output,
            "SemaWriteOutput",
        );
        let sema_read_arms = self.split_output_arms(
            projection.sema_read_output,
            projection.signal_output,
            "SemaReadOutput",
        );
        self.emit_item_tokens(quote! {
            impl nexus::Nexus<nexus::Work> {
                pub fn into_nexus_action(self) -> nexus::Nexus<nexus::Action> {
                    let origin_route = self.origin_route();
                    match self.into_root() {
                        NexusWork::SignalArrived(input) => match input {
                            #(#signal_arrived_arms)*
                        },
                        NexusWork::SemaWriteCompleted(output) => match output {
                            #(#sema_write_arms)*
                        },
                        NexusWork::SemaReadCompleted(output) => match output {
                            #(#sema_read_arms)*
                        },
                        _ => panic!("nexus work cannot project to a generated nexus action"),
                    }
                    .with_origin_route(origin_route)
                }
            }
        });
        self.blank();
    }

    /// The `Input::Variant(payload) => NexusAction::from(...)` arms for the
    /// `SignalArrived` leg: each signal input variant routes to a SEMA write
    /// or read input target, preferring an exact name match before a unique
    /// payload-type fallback.
    fn split_signal_arrived_arms(&self, projection: &SplitSemaProjection<'_>) -> Vec<TokenStream> {
        let mut arms = Vec::new();
        for variant in projection.signal_input.variants() {
            let source = RustIdentifier::new(variant.name().as_str());
            if let Some(target_variant) =
                self.exact_target_variant_for_source(variant, projection.sema_write_input)
            {
                let target = RustIdentifier::new(target_variant.name().as_str());
                arms.push(quote! {
                    Input::#source(payload) => NexusAction::from(SemaWriteInput::#target(payload)),
                });
                continue;
            }
            if let Some(target_variant) =
                self.exact_target_variant_for_source(variant, projection.sema_read_input)
            {
                let target = RustIdentifier::new(target_variant.name().as_str());
                arms.push(quote! {
                    Input::#source(payload) => NexusAction::from(SemaReadInput::#target(payload)),
                });
                continue;
            }
            let write_fallback =
                self.fallback_target_variant_for_source(variant, projection.sema_write_input);
            let read_fallback =
                self.fallback_target_variant_for_source(variant, projection.sema_read_input);
            match (write_fallback, read_fallback) {
                (Some(target_variant), None) => {
                    let target = RustIdentifier::new(target_variant.name().as_str());
                    arms.push(quote! {
                        Input::#source(payload) => NexusAction::from(SemaWriteInput::#target(payload)),
                    });
                }
                (None, Some(target_variant)) => {
                    let target = RustIdentifier::new(target_variant.name().as_str());
                    arms.push(quote! {
                        Input::#source(payload) => NexusAction::from(SemaReadInput::#target(payload)),
                    });
                }
                (Some(_), Some(_)) | (None, None) => {}
            }
        }
        arms
    }

    /// The `<SemaOutput>::Variant(payload) => NexusAction::from(Output::Target(payload))`
    /// arms for a SEMA completion leg, keyed by the source-output enum name.
    fn split_output_arms(
        &self,
        sema_output: &RustEnum,
        signal_output: &RustEnum,
        source_enum: &str,
    ) -> Vec<TokenStream> {
        let source_enum = RustIdentifier::new(source_enum);
        let mut arms = Vec::new();
        for variant in sema_output.variants() {
            if let Some(target_variant) = self.target_variant_for_source(variant, signal_output) {
                let source = RustIdentifier::new(variant.name().as_str());
                let target = RustIdentifier::new(target_variant.name().as_str());
                arms.push(quote! {
                    #source_enum::#source(payload) => NexusAction::from(Output::#target(payload)),
                });
            }
        }
        arms
    }

    fn emit_nexus_action_projection(&mut self, nexus_action: &RustEnum) {
        let has_sema_write =
            self.enum_has_variant_payload(nexus_action, "CommandSemaWrite", "SemaWriteInput");
        let has_sema_read =
            self.enum_has_variant_payload(nexus_action, "CommandSemaRead", "SemaReadInput");
        let has_signal = self.enum_has_variant_payload(nexus_action, "ReplyToSignal", "Output");
        if !has_sema_write && !has_sema_read && !has_signal {
            return;
        }
        let mut methods = Vec::<TokenStream>::new();
        if has_sema_write {
            methods.push(quote! {
                pub fn into_sema_write_input(self) -> sema::Sema<sema::WriteInput> {
                    let origin_route = self.origin_route();
                    match self.into_root() {
                        NexusAction::CommandSemaWrite(input) => input.with_origin_route(origin_route),
                        _ => panic!("nexus action is not a SEMA write input"),
                    }
                }
            });
        }
        if has_sema_read {
            methods.push(quote! {
                pub fn into_sema_read_input(self) -> sema::Sema<sema::ReadInput> {
                    let origin_route = self.origin_route();
                    match self.into_root() {
                        NexusAction::CommandSemaRead(input) => input.with_origin_route(origin_route),
                        _ => panic!("nexus action is not a SEMA read input"),
                    }
                }
            });
        }
        if has_signal {
            methods.push(quote! {
                pub fn into_signal_output(self) -> signal::Signal<signal::Output> {
                    let origin_route = self.origin_route();
                    match self.into_root() {
                        NexusAction::ReplyToSignal(output) => output.with_origin_route(origin_route),
                        _ => panic!("nexus action is not a signal reply"),
                    }
                }
            });
        }
        self.emit_item_tokens(quote! {
            impl nexus::Nexus<nexus::Action> {
                #(#methods)*
            }
        });
        self.blank();
    }

    fn emit_split_sema_output_projection(&mut self, plane_alias: &str, type_name: &str) {
        let plane_alias = RustIdentifier::new(plane_alias);
        self.emit_item_tokens(quote! {
            impl sema::Sema<sema::#plane_alias> {
                pub fn into_nexus_work(self) -> nexus::Nexus<nexus::Work> {
                    let origin_route = self.origin_route();
                    NexusWork::from(self.into_root()).with_origin_route(origin_route)
                }
            }
        });
        self.blank();
        let _ = type_name;
    }

    fn can_emit_split_nexus_work_projection(&self, projection: &SplitSemaProjection<'_>) -> bool {
        self.enum_has_unique_payload_variant(projection.nexus_work, "SignalArrived", "Input")
            && self.enum_has_unique_payload_variant(
                projection.nexus_work,
                "SemaWriteCompleted",
                "SemaWriteOutput",
            )
            && self.enum_has_unique_payload_variant(
                projection.nexus_work,
                "SemaReadCompleted",
                "SemaReadOutput",
            )
            && self.enum_has_unique_payload_variant(
                projection.nexus_action,
                "CommandSemaWrite",
                "SemaWriteInput",
            )
            && self.enum_has_unique_payload_variant(
                projection.nexus_action,
                "CommandSemaRead",
                "SemaReadInput",
            )
            && self.enum_has_unique_payload_variant(
                projection.nexus_action,
                "ReplyToSignal",
                "Output",
            )
            && self.all_payloads_project_to_one_of(
                projection.signal_input,
                projection.sema_write_input,
                projection.sema_read_input,
            )
            && self.all_payloads_project_to(projection.sema_write_output, projection.signal_output)
            && self.all_payloads_project_to(projection.sema_read_output, projection.signal_output)
    }

    fn all_payloads_project_to_one_of(
        &self,
        source: &RustEnum,
        first_target: &RustEnum,
        second_target: &RustEnum,
    ) -> bool {
        source.variants().iter().all(|variant| {
            self.exact_target_variant_for_source(variant, first_target)
                .is_some()
                || self
                    .exact_target_variant_for_source(variant, second_target)
                    .is_some()
                || matches!(
                    (
                        self.fallback_target_variant_for_source(variant, first_target),
                        self.fallback_target_variant_for_source(variant, second_target),
                    ),
                    (Some(_), None) | (None, Some(_))
                )
        })
    }

    fn all_payloads_project_to(&self, source: &RustEnum, target: &RustEnum) -> bool {
        source
            .variants()
            .iter()
            .all(|variant| self.target_variant_for_source(variant, target).is_some())
    }

    fn target_variant_for_source<'target>(
        &self,
        source_variant: &RustEnumVariant,
        target: &'target RustEnum,
    ) -> Option<&'target RustEnumVariant> {
        self.exact_target_variant_for_source(source_variant, target)
            .or_else(|| self.fallback_target_variant_for_source(source_variant, target))
    }

    fn exact_target_variant_for_source<'target>(
        &self,
        source_variant: &RustEnumVariant,
        target: &'target RustEnum,
    ) -> Option<&'target RustEnumVariant> {
        let payload_name = self.plain_payload_name(source_variant)?;
        target.variants().iter().find(|target_variant| {
            target_variant.name().as_str() == source_variant.name().as_str()
                && self.plain_payload_name(target_variant) == Some(payload_name)
        })
    }

    fn fallback_target_variant_for_source<'target>(
        &self,
        source_variant: &RustEnumVariant,
        target: &'target RustEnum,
    ) -> Option<&'target RustEnumVariant> {
        let payload_name = self.plain_payload_name(source_variant)?;
        self.unique_plain_payload_variants(target)
            .into_iter()
            .find(|target_variant| self.plain_payload_name(target_variant) == Some(payload_name))
    }

    fn enum_has_unique_payload_variant(
        &self,
        declaration: &RustEnum,
        variant_name: &str,
        payload_name: &str,
    ) -> bool {
        self.unique_plain_payload_variants(declaration)
            .iter()
            .any(|variant| {
                variant.name().as_str() == variant_name
                    && self.plain_payload_name(variant) == Some(payload_name)
            })
    }

    fn enum_has_variant_payload(
        &self,
        declaration: &RustEnum,
        variant_name: &str,
        payload_name: &str,
    ) -> bool {
        declaration.variants().iter().any(|variant| {
            variant.name().as_str() == variant_name
                && self.plain_payload_name(variant) == Some(payload_name)
        })
    }

    fn enum_has_variant_named(&self, declaration: &RustEnum, variant_name: &str) -> bool {
        declaration
            .variants()
            .iter()
            .any(|variant| variant.name().as_str() == variant_name)
    }

    fn variant_plain_payload_name(
        &self,
        declaration: &RustEnum,
        variant_name: &str,
    ) -> Option<String> {
        declaration
            .variants()
            .iter()
            .find(|variant| variant.name().as_str() == variant_name)
            .and_then(|variant| self.plain_payload_name(variant))
            .map(ToOwned::to_owned)
    }

    fn nexus_runner_shape(&self, declarations: &[RustDeclaration]) -> Option<NexusRunnerShape> {
        let nexus_work = self.declaration_enum_named(declarations, "NexusWork")?;
        let nexus_action = self.declaration_enum_named(declarations, "NexusAction")?;
        let reply_type = self.variant_plain_payload_name(nexus_action, "ReplyToSignal")?;

        let sema_write_input_type =
            self.variant_plain_payload_name(nexus_action, "CommandSemaWrite");
        let sema_write_output_type =
            self.variant_plain_payload_name(nexus_work, "SemaWriteCompleted");
        let sema_read_input_type = self.variant_plain_payload_name(nexus_action, "CommandSemaRead");
        let sema_read_output_type =
            self.variant_plain_payload_name(nexus_work, "SemaReadCompleted");
        let effect_command_type = self.variant_plain_payload_name(nexus_action, "CommandEffect");
        let effect_result_type = self.variant_plain_payload_name(nexus_work, "EffectCompleted");
        let continue_type = self.variant_plain_payload_name(nexus_action, "Continue");
        let has_continue_variant = self.enum_has_variant_named(nexus_action, "Continue");
        let has_continue = continue_type
            .as_deref()
            .is_some_and(|type_name| type_name == "NexusWork");

        if self.enum_has_variant_named(nexus_action, "CommandSemaWrite")
            != sema_write_input_type.is_some()
            || self.enum_has_variant_named(nexus_work, "SemaWriteCompleted")
                != sema_write_output_type.is_some()
            || self.enum_has_variant_named(nexus_action, "CommandSemaRead")
                != sema_read_input_type.is_some()
            || self.enum_has_variant_named(nexus_work, "SemaReadCompleted")
                != sema_read_output_type.is_some()
            || self.enum_has_variant_named(nexus_action, "CommandEffect")
                != effect_command_type.is_some()
            || self.enum_has_variant_named(nexus_work, "EffectCompleted")
                != effect_result_type.is_some()
            || has_continue_variant != has_continue
        {
            return None;
        }

        if sema_write_input_type.is_some() != sema_write_output_type.is_some()
            || sema_read_input_type.is_some() != sema_read_output_type.is_some()
            || effect_command_type.is_some() != effect_result_type.is_some()
        {
            return None;
        }

        let recognized_action_variants = nexus_action.variants().iter().all(|variant| {
            matches!(
                variant.name().as_str(),
                "ReplyToSignal"
                    | "CommandSemaWrite"
                    | "CommandSemaRead"
                    | "CommandEffect"
                    | "Continue"
            )
        });

        if !recognized_action_variants {
            return None;
        }

        Some(NexusRunnerShape {
            reply_type,
            sema_write_input_type,
            sema_write_output_type,
            sema_read_input_type,
            sema_read_output_type,
            effect_command_type,
            effect_result_type,
            has_continue,
        })
    }

    fn emit_upgrade_support(&mut self) {
        self.emit_item_tokens(quote! {
            pub trait UpgradeFrom<Previous>: Sized {
                type Error;
                fn upgrade_from(previous: Previous) -> Result<Self, Self::Error>;
            }
            pub trait AcceptPrevious<Previous>: UpgradeFrom<Previous> {
                fn accept_previous(previous: Previous) -> Result<Self, Self::Error> {
                    Self::upgrade_from(previous)
                }
            }
            impl<Current, Previous> AcceptPrevious<Previous> for Current where
                Current: UpgradeFrom<Previous>
            {}
        });
    }

    fn emits_signal_engine_support(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> bool {
        if !self.has_root_enum(root_enums, "Input") || !self.has_root_enum(root_enums, "Output") {
            return false;
        }
        if matches!(self.target, RustEmissionTarget::SignalRuntime) {
            return true;
        }
        self.runtime_planes().emits_signal()
            && self.has_type(declarations, "NexusWork")
            && self.has_type(declarations, "NexusAction")
    }

    fn emits_concrete_signal_engine_support(&self, declarations: &[RustDeclaration]) -> bool {
        self.runtime_planes().emits_nexus()
            && self.has_type(declarations, "NexusWork")
            && self.has_type(declarations, "NexusAction")
    }

    fn emits_nexus_engine_support(&self, declarations: &[RustDeclaration]) -> bool {
        self.runtime_planes().emits_nexus()
            && self.has_type(declarations, "NexusWork")
            && self.has_type(declarations, "NexusAction")
    }

    fn sema_write_input_type_name(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> Option<&'static str> {
        if self.has_type(declarations, "WriteInput") || self.has_root_enum(root_enums, "WriteInput")
        {
            Some("WriteInput")
        } else if self.has_type(declarations, "SemaWriteInput") {
            Some("SemaWriteInput")
        } else {
            None
        }
    }

    fn sema_write_output_type_name(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> Option<&'static str> {
        if self.has_type(declarations, "WriteOutput")
            || self.has_root_enum(root_enums, "WriteOutput")
        {
            Some("WriteOutput")
        } else if self.has_type(declarations, "SemaWriteOutput") {
            Some("SemaWriteOutput")
        } else {
            None
        }
    }

    fn sema_read_input_type_name(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> Option<&'static str> {
        if self.has_type(declarations, "ReadInput") || self.has_root_enum(root_enums, "ReadInput") {
            Some("ReadInput")
        } else if self.has_type(declarations, "SemaReadInput") {
            Some("SemaReadInput")
        } else {
            None
        }
    }

    fn sema_read_output_type_name(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> Option<&'static str> {
        if self.has_type(declarations, "ReadOutput") || self.has_root_enum(root_enums, "ReadOutput")
        {
            Some("ReadOutput")
        } else if self.has_type(declarations, "SemaReadOutput") {
            Some("SemaReadOutput")
        } else {
            None
        }
    }

    fn sema_write_input_root<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
        root_enums: &'schema [RustEnum],
    ) -> Option<&'schema RustEnum> {
        self.declaration_enum_named(declarations, "WriteInput")
            .or_else(|| self.root_enum_named(root_enums, "WriteInput"))
            .or_else(|| self.declaration_enum_named(declarations, "SemaWriteInput"))
    }

    fn sema_write_output_root<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
        root_enums: &'schema [RustEnum],
    ) -> Option<&'schema RustEnum> {
        self.declaration_enum_named(declarations, "WriteOutput")
            .or_else(|| self.root_enum_named(root_enums, "WriteOutput"))
            .or_else(|| self.declaration_enum_named(declarations, "SemaWriteOutput"))
    }

    fn sema_read_input_root<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
        root_enums: &'schema [RustEnum],
    ) -> Option<&'schema RustEnum> {
        self.declaration_enum_named(declarations, "ReadInput")
            .or_else(|| self.root_enum_named(root_enums, "ReadInput"))
            .or_else(|| self.declaration_enum_named(declarations, "SemaReadInput"))
    }

    fn sema_read_output_root<'schema>(
        &self,
        declarations: &'schema [RustDeclaration],
        root_enums: &'schema [RustEnum],
    ) -> Option<&'schema RustEnum> {
        self.declaration_enum_named(declarations, "ReadOutput")
            .or_else(|| self.root_enum_named(root_enums, "ReadOutput"))
            .or_else(|| self.declaration_enum_named(declarations, "SemaReadOutput"))
    }

    fn has_sema_write_roots(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> bool {
        self.sema_write_input_root(declarations, root_enums)
            .is_some()
            && self
                .sema_write_output_root(declarations, root_enums)
                .is_some()
    }

    fn has_sema_read_roots(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> bool {
        self.sema_read_input_root(declarations, root_enums)
            .is_some()
            && self
                .sema_read_output_root(declarations, root_enums)
                .is_some()
    }

    fn emits_sema_apply_support(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> bool {
        self.runtime_planes().emits_sema() && self.has_sema_write_roots(declarations, root_enums)
    }

    fn emits_sema_observe_support(
        &self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) -> bool {
        self.runtime_planes().emits_sema() && self.has_sema_read_roots(declarations, root_enums)
    }

    fn emit_schema_plane_trait_support(
        &mut self,
        declarations: &[RustDeclaration],
        root_enums: &[RustEnum],
    ) {
        let emits_signal_engine = self.emits_signal_engine_support(declarations, root_enums);
        let emits_concrete_signal_engine =
            emits_signal_engine && self.emits_concrete_signal_engine_support(declarations);
        let emits_nexus_engine = self.emits_nexus_engine_support(declarations);
        let emits_sema_apply = self.emits_sema_apply_support(declarations, root_enums);
        let emits_sema_observe = self.emits_sema_observe_support(declarations, root_enums);
        let emits_sema_engine = emits_sema_apply || emits_sema_observe;
        let nexus_runner_shape = if emits_nexus_engine {
            self.nexus_runner_shape(declarations)
        } else {
            None
        };

        if emits_signal_engine || emits_nexus_engine || emits_sema_engine {
            self.emit_actor_lifecycle_support();
        }

        if let Some(shape) = nexus_runner_shape.as_ref() {
            self.emit_nexus_runner_next_step_projection(shape);
        }

        if emits_signal_engine {
            self.emit_item_tokens(
                SignalEngineTraitTokens::new(emits_concrete_signal_engine).into_token_stream(),
            );
            self.blank();
        }
        if emits_nexus_engine {
            self.emit_item_tokens(
                NexusEngineTraitTokens::new(nexus_runner_shape.as_ref()).into_token_stream(),
            );
            self.blank();
            if let Some(shape) = nexus_runner_shape.as_ref() {
                self.emit_nexus_runner_adapter(shape);
            }
        }
        if emits_sema_engine {
            self.emit_item_tokens(
                SemaEngineTraitTokens::new(emits_sema_apply, emits_sema_observe)
                    .into_token_stream(),
            );
            self.blank();
        }
    }

    fn emit_actor_lifecycle_support(&mut self) {
        self.emit_item_tokens(EngineLifecycleSupportTokens::new().into_token_stream());
        self.blank();
    }

    fn has_root_enum(&self, root_enums: &[RustEnum], type_name: &str) -> bool {
        root_enums
            .iter()
            .any(|declaration| declaration.name().as_str() == type_name)
    }

    fn has_type(&self, declarations: &[RustDeclaration], type_name: &str) -> bool {
        declarations
            .iter()
            .any(|declaration| declaration.name().as_str() == type_name)
    }

    fn root_enum_named<'root>(
        &self,
        root_enums: &'root [RustEnum],
        type_name: &str,
    ) -> Option<&'root RustEnum> {
        root_enums
            .iter()
            .find(|declaration| declaration.name().as_str() == type_name)
    }

    fn declaration_enum_named<'declaration>(
        &self,
        declarations: &'declaration [RustDeclaration],
        type_name: &str,
    ) -> Option<&'declaration RustEnum> {
        declarations
            .iter()
            .find(|declaration| declaration.name().as_str() == type_name)
            .and_then(|declaration| match declaration.value() {
                RustTypeDeclaration::Enum(value) => Some(value),
                _ => None,
            })
    }

    /// The Rust type for a reference.
    ///
    /// Scalar leaves map to emitted scalar aliases. A plain declared-name
    /// leaf maps to its local or imported type name. The
    /// collection variants recurse: `Vector` → `Vec<inner>`, `Map` →
    /// `BTreeMap<key, value>` (the `KeyValue` keyword), `Optional` →
    /// `Option<inner>`. `BTreeMap` is written fully-qualified so no
    /// `use` is emitted and the ordering is deterministic (rkyv + NOTA
    /// round-trips need a stable key order).
    fn rust_type(&self, reference: &TypeReference) -> String {
        match reference {
            TypeReference::String => "String".to_owned(),
            TypeReference::Integer => "Integer".to_owned(),
            TypeReference::Boolean => "Boolean".to_owned(),
            TypeReference::Path => "Path".to_owned(),
            TypeReference::Bytes => "Bytes".to_owned(),
            TypeReference::FixedBytes(width) => format!("FixedBytes<{width}>"),
            TypeReference::Plain(name) => name.as_str().to_owned(),
            TypeReference::Vector(inner) => format!("Vec<{}>", self.rust_type(inner)),
            TypeReference::Map(key, value) => format!(
                "std::collections::BTreeMap<{}, {}>",
                self.rust_type(key),
                self.rust_type(value)
            ),
            TypeReference::Optional(inner) => format!("Option<{}>", self.rust_type(inner)),
            TypeReference::ScopeOf(inner) => match inner.plain_name() {
                Some(name) => format!("{name}Scope"),
                None => self.rust_type(inner),
            },
            TypeReference::Application { head, arguments } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.rust_type(argument))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{arguments}>", head.name().as_str())
            }
        }
    }
}

struct RustKeyword<'name> {
    name: &'name str,
}

impl<'name> RustKeyword<'name> {
    fn new(name: &'name str) -> Self {
        Self { name }
    }

    fn is_reserved(&self) -> bool {
        matches!(
            self.name,
            "as" | "break"
                | "const"
                | "continue"
                | "crate"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "ref"
                | "return"
                | "self"
                | "Self"
                | "static"
                | "struct"
                | "super"
                | "trait"
                | "true"
                | "type"
                | "unsafe"
                | "use"
                | "where"
                | "while"
                | "async"
                | "await"
                | "dyn"
                | "abstract"
                | "become"
                | "box"
                | "do"
                | "final"
                | "macro"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "yield"
                | "try"
        )
    }
}
