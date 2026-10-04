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
fn load(path: &Path, records: &[RecordDefinition]) -> Result<Vec<Provider>, Diagnostic> {
    let text = read(path)?;
    let mut lines = text.lines();
    if lines.next() != Some("nil-plugin 1") {
        return Err(nil_hir::plugin::error(
            "unsupported plugin manifest version",
        ));
    }
    let mut id = None;
    let mut source = None;
    let mut effect = false;
    let mut record_buffer = false;
    let mut exports = BTreeMap::new();
    for line in lines {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            [] => {}
            ["id", n] if id.is_none() => {
                id = Some(number(n)?);
            }
            ["source", p] if source.is_none() => {
                source = Some(*p);
            }
            ["effect", "borrow"] if !effect => {
                effect = true;
            }
            ["types", "record-buffer"] if !record_buffer => {
                record_buffer = true;
            }
            ["export", op, label] => {
                if exports.insert(number(op)?, number(label)?).is_some() {
                    return Err(nil_hir::plugin::error("duplicate plugin export"));
                }
            }
            _ => {
                return Err(nil_hir::plugin::error(
                    "unknown/duplicate plugin manifest declaration",
                ));
            }
        }
    }
    let id = id.ok_or_else(|| nil_hir::plugin::error("missing plugin id"))?;
    if id == 0 {
        return Err(nil_hir::plugin::error(
            "plugin id 0 is reserved for shipped equality",
        ));
    }
    if !effect || exports.is_empty() || exports.len() > 128 {
        return Err(nil_hir::plugin::error(
            "borrow effect and 1..128 exports required",
        ));
    }
    let source = source.ok_or_else(|| nil_hir::plugin::error("missing plugin source"))?;
    let source_path = Path::new(source);
    if source_path.is_absolute()
        || source_path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err(nil_hir::plugin::error(
            "plugin source must be relative without parent traversal",
        ));
    }
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
        &path.parent().unwrap_or(Path::new(".")).join(source_path),
    )?);
    let compiled = crate::compile_with_profile(&provider_source, SourceProfile::ExprV5)?;
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
    if requires && !record_buffer {
        return Err(nil_hir::plugin::error(
            "record-buffer type capability required",
        ));
    }
    exports
        .into_iter()
        .map(|(op, label)| {
            let entry = compiled
                .function(label)
                .ok_or_else(|| nil_hir::plugin::error("unknown plugin export function"))?;
            Provider::new(id, op, compiled.hir.clone(), entry)
        })
        .collect()
}
fn link(
    list: &mut [syntax::Instruction],
    providers: &BTreeMap<(u32, u32), Provider>,
) -> Result<(), Diagnostic> {
    for instruction in list {
        match &mut instruction.kind {
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
                instruction.kind =
                    syntax::InstructionKind::LinkedPlugin(Box::new(provider), args.clone());
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
pub fn compile_with_plugins(
    source: &str,
    profile: SourceProfile,
    paths: &[impl AsRef<Path>],
) -> Result<crate::CompiledProgram, Diagnostic> {
    if profile != SourceProfile::ExprV5 && !paths.is_empty() {
        return Err(nil_hir::plugin::error("plugins require expr-v5"));
    }
    if paths.len() > 128 {
        return Err(nil_hir::plugin::error("too many plugin manifests"));
    }
    let mut module = crate::parser::parse_with_profile(source, profile)?;
    let mut providers = BTreeMap::new();
    let mut ids = std::collections::BTreeSet::new();
    for path in paths {
        let loaded = load(path.as_ref(), &module.records)?;
        if !ids.insert(loaded[0].id) {
            return Err(nil_hir::plugin::error("duplicate plugin id"));
        }
        for provider in loaded {
            providers.insert((provider.id, provider.operation), provider);
        }
    }
    for f in &mut module.functions {
        link(&mut f.instructions, &providers)?;
    }
    let arithmetic = if matches!(
        profile,
        SourceProfile::ExprV3 | SourceProfile::ExprV4 | SourceProfile::ExprV5
    ) {
        Arithmetic::Wrapping
    } else {
        Arithmetic::Checked
    };
    lower_with_arithmetic(module, arithmetic)
}
