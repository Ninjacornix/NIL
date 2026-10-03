use super::*;
impl Builder<'_> {
    pub(super) fn canonical_float(&mut self, value: Operand) -> Operand {
        let nan = self.value(
            Type::Bool,
            format!("fcmp uno double {}, {}", value.text, value.text),
        );
        self.value(
            Type::F64,
            format!(
                "select i1 {}, double 0x7FF8000000000000, double {}",
                nan.text, value.text
            ),
        )
    }
    pub(super) fn wide_parts(&mut self, a: &Operand) -> String {
        let slot = self.register();
        self.allocations
            .push(format!("{slot} = alloca [2 x i64], align 8"));
        let low = self.value(Type::U64, format!("trunc i128 {} to i64", a.text));
        let high = self.value(Type::U128, format!("lshr i128 {}, 64", a.text));
        let high = self.value(Type::U64, format!("trunc i128 {} to i64", high.text));
        let ptr = self.register();
        self.line(format!("{ptr} = getelementptr i64, ptr {slot}, i64 1"));
        self.line(format!("store i64 {}, ptr {slot}, align 8", low.text));
        self.line(format!("store i64 {}, ptr {ptr}, align 8", high.text));
        slot
    }
    pub(super) fn wide_value(&mut self, slot: &str) -> Operand {
        let low = self.value(Type::U64, format!("load i64, ptr {slot}, align 8"));
        let ptr = self.register();
        self.line(format!("{ptr} = getelementptr i64, ptr {slot}, i64 1"));
        let high = self.value(Type::U64, format!("load i64, ptr {ptr}, align 8"));
        self.wide_join(&low, &high)
    }
    pub(super) fn wide_join(&mut self, low: &Operand, high: &Operand) -> Operand {
        let low = self.value(Type::U128, format!("zext i64 {} to i128", low.text));
        let high = self.value(Type::U128, format!("zext i64 {} to i128", high.text));
        let high = self.value(Type::U128, format!("shl i128 {}, 64", high.text));
        self.value(Type::U128, format!("or i128 {}, {}", low.text, high.text))
    }
    pub(super) fn numeric(
        &mut self,
        op: Intrinsic,
        a: &Operand,
        target: Type,
        span: Option<Span>,
    ) -> Operand {
        let (start, end) = Self::span(span);
        match op {
            Intrinsic::Bits => {
                return self.value(Type::U64, format!("bitcast double {} to i64", a.text));
            }
            Intrinsic::FloatBits => {
                let v = self.value(Type::F64, format!("bitcast i64 {} to double", a.text));
                return self.canonical_float(v);
            }
            Intrinsic::ParseF64 => {
                return self.value(
                    Type::F64,
                    format!(
                        "call double @nil_parse_f64(ptr {}, i64 {start}, i64 {end})",
                        a.text
                    ),
                );
            }
            Intrinsic::ParseU64 => {
                return self.value(
                    Type::U64,
                    format!(
                        "call i64 @nil_parse_u64(ptr {}, i64 {start}, i64 {end})",
                        a.text
                    ),
                );
            }
            Intrinsic::ParseU128 => {
                let slot = self.register();
                self.allocations
                    .push(format!("{slot} = alloca [2 x i64], align 8"));
                self.line(format!(
                    "call void @nil_parse_u128(ptr {}, ptr {slot}, i64 {start}, i64 {end})",
                    a.text
                ));
                return self.wide_value(&slot);
            }
            Intrinsic::Format => {
                let (name, arg) = match a.ty {
                    Type::F64 => ("f64", format!("double {}", a.text)),
                    Type::U64 => ("u64", format!("i64 {}", a.text)),
                    Type::U128 => {
                        let p = self.wide_parts(a);
                        ("u128", format!("ptr {p}"))
                    }
                    _ => unreachable!(),
                };
                return self.value(
                    Type::Bytes,
                    format!("call ptr @nil_format_{name}({arg}, i64 {start}, i64 {end})"),
                );
            }
            _ => {}
        }
        if a.ty == target {
            return a.clone();
        }
        if target == Type::F64 {
            return self.value(
                target,
                format!(
                    "{} {} {} to double",
                    if a.ty == Type::I64 {
                        "sitofp"
                    } else {
                        "uitofp"
                    },
                    ty(a.ty),
                    a.text
                ),
            );
        }
        if a.ty == Type::F64 {
            let (lower, upper) = match target {
                Type::I64 => ("0xC3E0000000000000", "0x43E0000000000000"),
                Type::U64 => ("0x0000000000000000", "0x43F0000000000000"),
                Type::U128 => ("0x0000000000000000", "0x47F0000000000000"),
                _ => unreachable!(),
            };
            let lower = self.value(Type::Bool, format!("fcmp oge double {}, {lower}", a.text));
            let upper = self.value(Type::Bool, format!("fcmp olt double {}, {upper}", a.text));
            let valid = self.value(Type::Bool, format!("and i1 {}, {}", lower.text, upper.text));
            let bad = self.value(Type::Bool, format!("xor i1 {}, true", valid.text));
            self.guard(&bad.text, 14, span);
            return self.value(
                target,
                format!(
                    "{} double {} to {}",
                    if target == Type::I64 {
                        "fptosi"
                    } else {
                        "fptoui"
                    },
                    a.text,
                    ty(target)
                ),
            );
        }
        if !matches!(op, Intrinsic::TruncI64 | Intrinsic::TruncU64) {
            if a.ty == Type::I64 && target != Type::I64 {
                let bad = self.value(Type::Bool, format!("icmp slt i64 {}, 0", a.text));
                self.guard(&bad.text, 14, span);
            } else if target == Type::I64 || (target == Type::U64 && a.ty == Type::U128) {
                let limit = if target == Type::I64 {
                    i64::MAX as u128
                } else {
                    u64::MAX as u128
                };
                let bad = self.value(
                    Type::Bool,
                    format!("icmp ugt {} {}, {limit}", ty(a.ty), a.text),
                );
                self.guard(&bad.text, 14, span);
            }
        }
        if target == Type::U128 {
            self.value(target, format!("zext i64 {} to i128", a.text))
        } else if a.ty == Type::U128 {
            self.value(target, format!("trunc i128 {} to i64", a.text))
        } else {
            Operand {
                ty: target,
                text: a.text.clone(),
            }
        }
    }
}
