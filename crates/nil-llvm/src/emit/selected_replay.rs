//! Selected-arm scalar snapshots across a sequence-valued region boundary.
use super::{Builder, Operand, read_replay::Probe, ty};
use nil_hir::{Instruction, Type};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(super) struct Continuation {
    original: Operand,
    owner: String,
    valid_slot: String,
    probes: Vec<Probe>,
    future: Vec<(usize, Instruction)>,
    needed: BTreeSet<usize>,
    cache: BTreeMap<usize, (Type, String)>,
}
impl Builder<'_> {
    pub(super) fn begin_selected_replay(
        &mut self,
        instructions: &[Instruction],
        position: usize,
        needed: BTreeSet<usize>,
        original: Operand,
        owner: String,
        values: &[Operand],
    ) {
        let valid_slot = self.register();
        self.allocations
            .push(format!("{valid_slot} = alloca i1, align 1"));
        self.line(format!("store i1 false, ptr {valid_slot}, align 1"));
        let mut cache = BTreeMap::new();
        for &site in &needed {
            let kind = instructions[site].ty;
            let slot = self.register();
            self.allocations
                .push(format!("{slot} = alloca {}, align 8", ty(kind)));
            let zero = if kind == Type::Bool { "false" } else { "0" };
            self.line(format!("store {} {zero}, ptr {slot}, align 8", ty(kind)));
            cache.insert(site, (kind, slot));
        }
        let mut probes = values
            .iter()
            .cloned()
            .map(Self::replay_known)
            .collect::<Vec<_>>();
        probes.push(self.replay_invalid(instructions[position].ty));
        let end = *needed.last().expect("scalar continuation");
        let future = instructions
            .iter()
            .enumerate()
            .take(end + 1)
            .skip(position + 1)
            .map(|(site, instruction)| (site, instruction.clone()))
            .collect();
        self.selected_replays.push(Continuation {
            original,
            owner,
            valid_slot,
            probes,
            future,
            needed,
            cache,
        });
    }

    pub(super) fn probe_selected_replays(&mut self, original: &Operand) -> Option<Operand> {
        let frames = self
            .selected_replays
            .iter()
            .filter(|f| f.original.text == original.text)
            .cloned()
            .collect::<Vec<_>>();
        if frames.is_empty() {
            return None;
        }
        let mut valid = Operand {
            ty: Type::Bool,
            text: "true".into(),
        };
        for frame in frames {
            let mut probes = frame.probes;
            let mut budget = 512;
            for (site, instruction) in frame.future {
                let probe = if frame.needed.contains(&site) {
                    let probe = self.replay_instruction(&instruction, &probes, 8, &mut budget);
                    valid = self.replay_and(&valid, &probe.valid);
                    let (kind, slot) = &frame.cache[&site];
                    self.line(format!(
                        "store {} {}, ptr {slot}, align 8",
                        ty(*kind),
                        probe.value.text
                    ));
                    probe
                } else {
                    self.replay_invalid(instruction.ty)
                };
                probes.push(probe);
            }
        }
        Some(valid)
    }

    pub(super) fn retire_selected_replays(&mut self, original: &Operand, valid: &Operand) {
        let frames = self
            .selected_replays
            .iter()
            .filter(|f| f.original.text == original.text)
            .cloned()
            .collect::<Vec<_>>();
        for frame in frames {
            self.line(format!(
                "store i1 {}, ptr {}, align 1",
                valid.text, frame.valid_slot
            ));
            let retained = self.value(
                Type::Buffer,
                format!("select i1 {}, ptr null, ptr {}", valid.text, original.text),
            );
            self.line("; selected continuation retires the outer owner only on valid snapshots");
            self.store_root(&frame.owner, &retained.text);
        }
    }

    pub(super) fn finish_selected_replay(&mut self) -> BTreeMap<usize, (Operand, Operand)> {
        let frame = self
            .selected_replays
            .pop()
            .expect("selected continuation frame");
        let valid = self.value(
            Type::Bool,
            format!("load i1, ptr {}, align 1", frame.valid_slot),
        );
        frame
            .cache
            .into_iter()
            .map(|(site, (kind, slot))| {
                let value = self.value(kind, format!("load {}, ptr {slot}, align 8", ty(kind)));
                (site, (valid.clone(), value))
            })
            .collect()
    }
}
