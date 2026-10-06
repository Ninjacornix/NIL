//! Explicit local manifest loading for the validated semantic linking interface.
use crate::{SourceProfile, lower_with_arithmetic, syntax};
use nil_hir::{Arithmetic, Diagnostic, RecordDefinition, Type, plugin::Provider};
use std::{collections::BTreeMap, path::Path};

fn number(s: &str) -> Result<u32, Diagnostic> {
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(nil_hir::plugin::error("canonical plugin integer required"));
    }
    s.parse()
        .map_err(|_| nil_hir::plugin::error("plugin integer overflow"))
}
fn read(path: &Path) -> Result<String, Diagnostic> {
    let size = std::fs::metadata(path).map_err(|e| {
        nil_hir::plugin::error(format!("cannot read plugin {}: {e}", path.display()))
    })?;
    if !size.is_file() || size.len() > crate::parser::MAX_SOURCE_BYTES as u64 {
        return Err(nil_hir::plugin::error(
            "plugin input must be a bounded regular file",
        ));
    }
    // Recheck after the read; a concurrent change cannot bypass the size limit.
    let mut file = std::fs::File::open(path).map_err(|e| nil_hir::plugin::error(e.to_string()))?;
    use std::io::Read;
    let mut bytes = vec![];
    (&mut file)
        .take(crate::parser::MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| nil_hir::plugin::error(e.to_string()))?;
    if bytes.len() > crate::parser::MAX_SOURCE_BYTES {
        return Err(nil_hir::plugin::error("plugin input exceeds source limit"));
    }
    String::from_utf8(bytes).map_err(|_| nil_hir::plugin::error("plugin input must be UTF-8"))
}
fn spelling(ty: Type, records: &[RecordDefinition]) -> String {
    match ty {
        Type::I64 => "i".into(),
        Type::U64 => "u64".into(),
        Type::U128 => "u128".into(),
        Type::F64 => "f64".into(),
        Type::Bool => "b".into(),
        Type::Bytes => "s".into(),
        Type::Buffer => "v".into(),
        Type::MapI64 => "m".into(),
        Type::MapBytes => "t".into(),
        Type::Array(n) => n.to_string(),
        Type::Record(id, _) => records[id].name.clone(),
        Type::MapRecord(id, _) => format!("map[{}]", records[id].name),
        Type::RecordBuffer(id, _) => format!("v[{}]", records[id].name),
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plugin,
    Module,
}
struct Manifest {
    kind: Kind,
    id: u32,
    source: String,
    record_buffer: bool,
    exports: BTreeMap<u32, u32>,
    imports: Vec<String>,
}
fn relative(s: &str) -> Result<&Path, Diagnostic> {
    let path = Path::new(s);
    if path.is_absolute()
        || path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err(nil_hir::plugin::error(
            "source/import must be relative without parent traversal",
        ));
    }
    Ok(path)
}
impl Manifest {
    fn read(path: &Path) -> Result<Self, Diagnostic> {
        let text = read(path)?;
        let mut lines = text.lines();
        let kind = match lines.next() {
            Some("nil-plugin 1") => Kind::Plugin,
            Some("nil-module 1") => Kind::Module,
            _ => {
                return Err(nil_hir::plugin::error(
                    "unsupported source manifest version",
                ));
            }
        };
        let mut id = None;
        let mut source = None;
        let mut effect = false;
        let mut record_buffer = false;
        let mut exports = BTreeMap::new();
        let mut imports = vec![];
        for line in lines {
            match line.split_whitespace().collect::<Vec<_>>().as_slice() {
                [] => {}
                ["id", n] if id.is_none() => id = Some(number(n)?),
                ["source", p] if source.is_none() => {
                    relative(p)?;
                    source = Some((*p).to_string());
                }
                ["effect", "borrow"] if kind == Kind::Plugin && !effect => effect = true,
                ["types", "record-buffer"] if kind == Kind::Plugin && !record_buffer => {
                    record_buffer = true
                }
                ["import", p] if kind == Kind::Module => {
                    relative(p)?;
                    if imports.iter().any(|s| s == p) {
                        return Err(nil_hir::plugin::error("duplicate import"));
                    }
                    imports.push((*p).to_string());
                }
                ["export", op, label] => {
                    if exports.insert(number(op)?, number(label)?).is_some() {
                        return Err(nil_hir::plugin::error("duplicate export"));
                    }
                }
                _ => {
                    return Err(nil_hir::plugin::error(
                        "unknown/duplicate manifest declaration",
                    ));
                }
            }
        }
        let id = id.ok_or_else(|| nil_hir::plugin::error("missing manifest id"))?;
        if id == 0 || exports.is_empty() || exports.len() > 128 || (kind == Kind::Plugin && !effect)
        {
            return Err(nil_hir::plugin::error(
                "nonzero id, 1..128 exports and plugin borrow effect required",
            ));
        }
        Ok(Self {
            kind,
            id,
            source: source.ok_or_else(|| nil_hir::plugin::error("missing source"))?,
            record_buffer,
            exports,
            imports,
        })
    }
}
fn provider_source(
    path: &Path,
    manifest: &Manifest,
    records: &[RecordDefinition],
) -> Result<String, Diagnostic> {
    let mut provider_source = String::new();
    for record in records {
        provider_source.push_str(&format!(
            "record {}({})\n",
            record.name,
            record
                .fields
                .iter()
                .map(|f| format!("{}:{}", f.name, spelling(f.ty, records)))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    provider_source.push_str(&read(
        &path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&manifest.source),
    )?);
    if provider_source.len() > crate::parser::MAX_SOURCE_BYTES {
        return Err(nil_hir::plugin::error("combined source exceeds limit"));
    }
    Ok(provider_source)
}
fn load_plugin(
    provider_source: &str,
    manifest: &Manifest,
    records: &[RecordDefinition],
) -> Result<Vec<Provider>, Diagnostic> {
    let ast = crate::parser::parse_with_profile(provider_source, SourceProfile::ExprV5)?;
    if ast.functions.iter().any(|f| {
        f.parameters
            .iter()
            .any(|p| matches!(p, syntax::Parameter::Function(_)))
    }) {
        return Err(nil_hir::plugin::error(
            "function parameters require a source module, not a borrow-only provider",
        ));
    }
    let compiled = crate::compile_with_profile(provider_source, SourceProfile::ExprV5)?;
    if compiled.hir.program().records != records {
        return Err(nil_hir::plugin::error(
            "provider must use caller record declarations, not add types",
        ));
    }
    fn contains(items: &[nil_hir::Instruction]) -> bool {
        items.iter().any(|i| {
            matches!(i.ty, Type::RecordBuffer(..))
                || match &i.operation {
                    nil_hir::Operation::If {
                        then_region,
                        else_region,
                        ..
                    } => contains(&then_region.instructions) || contains(&else_region.instructions),
                    nil_hir::Operation::Loop {
                        condition,
                        body,
                        finish,
                        ..
                    } => [condition, body, finish]
                        .iter()
                        .any(|r| contains(&r.instructions)),
                    _ => false,
                }
        })
    }
    let program = compiled.hir.program();
    let requires = program.records.iter().any(|r| {
        r.fields
            .iter()
            .any(|f| matches!(f.ty, Type::RecordBuffer(..)))
    }) || program.functions.iter().any(|f| {
        f.parameters
            .iter()
            .chain(std::iter::once(&f.result_type))
            .any(|ty| matches!(ty, Type::RecordBuffer(..)))
            || contains(&f.instructions)
    });
    if requires && !manifest.record_buffer {
        return Err(nil_hir::plugin::error(
            "record-buffer type capability required",
        ));
    }
    manifest
        .exports
        .iter()
        .map(|(op, label)| {
            let entry = compiled
                .function(*label)
                .ok_or_else(|| nil_hir::plugin::error("unknown plugin export function"))?;
            Provider::new(manifest.id, *op, compiled.hir.clone(), entry)
        })
        .collect()
}
#[derive(Clone)]
enum Target {
    Plugin(Provider),
    Function(u32),
}
fn link(
    list: &mut [syntax::Instruction],
    providers: &BTreeMap<(u32, u32), Target>,
) -> Result<(), Diagnostic> {
    for instruction in list {
        match &mut instruction.kind {
            syntax::InstructionKind::FunctionReference(syntax::FunctionReference::Export(
                id,
                op,
            )) => {
                let target = providers
                    .get(&(*id, *op))
                    .ok_or_else(|| nil_hir::plugin::error("unknown module function export"))?;
                let Target::Function(label) = target else {
                    return Err(nil_hir::plugin::error(
                        "function arguments require ordinary source module exports",
                    ));
                };
                instruction.kind = syntax::InstructionKind::FunctionReference(
                    syntax::FunctionReference::Local(*label),
                );
            }
            syntax::InstructionKind::Plugin(0, 0, args) => {
                instruction.kind =
                    syntax::InstructionKind::Intrinsic(nil_hir::Intrinsic::Equal, args.clone());
            }
            syntax::InstructionKind::Plugin(id, op, args) => {
                let provider = providers
                    .get(&(*id, *op))
                    .ok_or_else(|| {
                        nil_hir::plugin::error(
                            "unknown plugin/operation; explicitly load its manifest",
                        )
                    })?
                    .clone();
                instruction.kind = match provider {
                    Target::Plugin(provider) => {
                        syntax::InstructionKind::LinkedPlugin(Box::new(provider), args.clone())
                    }
                    Target::Function(label) => syntax::InstructionKind::Call(label, args.clone()),
                };
            }
            syntax::InstructionKind::If(_, a, b) => {
                link(&mut a.instructions, providers)?;
                link(&mut b.instructions, providers)?;
            }
            syntax::InstructionKind::Loop(_, a, b, c) => {
                for r in [a, b, c] {
                    link(&mut r.instructions, providers)?;
                }
            }
            syntax::InstructionKind::Each(_, _, a, b) => {
                link(&mut a.instructions, providers)?;
                link(&mut b.instructions, providers)?;
            }
            _ => {}
        }
    }
    Ok(())
}
type Targets = BTreeMap<(u32, u32), Target>;
type Loaded = (u32, Targets);

/// One source loader for restricted semantic plugins and general modules.
struct Loader<'a> {
    records: &'a [RecordDefinition],
    cache: BTreeMap<std::path::PathBuf, Loaded>,
    active: std::collections::BTreeSet<std::path::PathBuf>,
    ids: BTreeMap<u32, std::path::PathBuf>,
    functions: Vec<syntax::Function>,
    next_label: u32,
    source_bytes: usize,
}
impl Loader<'_> {
    fn load(&mut self, path: &Path) -> Result<Loaded, Diagnostic> {
        let path = path
            .canonicalize()
            .map_err(|e| nil_hir::plugin::error(format!("cannot resolve manifest: {e}")))?;
        if self.active.contains(&path) {
            return Err(nil_hir::plugin::error("module import cycle"));
        }
        if let Some(loaded) = self.cache.get(&path) {
            return Ok(loaded.clone());
        }
        if self.ids.len() >= 128 || self.active.len() >= 64 {
            return Err(nil_hir::plugin::error("module graph limit exceeded"));
        }
        self.active.insert(path.clone());
        let manifest = Manifest::read(&path)?;
        if self.ids.insert(manifest.id, path.clone()).is_some() {
            return Err(nil_hir::plugin::error("duplicate source id"));
        }
        let mut exports = BTreeMap::new();
        match manifest.kind {
            Kind::Plugin => {
                // Account restricted source too: it participates in the same bounded graph.
                let source = provider_source(&path, &manifest, self.records)?;
                self.charge(source.len())?;
                for p in load_plugin(&source, &manifest, self.records)? {
                    exports.insert((p.id, p.operation), Target::Plugin(p));
                }
            }
            Kind::Module => {
                let mut visible = BTreeMap::new();
                let mut direct_ids = std::collections::BTreeSet::new();
                for import in &manifest.imports {
                    let (id, targets) = self.load(&path.parent().unwrap().join(import))?;
                    if !direct_ids.insert(id) {
                        return Err(nil_hir::plugin::error("duplicate direct import id"));
                    }
                    visible.extend(targets);
                }
                let source = provider_source(&path, &manifest, self.records)?;
                self.charge(source.len())?;
                let mut module = crate::parser::parse_with_profile(&source, SourceProfile::ExprV5)?;
                if module.records != self.records {
                    return Err(nil_hir::plugin::error(
                        "module must use root record registry",
                    ));
                }
                if module.functions.len() > 128
                    || self.functions.len() + module.functions.len() > 4096
                {
                    return Err(nil_hir::plugin::error("module function limit exceeded"));
                }
                let mut labels = BTreeMap::new();
                for f in &module.functions {
                    let fresh = self.next_label;
                    self.next_label = fresh
                        .checked_add(1)
                        .ok_or_else(|| nil_hir::plugin::error("function label limit exceeded"))?;
                    if labels.insert(f.label, fresh).is_some() {
                        return Err(Diagnostic::new(
                            "E003",
                            nil_hir::Phase::Check,
                            Some(f.span),
                            "duplicate local function",
                        ));
                    }
                }
                for (op, label) in &manifest.exports {
                    let target = labels
                        .get(label)
                        .ok_or_else(|| nil_hir::plugin::error("unknown module export function"))?;
                    exports.insert((manifest.id, *op), Target::Function(*target));
                }
                for f in &mut module.functions {
                    // Relocate only local calls, then resolve imports. Imported targets
                    // must never be accidentally relocated as local labels.
                    relocate(&mut f.instructions, &labels)?;
                    link(&mut f.instructions, &visible)?;
                    f.label = labels[&f.label];
                }
                self.functions.extend(module.functions);
            }
        }
        self.active.remove(&path);
        let loaded = (manifest.id, exports);
        self.cache.insert(path, loaded.clone());
        Ok(loaded)
    }
    fn charge(&mut self, size: usize) -> Result<(), Diagnostic> {
        self.source_bytes = self
            .source_bytes
            .checked_add(size)
            .ok_or_else(|| nil_hir::plugin::error("source limit exceeded"))?;
        if self.source_bytes > crate::parser::MAX_SOURCE_BYTES {
            return Err(nil_hir::plugin::error(
                "module graph exceeds aggregate source limit",
            ));
        }
        Ok(())
    }
}
fn relocate(
    list: &mut [syntax::Instruction],
    labels: &BTreeMap<u32, u32>,
) -> Result<(), Diagnostic> {
    for i in list {
        match &mut i.kind {
            syntax::InstructionKind::FunctionReference(syntax::FunctionReference::Local(label))
            | syntax::InstructionKind::Call(label, _) => {
                *label = *labels.get(label).ok_or_else(|| {
                    Diagnostic::new(
                        "E004",
                        nil_hir::Phase::Check,
                        Some(i.span),
                        "unknown local function",
                    )
                })?;
            }
            syntax::InstructionKind::If(_, a, b) => {
                relocate(&mut a.instructions, labels)?;
                relocate(&mut b.instructions, labels)?;
            }
            syntax::InstructionKind::Loop(_, a, b, c) => {
                for r in [a, b, c] {
                    relocate(&mut r.instructions, labels)?;
                }
            }
            syntax::InstructionKind::Each(_, _, a, b) => {
                relocate(&mut a.instructions, labels)?;
                relocate(&mut b.instructions, labels)?;
            }
            _ => {}
        }
    }
    Ok(())
}
pub fn compile_with_plugins(
    source: &str,
    profile: SourceProfile,
    paths: &[impl AsRef<Path>],
) -> Result<crate::CompiledProgram, Diagnostic> {
    if profile != SourceProfile::ExprV5 && !paths.is_empty() {
        return Err(nil_hir::plugin::error("source imports require expr-v5"));
    }
    if paths.len() > 128 {
        return Err(nil_hir::plugin::error("too many source manifests"));
    }
    let mut module = crate::parser::parse_with_profile(source, profile)?;
    let root_labels = module
        .functions
        .iter()
        .map(|f| f.label)
        .collect::<std::collections::BTreeSet<_>>();
    // Empty imports preserve the original single-file path, including label bounds.
    let mut providers = BTreeMap::new();
    if !paths.is_empty() {
        let next_label = root_labels
            .last()
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| nil_hir::plugin::error("function label overflow"))?;
        let mut loader = Loader {
            records: &module.records,
            cache: BTreeMap::new(),
            active: Default::default(),
            ids: BTreeMap::new(),
            functions: vec![],
            next_label,
            source_bytes: source.len(),
        };
        let mut root_ids = std::collections::BTreeSet::new();
        for path in paths {
            let (id, targets) = loader.load(path.as_ref())?;
            if !root_ids.insert(id) {
                return Err(nil_hir::plugin::error("duplicate root import id"));
            }
            providers.extend(targets);
        }
        if module.functions.len() + loader.functions.len() > 4096 {
            return Err(nil_hir::plugin::error("linked function limit exceeded"));
        }
        let local = root_labels.iter().map(|n| (*n, *n)).collect();
        for f in &mut module.functions {
            relocate(&mut f.instructions, &local)?;
            link(&mut f.instructions, &providers)?;
        }
        module.functions.extend(loader.functions);
    } else {
        for f in &mut module.functions {
            link(&mut f.instructions, &providers)?;
        }
    }
    let arithmetic = if matches!(
        profile,
        SourceProfile::ExprV3 | SourceProfile::ExprV4 | SourceProfile::ExprV5
    ) {
        Arithmetic::Wrapping
    } else {
        Arithmetic::Checked
    };
    let mut compiled = lower_with_arithmetic(module, arithmetic)?;
    compiled.retain_entry_labels(&root_labels);
    Ok(compiled)
}
