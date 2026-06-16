use super::*;

impl<'a> Interpreter<'a> {
    pub(crate) fn match_pattern(&self, pat: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
        let mut bindings = Vec::new();
        if self.match_pattern_inner(pat, value, &mut bindings) {
            Some(bindings)
        } else {
            None
        }
    }

    fn match_pattern_inner(&self, pat: &Pattern, value: &Value, bindings: &mut Vec<(String, Value)>) -> bool {
        match pat {
            Pattern::Wildcard => true,
            Pattern::Variable(name) => {
                bindings.push((name.clone(), value.clone()));
                true
            }
            Pattern::Literal(l) => {
                let expected = match l {
                    Lit::Int(v) => Value::Int(*v),
                    Lit::Float(v) => Value::Float(*v),
                    Lit::Bool(v) => Value::Bool(*v),
                    Lit::String(v) => Value::String(v.clone()),
                    Lit::FString(_) => return false, // f-strings can't be used in patterns
                    Lit::Unit => Value::Unit,
                };
                values_equal(value, &expected)
            }
            Pattern::Constructor(name, pats) => {
                match value {
                    Value::Variant(vname, vals) if vname == name => {
                        if pats.len() != vals.len() {
                            return false;
                        }
                        for (p, v) in pats.iter().zip(vals.iter()) {
                            if !self.match_pattern_inner(p, v, bindings) {
                                return false;
                            }
                        }
                        true
                    }
                    // Handle newtype pattern matching: UserId(v) matches Newtype(v)
                    Value::Newtype(inner) if pats.len() == 1 => {
                        self.match_pattern_inner(&pats[0], inner, bindings)
                    }
                    _ => false,
                }
            }
            Pattern::Tuple(pats) => {
                match value {
                    Value::Tuple(vals) if pats.len() == vals.len() => {
                        for (p, v) in pats.iter().zip(vals.iter()) {
                            if !self.match_pattern_inner(p, v, bindings) {
                                return false;
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
        }
    }
}
