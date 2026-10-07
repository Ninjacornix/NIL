use super::*;
impl Builder<'_> {
    pub(super) fn unary(&mut self, op: UnaryOp, a: &Operand, span: Option<Span>) -> Operand {
        match op {
            UnaryOp::Not | UnaryOp::BitNot => self.value(
                a.ty,
                format!(
                    "xor {} {}, {}",
                    ty(a.ty),
                    a.text,
                    if op == UnaryOp::Not { "true" } else { "-1" }
                ),
            ),
            UnaryOp::Abs if a.ty == Type::I64 => {
                let bad = self.value(Type::Bool, format!("icmp eq i64 {}, {}", a.text, i64::MIN));
                self.guard(&bad.text, 2, span);
                let neg = self.value(Type::I64, format!("sub i64 0, {}", a.text));
                let less = self.value(Type::Bool, format!("icmp slt i64 {}, 0", a.text));
                self.value(
                    Type::I64,
                    format!("select i1 {}, i64 {}, i64 {}", less.text, neg.text, a.text),
                )
            }
            UnaryOp::Abs if a.ty != Type::F64 => a.clone(),
            _ => {
                let name = match op {
                    UnaryOp::Abs => "fabs",
                    UnaryOp::Sqrt => "sqrt",
                    UnaryOp::Floor => "floor",
                    UnaryOp::Ceil => "ceil",
                    UnaryOp::Trunc => "trunc",
                    UnaryOp::Round => "roundeven",
                    _ => unreachable!("validated float unary"),
                };
                let v = self.value(
                    Type::F64,
                    format!("call double @llvm.{name}.f64(double {})", a.text),
                );
                self.canonical_float(v)
            }
        }
    }

    pub(super) fn scalar_binary(
        &mut self,
        op: BinaryOp,
        a: &Operand,
        b: &Operand,
        span: Option<Span>,
    ) -> Operand {
        use BinaryOp::*;
        let t = ty(a.ty);
        if matches!(op, Min | Max) {
            let float = a.ty == Type::F64;
            let pred = if float {
                if op == Min { "olt" } else { "ogt" }
            } else if a.ty == Type::I64 {
                if op == Min { "slt" } else { "sgt" }
            } else if op == Min {
                "ult"
            } else {
                "ugt"
            };
            let c = self.value(
                Type::Bool,
                format!(
                    "{} {pred} {t} {}, {}",
                    if float { "fcmp" } else { "icmp" },
                    a.text,
                    b.text
                ),
            );
            let v = self.value(
                a.ty,
                format!("select i1 {}, {t} {}, {t} {}", c.text, a.text, b.text),
            );
            if !float {
                return v;
            }
            let az = self.value(Type::Bool, format!("fcmp oeq double {}, 0.0", a.text));
            let bz = self.value(Type::Bool, format!("fcmp oeq double {}, 0.0", b.text));
            let zeros = self.value(Type::Bool, format!("and i1 {}, {}", az.text, bz.text));
            let ab = self.value(Type::U64, format!("bitcast double {} to i64", a.text));
            let bb = self.value(Type::U64, format!("bitcast double {} to i64", b.text));
            let zb = self.value(
                Type::U64,
                format!(
                    "{} i64 {}, {}",
                    if op == Min { "or" } else { "and" },
                    ab.text,
                    bb.text
                ),
            );
            let z = self.value(Type::F64, format!("bitcast i64 {} to double", zb.text));
            let v = self.value(
                Type::F64,
                format!(
                    "select i1 {}, double {}, double {}",
                    zeros.text, z.text, v.text
                ),
            );
            let nan = self.value(
                Type::Bool,
                format!("fcmp uno double {}, {}", a.text, b.text),
            );
            return self.value(
                Type::F64,
                format!(
                    "select i1 {}, double 0x7FF8000000000000, double {}",
                    nan.text, v.text
                ),
            );
        }
        if matches!(op, Shl | Shr) {
            let width = if a.ty == Type::U128 { 128 } else { 64 };
            let bad = self.value(Type::Bool, format!("icmp uge i64 {}, {width}", b.text));
            self.guard(&bad.text, 14, span);
            let count = if a.ty == Type::U128 {
                self.value(Type::U128, format!("zext i64 {} to i128", b.text))
            } else {
                b.clone()
            };
            let name = if op == Shl {
                "shl"
            } else if a.ty == Type::I64 {
                "ashr"
            } else {
                "lshr"
            };
            return self.value(a.ty, format!("{name} {t} {}, {}", a.text, count.text));
        }
        if op == Rem {
            let zero = self.value(Type::Bool, format!("icmp eq {t} {}, 0", b.text));
            self.guard(&zero.text, 3, span);
            if a.ty == Type::I64 {
                let amin = self.value(Type::Bool, format!("icmp eq i64 {}, {}", a.text, i64::MIN));
                let bm = self.value(Type::Bool, format!("icmp eq i64 {}, -1", b.text));
                let edge = self.value(Type::Bool, format!("and i1 {}, {}", amin.text, bm.text));
                let divisor = self.value(
                    Type::I64,
                    format!("select i1 {}, i64 1, i64 {}", edge.text, b.text),
                );
                return self.value(Type::I64, format!("srem i64 {}, {}", a.text, divisor.text));
            }
            return self.value(a.ty, format!("urem {t} {}, {}", a.text, b.text));
        }
        let name = match op {
            BitAnd => "and",
            BitOr => "or",
            BitXor => "xor",
            _ => unreachable!("extended scalar binary"),
        };
        self.value(a.ty, format!("{name} {t} {}, {}", a.text, b.text))
    }
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
