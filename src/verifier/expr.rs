use crate::ast::*;
use crate::verifier::ctx::Z3VarMap;
use crate::verifier::helpers::{block_tail_expr, extract_string_empty_cmp, is_string_empty_cmp};
use z3::ast::{Bool as Z3Bool, Int as Z3Int, Real as Z3Real};

impl crate::verifier::Verifier {
    pub(crate) fn expr_to_z3_int(&self, expr: &Expr, vars: &Z3VarMap) -> Option<Z3Int> {
        match expr {
            Expr::Literal(Lit::Int(n)) => Some(Z3Int::from_i64(*n)),
            Expr::Ident(name) => vars.get_int(name).cloned(),
            Expr::Old(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    let old_name = format!("old_{}", name);
                    return vars.get_int(&old_name).cloned();
                }
                None
            }
            Expr::Binary(op, lhs, rhs) => {
                let l = self.expr_to_z3_int(lhs, vars)?;
                let r = self.expr_to_z3_int(rhs, vars)?;
                match op {
                    BinOp::Add => Some(Z3Int::add(&[&l, &r])),
                    BinOp::Sub => Some(Z3Int::sub(&[&l, &r])),
                    BinOp::Mul => Some(Z3Int::mul(&[&l, &r])),
                    BinOp::Div => Some(l.div(&r)),
                    BinOp::Mod => Some(l.modulo(&r)),
                    _ => None,
                }
            }
            Expr::Unary(UnOp::Neg, inner) => {
                let v = self.expr_to_z3_int(inner, vars)?;
                Some(v.unary_minus())
            }
            Expr::If { cond, then_, else_ } => {
                let cond_z3 = self.expr_to_z3_bool(cond, vars)?;
                let then_z3 = block_tail_expr(then_)
                    .and_then(|e| self.expr_to_z3_int(&e, vars))?;
                let else_z3 = else_
                    .as_ref()
                    .and_then(|b| block_tail_expr(b))
                    .and_then(|e| self.expr_to_z3_int(&e, vars))
                    .unwrap_or_else(|| Z3Int::from_i64(0));
                Some(cond_z3.ite(&then_z3, &else_z3))
            }
            _ => None,
        }
    }

    pub(crate) fn expr_to_z3_real(&self, expr: &Expr, vars: &Z3VarMap) -> Option<Z3Real> {
        match expr {
            Expr::Literal(Lit::Int(n)) => Some(Z3Real::from_int(&Z3Int::from_i64(*n))),
            Expr::Literal(Lit::Float(f)) => {
                if *f == 0.0 {
                    Some(Z3Real::from_int(&Z3Int::from_i64(0)))
                } else if f.is_infinite() || f.is_nan() {
                    None
                } else {
                    let scaled = (*f * 1000000.0).round() as i64;
                    Some(
                        Z3Real::from_int(&Z3Int::from_i64(scaled))
                            / Z3Real::from_int(&Z3Int::from_i64(1000000)),
                    )
                }
            }
            Expr::Ident(name) => {
                if let Some(v) = vars.get_real(name) {
                    Some(v.clone())
                } else {
                    vars.get_int(name).map(|v| Z3Real::from_int(v))
                }
            }
            Expr::Old(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    let old_name = format!("old_{}", name);
                    if let Some(v) = vars.get_real(&old_name) {
                        return Some(v.clone());
                    }
                    return vars.get_int(&old_name).map(|v| Z3Real::from_int(v));
                }
                None
            }
            Expr::Binary(op, lhs, rhs) => {
                let l = self.expr_to_z3_real(lhs, vars)?;
                let r = self.expr_to_z3_real(rhs, vars)?;
                match op {
                    BinOp::Add => Some(l + r),
                    BinOp::Sub => Some(l - r),
                    BinOp::Mul => Some(l * r),
                    BinOp::Div => Some(l / r),
                    _ => None,
                }
            }
            Expr::Unary(UnOp::Neg, inner) => {
                let v = self.expr_to_z3_real(inner, vars)?;
                Some(-v)
            }
            Expr::If { cond, then_, else_ } => {
                let cond_z3 = self.expr_to_z3_bool(cond, vars)?;
                let then_z3 = block_tail_expr(then_)
                    .and_then(|e| self.expr_to_z3_real(&e, vars))?;
                let else_z3 = else_
                    .as_ref()
                    .and_then(|b| block_tail_expr(b))
                    .and_then(|e| self.expr_to_z3_real(&e, vars))
                    .unwrap_or_else(|| Z3Real::from_int(&Z3Int::from_i64(0)));
                Some(cond_z3.ite(&then_z3, &else_z3))
            }
            _ => None,
        }
    }

    pub(crate) fn expr_to_z3_bool(&self, expr: &Expr, vars: &Z3VarMap) -> Option<Z3Bool> {
        match expr {
            Expr::Literal(Lit::Bool(b)) => Some(Z3Bool::from_bool(*b)),
            Expr::Ident(name) => {
                if let Some(v) = vars.get_int(name) {
                    Some(v.ne(&Z3Int::from_i64(0)))
                } else {
                    None
                }
            }
            Expr::Old(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    let old_name = format!("old_{}", name);
                    if let Some(v) = vars.get_int(&old_name) {
                        return Some(v.ne(&Z3Int::from_i64(0)));
                    }
                }
                None
            }
            Expr::Binary(op, lhs, rhs) => {
                // Check string emptiness comparison before int/real
                if is_string_empty_cmp(lhs, rhs, op) {
                    let (name, empty_op) = extract_string_empty_cmp(lhs, rhs, op);
                    if let Some(ne) = vars.get_string_nonempty(&name) {
                        match empty_op {
                            BinOp::NeCmp => return Some(ne.clone()),
                            BinOp::EqCmp => return Some(ne.not()),
                            _ => {}
                        }
                    }
                }

                let use_real = self.is_real_expr(lhs, vars) || self.is_real_expr(rhs, vars);

                match op {
                    BinOp::EqCmp if use_real => {
                        let l = self.expr_to_z3_real(lhs, vars)?;
                        let r = self.expr_to_z3_real(rhs, vars)?;
                        Some(l.eq(&r))
                    }
                    BinOp::NeCmp if use_real => {
                        let l = self.expr_to_z3_real(lhs, vars)?;
                        let r = self.expr_to_z3_real(rhs, vars)?;
                        Some(l.eq(&r).not())
                    }
                    BinOp::Lt if use_real => {
                        let l = self.expr_to_z3_real(lhs, vars)?;
                        let r = self.expr_to_z3_real(rhs, vars)?;
                        Some(l.lt(&r))
                    }
                    BinOp::Gt if use_real => {
                        let l = self.expr_to_z3_real(lhs, vars)?;
                        let r = self.expr_to_z3_real(rhs, vars)?;
                        Some(l.gt(&r))
                    }
                    BinOp::Le if use_real => {
                        let l = self.expr_to_z3_real(lhs, vars)?;
                        let r = self.expr_to_z3_real(rhs, vars)?;
                        Some(l.le(&r))
                    }
                    BinOp::Ge if use_real => {
                        let l = self.expr_to_z3_real(lhs, vars)?;
                        let r = self.expr_to_z3_real(rhs, vars)?;
                        Some(l.ge(&r))
                    }
                    BinOp::EqCmp => {
                        let l = self.expr_to_z3_int(lhs, vars)?;
                        let r = self.expr_to_z3_int(rhs, vars)?;
                        Some(l.eq(&r))
                    }
                    BinOp::NeCmp => {
                        let l = self.expr_to_z3_int(lhs, vars)?;
                        let r = self.expr_to_z3_int(rhs, vars)?;
                        Some(l.eq(&r).not())
                    }
                    BinOp::Lt => {
                        let l = self.expr_to_z3_int(lhs, vars)?;
                        let r = self.expr_to_z3_int(rhs, vars)?;
                        Some(l.lt(&r))
                    }
                    BinOp::Gt => {
                        let l = self.expr_to_z3_int(lhs, vars)?;
                        let r = self.expr_to_z3_int(rhs, vars)?;
                        Some(l.gt(&r))
                    }
                    BinOp::Le => {
                        let l = self.expr_to_z3_int(lhs, vars)?;
                        let r = self.expr_to_z3_int(rhs, vars)?;
                        Some(l.le(&r))
                    }
                    BinOp::Ge => {
                        let l = self.expr_to_z3_int(lhs, vars)?;
                        let r = self.expr_to_z3_int(rhs, vars)?;
                        Some(l.ge(&r))
                    }
                    BinOp::And => {
                        let l = self.expr_to_z3_bool(lhs, vars)?;
                        let r = self.expr_to_z3_bool(rhs, vars)?;
                        Some(Z3Bool::and(&[&l, &r]))
                    }
                    BinOp::Or => {
                        let l = self.expr_to_z3_bool(lhs, vars)?;
                        let r = self.expr_to_z3_bool(rhs, vars)?;
                        Some(Z3Bool::or(&[&l, &r]))
                    }
                    _ => None,
                }
            }
            Expr::Unary(UnOp::Not, inner) => {
                let v = self.expr_to_z3_bool(inner, vars)?;
                Some(v.not())
            }
            Expr::If { cond, then_, else_ } => {
                let cond_z3 = self.expr_to_z3_bool(cond, vars)?;
                let then_z3 = block_tail_expr(then_)
                    .and_then(|e| self.expr_to_z3_bool(&e, vars))?;
                let else_z3 = else_
                    .as_ref()
                    .and_then(|b| block_tail_expr(b))
                    .and_then(|e| self.expr_to_z3_bool(&e, vars))
                    .unwrap_or_else(|| Z3Bool::from_bool(true));
                Some(cond_z3.ite(&then_z3, &else_z3))
            }
            _ => None,
        }
    }

    pub(crate) fn is_real_expr(&self, expr: &Expr, vars: &Z3VarMap) -> bool {
        match expr {
            Expr::Ident(name) => vars.is_real(name),
            Expr::Literal(Lit::Float(_)) => true,
            Expr::Old(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    let old_name = format!("old_{}", name);
                    vars.is_real(&old_name)
                } else {
                    false
                }
            }
            Expr::Binary(_, lhs, rhs) => {
                self.is_real_expr(lhs, vars) || self.is_real_expr(rhs, vars)
            }
            Expr::Unary(_, inner) => self.is_real_expr(inner, vars),
            _ => false,
        }
    }
}
