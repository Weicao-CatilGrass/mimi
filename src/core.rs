use crate::ast::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
}

impl Diagnostic {
    fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

pub fn check(file: &File) -> Result<(), Vec<Diagnostic>> {
    let mut checker = Checker::new(file);
    checker.check()
}

pub fn check_strict(file: &File) -> Result<(), Vec<Diagnostic>> {
    let mut checker = Checker::new(file);
    checker.strict = true;
    checker.check()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BorrowState {
    Unborrowed,
    BorrowedImm,
    BorrowedMut,
}

struct Checker<'a> {
    file: &'a File,
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    funcs: HashMap<String, (Vec<Type>, Type)>,
    aliases: HashMap<String, Type>,
    types: HashMap<String, TypeDef>,
    /// Track newtype definitions: name -> inner type (unresolved)
    newtypes: HashMap<String, Type>,
    /// Track linear capabilities in scope: name -> consumed
    cap_vars: Vec<HashMap<String, bool>>,
    /// Track borrow state of variables: name -> borrow state
    borrows: Vec<HashMap<String, BorrowState>>,
    /// Track trait definitions: trait_name -> list of method names
    traits: HashMap<String, Vec<String>>,
    /// Track trait implementations: (trait_name, type_name) -> list of method names
    impls: HashMap<(String, String), Vec<String>>,
    /// Track where clauses for functions: func_name -> (type_param, bounds)
    where_clauses: HashMap<String, (String, Vec<String>)>,
    /// Track effects for functions: func_name -> list of effect names
    func_effects: HashMap<String, Vec<String>>,
    /// Track available effects in current scope
    available_effects: Vec<HashMap<String, bool>>,
    /// Strict mode: enforce $$ lock semantics
    strict: bool,
    /// Track variable scopes for shadowing detection
    var_scopes: Vec<HashMap<String, usize>>,
    /// Track mutable variables: name -> is_mut
    mut_vars: Vec<HashMap<String, bool>>,
    /// Track generic parameters per function: func_name -> generic params
    func_generics: HashMap<String, Vec<GenericParam>>,
    /// Track generic parameters per type def: type_name -> generic params
    type_generics: HashMap<String, Vec<GenericParam>>,
    /// Track methods available on types via traits: type_name -> list of (trait_name, method_name)
    type_methods: HashMap<String, Vec<(String, String)>>,
    /// Track trait method signatures: (trait_name, method_name) -> (param_types, return_type)
    trait_method_sigs: HashMap<(String, String), (Vec<Type>, Type)>,
    /// Track imported module names (from `use` statements)
    use_imports: Vec<String>,
    /// Track current module path for qualified names
    module_path: Vec<String>,
}

impl<'a> Checker<'a> {
    fn new(file: &'a File) -> Self {
        Self {
            file,
            errors: Vec::new(),
            warnings: Vec::new(),
            funcs: HashMap::new(),
            aliases: HashMap::new(),
            types: HashMap::new(),
            newtypes: HashMap::new(),
            cap_vars: vec![HashMap::new()],
            borrows: vec![HashMap::new()],
            traits: HashMap::new(),
            impls: HashMap::new(),
            where_clauses: HashMap::new(),
            func_effects: HashMap::new(),
            available_effects: vec![HashMap::new()],
            strict: false,
            var_scopes: vec![HashMap::new()],
            mut_vars: vec![HashMap::new()],
            func_generics: HashMap::new(),
            type_generics: HashMap::new(),
            type_methods: HashMap::new(),
            trait_method_sigs: HashMap::new(),
            use_imports: Vec::new(),
            module_path: Vec::new(),
        }
    }

    fn check(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.collect_decls();
        for item in &self.file.items {
            self.check_item(item);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn emit(&mut self, msg: impl Into<String>) {
        self.errors.push(Diagnostic::new(msg));
    }

    fn push_borrow_scope(&mut self) {
        self.borrows.push(HashMap::new());
    }

    fn pop_borrow_scope(&mut self) {
        self.borrows.pop();
    }

    fn lookup_borrow(&self, name: &str) -> Option<BorrowState> {
        for scope in self.borrows.iter().rev() {
            if let Some(&state) = scope.get(name) {
                return Some(state);
            }
        }
        None
    }

    fn set_borrow(&mut self, name: &str, state: BorrowState) {
        if let Some(scope) = self.borrows.last_mut() {
            scope.insert(name.into(), state);
        }
    }

    fn collect_decls(&mut self) {
        // Process imports: add module names to use_imports
        for import in &self.file.imports {
            if let Some(module_name) = import.path.first() {
                self.use_imports.push(module_name.clone());
            }
        }
        for item in &self.file.items {
            self.collect_item_decls(item);
        }
        // Check for type alias cycles
        self.check_alias_cycles();
    }

    /// Detect type alias cycles: type A = B; type B = A;
    fn check_alias_cycles(&mut self) {
        let alias_names: Vec<String> = self.aliases.keys().cloned().collect();
        for name in &alias_names {
            let mut visited = std::collections::HashSet::new();
            visited.insert(name.clone());
            if self.follows_alias_cycle(name, &visited) {
                self.emit(format!("type alias cycle detected: '{}' forms a cycle", name));
            }
        }
    }

    fn follows_alias_cycle(&self, name: &str, visited: &std::collections::HashSet<String>) -> bool {
        if let Some(Type::Name(target, _)) = self.aliases.get(name) {
            if visited.contains(target) {
                return true;
            }
            let mut new_visited = visited.clone();
            new_visited.insert(target.clone());
            return self.follows_alias_cycle(target, &new_visited);
        }
        false
    }

    fn collect_item_decls(&mut self, item: &Item) {
        match item {
            Item::Func(f) => {
                let qualified_name = if self.module_path.is_empty() {
                    f.name.clone()
                } else {
                    format!("{}::{}", self.module_path.join("::"), f.name)
                };
                if self.funcs.contains_key(&qualified_name) {
                    self.emit(format!("duplicate function definition '{}'", qualified_name));
                    return;
                }
                let params: Vec<Type> = f.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                let ret = f
                    .ret
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                self.funcs.insert(qualified_name.clone(), (params, ret));
                // Store generic parameters if present
                if !f.generics.is_empty() {
                    self.func_generics.insert(qualified_name.clone(), f.generics.clone());
                }
                // Store where clause if present
                if let Some(where_clause) = &f.where_clause {
                    self.where_clauses.insert(
                        qualified_name.clone(),
                        (where_clause.type_param.clone(), where_clause.bounds.clone()),
                    );
                }
                // Store effects if present
                if !f.effects.is_empty() {
                    self.func_effects.insert(qualified_name, f.effects.clone());
                }
            }
            Item::Type(t) => {
                if self.types.contains_key(&t.name) {
                    self.emit(format!("duplicate type definition '{}'", t.name));
                    return;
                }
                match &t.kind {
                    TypeDefKind::Alias(ty) => {
                        let resolved = self.resolve_type(ty);
                        self.aliases.insert(t.name.clone(), resolved);
                    }
                    TypeDefKind::Newtype(ty) => {
                        // Store the newtype with its inner type (unresolved for now)
                        self.newtypes.insert(t.name.clone(), ty.clone());
                        // The inner type is what the constructor takes as input
                        let inner = self.resolve_type(ty);
                        // The return type is the newtype itself, wrapped in Type::Newtype with name
                        let self_ty = Type::Newtype(t.name.clone(), Box::new(inner.clone()));
                        self.funcs.insert(t.name.clone(), (vec![inner], self_ty));
                    }
                    TypeDefKind::Enum(variants) => {
                        let self_ty = Type::Name(t.name.clone(), vec![]);
                        for v in variants {
                            let ret = self_ty.clone();
                            let params = match &v.payload {
                                None => vec![],
                                Some(VariantPayload::Tuple(types)) => types.iter().map(|ty| self.resolve_type(ty)).collect(),
                                Some(VariantPayload::Record(fields)) => fields.iter().map(|f| self.resolve_type(&f.ty)).collect(),
                            };
                            self.funcs.insert(v.name.clone(), (params, ret));
                        }
                    }
                    _ => {}
                }
                self.types.insert(t.name.clone(), t.clone());
                // Store generic parameters for type definitions
                if !t.generics.is_empty() {
                    self.type_generics.insert(t.name.clone(), t.generics.clone());
                }
            }
            Item::Module(m) => {
                self.module_path.push(m.name.clone());
                for inner in &m.items {
                    self.collect_item_decls(inner);
                }
                self.module_path.pop();
            }
            Item::Actor(actor) => {
                // Register actor type so it can be used as a type
                let actor_type_def = TypeDef {
                    name: actor.name.clone(),
                    commitment: actor.commitment,
                    pub_: actor.pub_,
                    kind: TypeDefKind::Record(actor.fields.iter().map(|f| Field {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                    }).collect()),
                    generics: Vec::new(),
                    derives: Vec::new(),
                };
                self.types.insert(actor.name.clone(), actor_type_def);

                // Collect actor methods as functions
                for method in &actor.methods {
                    if self.funcs.contains_key(&method.name) {
                        self.emit(format!("duplicate function definition '{}'", method.name));
                        return;
                    }
                    // Add implicit self parameter as first param
                    let self_type = Type::Name(actor.name.clone(), vec![]);
                    let mut params = vec![self_type];
                    params.extend(method.params.iter().map(|p| self.resolve_type(&p.ty)));
                    let ret = method
                        .ret
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                    self.funcs.insert(method.name.clone(), (params, ret));
                }
            }
            Item::Rule(_) | Item::Desc(_) | Item::Cap(_) => {}
            Item::Trait(trait_def) => {
                let method_names: Vec<String> = trait_def.methods.iter().map(|m| m.name.clone()).collect();
                self.traits.insert(trait_def.name.clone(), method_names.clone());
                // Store trait method signatures for argument validation
                for method in &trait_def.methods {
                    let params: Vec<Type> = method.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                    let ret = method.ret.as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                    self.trait_method_sigs.insert(
                        (trait_def.name.clone(), method.name.clone()),
                        (params, ret),
                    );
                }
            }
            Item::Impl(impl_def) => {
                let method_names: Vec<String> = impl_def.methods.iter().map(|m| m.name.clone()).collect();
                self.impls.insert(
                    (impl_def.trait_name.clone(), impl_def.type_name.clone()),
                    method_names.clone(),
                );
                // Register methods available on this type via this trait
                for method_name in &method_names {
                    self.type_methods
                        .entry(impl_def.type_name.clone())
                        .or_default()
                        .push((impl_def.trait_name.clone(), method_name.clone()));
                }
                // Also register impl methods as functions with self parameter
                for method in &impl_def.methods {
                    let mut params = vec![Type::Name(impl_def.type_name.clone(), vec![])];
                    params.extend(method.params.iter().map(|p| self.resolve_type(&p.ty)));
                    let ret = method
                        .ret
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                    let key = format!("{}_{}", impl_def.type_name, method.name);
                    self.funcs.insert(key, (params, ret));
                }
            }
            Item::ExternBlock(block) => {
                // Register extern functions for type checking
                for func in &block.funcs {
                    let params: Vec<Type> = func.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                    let ret = func.ret.as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                    self.funcs.insert(func.name.clone(), (params, ret));
                }
            }
        }
    }

    fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Name(name, args) => {
                if let Some(aliased) = self.aliases.get(name) {
                    // Simple aliases do not carry generic args in v0.2
                    aliased.clone()
                } else if let Some(inner_ty) = self.newtypes.get(name) {
                    // This is a newtype - wrap the resolved inner type in Type::Newtype with name
                    Type::Newtype(name.clone(), Box::new(self.resolve_type(inner_ty)))
                } else {
                    Type::Name(name.clone(), args.clone())
                }
            }
            Type::Ref(inner) => Type::Ref(Box::new(self.resolve_type(inner))),
            Type::RefMut(inner) => Type::RefMut(Box::new(self.resolve_type(inner))),
            Type::Option(inner) => Type::Option(Box::new(self.resolve_type(inner))),
            Type::Result(ok, err) => Type::Result(
                Box::new(self.resolve_type(ok)),
                Box::new(self.resolve_type(err)),
            ),
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| self.resolve_type(e)).collect()),
            Type::Func(args, ret) => Type::Func(
                args.iter().map(|a| self.resolve_type(a)).collect(),
                Box::new(self.resolve_type(ret)),
            ),
            Type::Cap(_) | Type::Shared(_) | Type::LocalShared(_) | Type::Weak(_) => ty.clone(),
            Type::Newtype(name, inner) => Type::Newtype(name.clone(), Box::new(self.resolve_type(inner))),
            Type::Nothing => Type::Nothing,
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Func(f) => {
                // Strict mode: check commitment locks
                if self.strict {
                    self.check_commitment_locks(f.name.as_str(), f.commitment, &f.body);
                }
                self.check_func(f)
            }
            Item::Module(m) => {
                for inner in &m.items {
                    self.check_item(inner);
                }
            }
            Item::Actor(actor) => {
                // Check actor fields
                for field in &actor.fields {
                    let field_ty = self.resolve_type(&field.ty);
                    // Validate field type is well-formed
                    if let Type::Name(name, args) = &field_ty {
                        // Check that the type exists (unless it's a built-in)
                        if !Self::is_builtin_type(name) && !self.types.contains_key(name) {
                            self.emit(format!("unknown type '{}' in actor field '{}'", name, field.name));
                        }
                        // Also check type arguments
                        for arg in args {
                            if let Type::Name(arg_name, _) = arg {
                                if !Self::is_builtin_type(arg_name) && !self.types.contains_key(arg_name) {
                                    self.emit(format!("unknown type '{}' in actor field type", arg_name));
                                }
                            }
                        }
                    }
                    // Check field initialization if present
                    if let Some(init) = &field.init {
                        let init_ty = self.infer_expr(init, &mut vec![HashMap::new()]);
                        if !same_type(&field_ty, &init_ty) {
                            self.emit(format!(
                                "actor field '{}' initializer type {} does not match field type {}",
                                field.name,
                                fmt_type(&init_ty),
                                fmt_type(&field_ty)
                            ));
                        }
                    }
                }
                // Check actor methods
                for method in &actor.methods {
                    // Add implicit self parameter to scope for actor methods
                    let self_ty = Type::Name(actor.name.clone(), vec![]);
                    let mut scopes: Vec<HashMap<String, Type>> = vec![HashMap::new()];
                    scopes[0].insert("self".to_string(), self_ty);
                    // Add other params
                    for p in &method.params {
                        let ty = self.resolve_type(&p.ty);
                        scopes[0].insert(p.name.clone(), ty);
                    }
                    // Check block with self in scope
                    let ret = method
                        .ret
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                    self.cap_vars.push(HashMap::new());
                    self.check_block(&method.body, &ret, &mut scopes);
                    self.check_unconsumed_caps();
                    self.cap_vars.pop();
                }
            }
            Item::Type(_) | Item::Cap(_) => {}
            Item::Rule(_) | Item::Desc(_) => {}
            Item::Trait(trait_def) => {
                // Check that all trait method types are well-formed
                for method in &trait_def.methods {
                    for param in &method.params {
                        let resolved = self.resolve_type(&param.ty);
                        self.check_type_well_formed(&resolved, &format!("trait '{}' method '{}'", trait_def.name, method.name));
                    }
                    if let Some(ret) = &method.ret {
                        let resolved = self.resolve_type(ret);
                        self.check_type_well_formed(&resolved, &format!("trait '{}' method '{}' return", trait_def.name, method.name));
                    }
                }
            }
            Item::Impl(impl_def) => {
                // Check that the trait exists
                if !self.traits.contains_key(&impl_def.trait_name) {
                    self.emit(format!("undefined trait '{}'", impl_def.trait_name));
                }
                // Check that the type exists
                if !self.types.contains_key(&impl_def.type_name) && !Self::is_builtin_type(&impl_def.type_name) {
                    self.emit(format!("undefined type '{}'", impl_def.type_name));
                }
                // Check that all required trait methods are implemented
                if let Some(required_methods) = self.traits.get(&impl_def.trait_name).cloned() {
                    let implemented: Vec<String> = impl_def.methods.iter().map(|m| m.name.clone()).collect();
                    for required in &required_methods {
                        if !implemented.contains(required) {
                            self.emit(format!(
                                "missing method '{}' in impl of trait '{}' for '{}'",
                                required, impl_def.trait_name, impl_def.type_name
                            ));
                        }
                    }
                }
                // Check impl method bodies
                for method in &impl_def.methods {
                    self.check_func(method);
                }
            }
            Item::ExternBlock(_) => {
                // Extern blocks are collected but not type-checked in v1.1
            }
        }
    }

    fn is_builtin_type(name: &str) -> bool {
        matches!(name, "i32" | "i64" | "f64" | "bool" | "string" | "unit" | "List" | "Future" | "Result" | "Option")
    }

    fn check_type_well_formed(&mut self, ty: &Type, context: &str) {
        match ty {
            Type::Name(name, args) => {
                if !Self::is_builtin_type(name) && !self.types.contains_key(name) {
                    self.emit(format!("unknown type '{}' in {}", name, context));
                }
                for arg in args {
                    self.check_type_well_formed(arg, context);
                }
            }
            Type::Ref(inner) | Type::RefMut(inner) | Type::Option(inner) | Type::Shared(inner) | Type::LocalShared(inner) | Type::Weak(inner) => {
                self.check_type_well_formed(inner, context);
            }
            Type::Result(ok, err) => {
                self.check_type_well_formed(ok, context);
                self.check_type_well_formed(err, context);
            }
            Type::Tuple(elems) => {
                for elem in elems {
                    self.check_type_well_formed(elem, context);
                }
            }
            Type::Func(args, ret) => {
                for arg in args {
                    self.check_type_well_formed(arg, context);
                }
                self.check_type_well_formed(ret, context);
            }
            Type::Newtype(name, inner) => {
                if !self.types.contains_key(name) && !self.newtypes.contains_key(name) {
                    self.emit(format!("unknown newtype '{}' in {}", name, context));
                }
                self.check_type_well_formed(inner, context);
            }
            Type::Cap(_) | Type::Nothing => {}
        }
    }

    /// Check if a type implements a trait
    fn type_implements_trait(&self, ty: &Type, trait_name: &str) -> bool {
        match ty {
            Type::Name(type_name, _) => {
                self.impls.contains_key(&(trait_name.to_string(), type_name.clone()))
            }
            _ => false,
        }
    }

    fn check_func(&mut self, func: &FuncDef) {
        let ret = func
            .ret
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
        let mut scopes: Vec<HashMap<String, Type>> = vec![HashMap::new()];
        // Push cap scope for function body
        self.cap_vars.push(HashMap::new());
        for p in &func.params {
            let ty = self.resolve_type(&p.ty);
            // If param is a cap type, track it
            if matches!(&ty, Type::Cap(_)) {
                self.cap_vars.last_mut().unwrap().insert(p.name.clone(), false);
            }
            scopes[0].insert(p.name.clone(), ty);
        }
        // Comptime functions: type-check body but mark as compile-time evaluable
        if func.is_comptime {
            // Comptime functions can only use pure expressions (no side effects)
            // For now, just type-check the body normally
        }
        // Check all-return-paths requirement
        if !matches!(&ret, Type::Name(n, _) if n == "unit") && !self.block_returns_on_all_paths(&func.body) {
            self.emit(format!(
                "function '{}' does not return on all paths (missing return in some branches)",
                func.name
            ));
        }
        self.check_block(&func.body, &ret, &mut scopes);
        // Check for unconsumed caps before popping
        self.check_unconsumed_caps();
        self.cap_vars.pop();
    }

    /// Check if a block returns on all paths
    fn block_returns_on_all_paths(&self, block: &Block) -> bool {
        if block.is_empty() {
            return false;
        }
        // Check if the last statement is an implicit return (expression statement)
        if let Some(last) = block.last() {
            match last {
                Stmt::Return(_) => return true,
                Stmt::Expr(_) => return true, // implicit return via last expression
                Stmt::If { then_, else_, .. } => {
                    let then_returns = self.block_returns_on_all_paths(then_);
                    let else_returns = else_.as_ref()
                        .map(|e| self.block_returns_on_all_paths(e))
                        .unwrap_or(false);
                    if then_returns && else_returns {
                        return true;
                    }
                }
                Stmt::Block(inner) => {
                    if self.block_returns_on_all_paths(inner) {
                        return true;
                    }
                }
                Stmt::Arena(inner) => {
                    if self.block_returns_on_all_paths(inner) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn check_unconsumed_caps(&mut self) {
        if let Some(scope) = self.cap_vars.last() {
            let unconsumed: Vec<String> = scope.iter()
                .filter(|(_, consumed)| !*consumed)
                .map(|(name, _)| name.clone())
                .collect();
            for name in unconsumed {
                self.emit(format!(
                    "linear capability '{}' must be consumed (via drop) before end of scope",
                    name
                ));
            }
        }
    }

    /// Check commitment locks in strict mode
    fn check_commitment_locks(&mut self, name: &str, commitment: Commitment, body: &Block) {
        match commitment {
            Commitment::StrongLocked | Commitment::StrongLockedQuestion | Commitment::StrongLockedQuestionQuestion => {
                // $$ locked: any modification to the function body is an error
                // Check for mms blocks that contain modified contracts
                for stmt in body {
                    if let Stmt::MmsBlock(text) = stmt {
                        if text.contains("requires:") || text.contains("ensures:") || text.contains("math:") {
                            // In strict mode, $$ locked functions should not have their contracts changed
                            // For now, just warn that this is a $$ locked function
                            self.emit(format!(
                                "strict mode: function '{}' is $$ locked - contract modifications not allowed",
                                name
                            ));
                        }
                    }
                }
            }
            Commitment::Locked | Commitment::LockedQuestion | Commitment::LockedQuestionQuestion => {
                // $ locked: warn about modifications
                for stmt in body {
                    if let Stmt::MmsBlock(text) = stmt {
                        if text.contains("requires:") || text.contains("ensures:") || text.contains("math:") {
                            // Just note it's locked
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn check_block(&mut self, block: &Block, ret: &Type, scopes: &mut Vec<HashMap<String, Type>>) {
        // Push cap scope and borrow scope for block
        self.cap_vars.push(HashMap::new());
        self.push_borrow_scope();
        let mut seen_return = false;
        for stmt in block {
            // Unreachable code detection
            if seen_return {
                self.emit("unreachable statement after return".to_string());
                break;
            }
            if let Stmt::Return(_) = stmt { seen_return = true; }
            self.check_stmt(stmt, ret, scopes);
        }
        // Check for unconsumed caps before popping
        self.check_unconsumed_caps();
        self.pop_borrow_scope();
        self.cap_vars.pop();
    }

    /// Check that a statement doesn't capture local_shared variables from outer scope
    fn check_stmt_parasteps_safe(&mut self, stmt: &Stmt, scopes: &mut Vec<HashMap<String, Type>>) {
        match stmt {
            Stmt::Expr(e) | Stmt::Return(Some(e)) => {
                self.check_expr_parasteps_safe(e, scopes);
            }
            Stmt::Let { init: Some(e), .. } => {
                self.check_expr_parasteps_safe(e, scopes);
            }
            Stmt::Assign { target, value } => {
                self.check_expr_parasteps_safe(target, scopes);
                self.check_expr_parasteps_safe(value, scopes);
            }
            Stmt::If { cond, then_, else_ } => {
                self.check_expr_parasteps_safe(cond, scopes);
                for s in then_ {
                    self.check_stmt_parasteps_safe(s, scopes);
                }
                if let Some(else_) = else_ {
                    for s in else_ {
                        self.check_stmt_parasteps_safe(s, scopes);
                    }
                }
            }
            Stmt::While { cond, body } => {
                self.check_expr_parasteps_safe(cond, scopes);
                for s in body {
                    self.check_stmt_parasteps_safe(s, scopes);
                }
            }
            Stmt::For { iterable, body, .. } => {
                self.check_expr_parasteps_safe(iterable, scopes);
                for s in body {
                    self.check_stmt_parasteps_safe(s, scopes);
                }
            }
            _ => {}
        }
    }

    /// Check that an expression doesn't reference local_shared variables
    fn check_expr_parasteps_safe(&mut self, expr: &Expr, scopes: &mut Vec<HashMap<String, Type>>) {
        match expr {
            Expr::Ident(name) => {
                // Check if this variable is local_shared from outer scope
                for scope in scopes.iter().rev() {
                    if let Some(ty) = scope.get(name) {
                        if matches!(ty, Type::LocalShared(_)) {
                            self.emit(format!(
                                "cannot capture 'local_shared' variable '{}' in parallel block (use 'shared' instead)",
                                name
                            ));
                        }
                        break;
                    }
                }
            }
            Expr::Binary(_, l, r) => {
                self.check_expr_parasteps_safe(l, scopes);
                self.check_expr_parasteps_safe(r, scopes);
            }
            Expr::Unary(_, e) => {
                self.check_expr_parasteps_safe(e, scopes);
            }
            Expr::Call(callee, args) => {
                self.check_expr_parasteps_safe(callee, scopes);
                for arg in args {
                    self.check_expr_parasteps_safe(arg, scopes);
                }
            }
            Expr::Field(obj, _) => {
                self.check_expr_parasteps_safe(obj, scopes);
            }
            Expr::Index(obj, idx) => {
                self.check_expr_parasteps_safe(obj, scopes);
                self.check_expr_parasteps_safe(idx, scopes);
            }
            Expr::List(elems) => {
                for e in elems {
                    self.check_expr_parasteps_safe(e, scopes);
                }
            }
            Expr::Tuple(elems) => {
                for e in elems {
                    self.check_expr_parasteps_safe(e, scopes);
                }
            }
            _ => {}
        }
    }

    fn check_stmt(
        &mut self,
        stmt: &Stmt,
        ret: &Type,
        scopes: &mut Vec<HashMap<String, Type>>,
    ) {
        match stmt {
            Stmt::Let { pat, ty, init, mut_, ref_ } => {
                // Shadowing detection
                if let Pattern::Variable(name) = pat {
                    for scope in self.var_scopes.iter().rev() {
                        if scope.contains_key(name) {
                            self.emit(format!("variable '{}' shadows an outer variable", name));
                            break;
                        }
                    }
                    self.var_scopes.last_mut().unwrap().insert(name.clone(), 0);
                }

                let init_ty = init
                    .as_ref()
                    .map(|e| self.infer_expr(e, scopes))
                    .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                let declared = ty.as_ref().map(|t| self.resolve_type(t));
                let final_ty = match declared {
                    Some(d) => {
                        if !same_type(&d, &init_ty) {
                            self.emit(format!(
                                "pattern declared as {} but initialized with {}",
                                fmt_type(&d),
                                fmt_type(&init_ty)
                            ));
                        }
                        d
                    }
                    None => {
                        if *ref_ {
                            // ref variables have reference type
                            Type::Ref(Box::new(init_ty))
                        } else {
                            init_ty
                        }
                    }
                };
                // Track mutability
                if let Pattern::Variable(name) = pat {
                    self.mut_vars.last_mut().unwrap().insert(name.clone(), *mut_);
                }
                self.check_pattern(pat, &final_ty, scopes);
                // Track cap variables for linear type checking and introduce effects
                if let Type::Cap(cap_name) = &final_ty {
                    if let Pattern::Variable(name) = pat {
                        self.cap_vars.last_mut().unwrap().insert(name.clone(), false);
                        // Introduce the cap as an effect
                        self.available_effects.last_mut().unwrap().insert(cap_name.clone(), true);
                    }
                }
            }
            Stmt::Return(None) => {
                if !same_type(ret, &Type::Name("unit".into(), vec![])) {
                    self.emit(format!(
                        "expected return value of type {}, found unit",
                        fmt_type(ret)
                    ));
                }
            }
            Stmt::Return(Some(e)) => {
                let t = self.infer_expr(e, scopes);
                if !same_type(ret, &t) {
                    self.emit(format!(
                        "return type mismatch: expected {}, found {}",
                        fmt_type(ret),
                        fmt_type(&t)
                    ));
                }
            }
            Stmt::Expr(e) => {
                self.infer_expr(e, scopes);
            }
            Stmt::If { cond, then_, else_ } => {
                let ct = self.infer_expr(cond, scopes);
                if !is_bool(&ct) {
                    self.emit(format!(
                        "if condition must be bool, found {}",
                        fmt_type(&ct)
                    ));
                }
                self.check_block(then_, ret, scopes);
                if let Some(else_) = else_ {
                    self.check_block(else_, ret, scopes);
                }
            }
            Stmt::While { cond, body } => {
                let ct = self.infer_expr(cond, scopes);
                if !is_bool(&ct) {
                    self.emit(format!(
                        "while condition must be bool, found {}",
                        fmt_type(&ct)
                    ));
                }
                self.check_block(body, ret, scopes);
            }
            Stmt::For { var, iterable, body } => {
                let it = self.infer_expr(iterable, scopes);
                let elem_ty = match it {
                    Type::Name(n, args) if n == "List" && args.len() == 1 => args[0].clone(),
                    _ => {
                        self.emit(format!(
                            "for loop requires a List, found {}",
                            fmt_type(&it)
                        ));
                        Type::Name("unknown".into(), vec![])
                    }
                };
                scopes.push(HashMap::new());
                scopes.last_mut().unwrap().insert(var.clone(), elem_ty);
                self.check_block(body, ret, scopes);
                scopes.pop();
            }
            Stmt::Block(block) => {
                scopes.push(HashMap::new());
                self.check_block(block, ret, scopes);
                scopes.pop();
            }
            Stmt::Arena(block) => {
                // Arena block is like a scope with special memory semantics
                // For now, just check the block contents
                scopes.push(HashMap::new());
                self.check_block(block, ret, scopes);
                scopes.pop();
            }
            Stmt::SharedLet { kind, name, ty, init } => {
                let init_ty = self.infer_expr(init, scopes);
                let final_ty = match kind {
                    SharedKind::Shared => Type::Shared(Box::new(init_ty.clone())),
                    SharedKind::LocalShared => Type::LocalShared(Box::new(init_ty.clone())),
                    SharedKind::Weak => {
                        // Expect init to be a Shared value
                        match &init_ty {
                            Type::Shared(inner) => Type::Weak(inner.clone()),
                            _ => {
                                self.emit(format!(
                                    "weak requires a shared value, found {}",
                                    fmt_type(&init_ty)
                                ));
                                Type::Weak(Box::new(Type::Name("unknown".into(), vec![])))
                            }
                        }
                    }
                    SharedKind::WeakLocal => {
                        match &init_ty {
                            Type::LocalShared(inner) => Type::Weak(inner.clone()),
                            _ => {
                                self.emit(format!(
                                    "weak_local requires a local_shared value, found {}",
                                    fmt_type(&init_ty)
                                ));
                                Type::Weak(Box::new(Type::Name("unknown".into(), vec![])))
                            }
                        }
                    }
                };
                if let Some(declared) = ty {
                    let declared = self.resolve_type(declared);
                    if !same_type(&declared, &final_ty) {
                        self.emit(format!(
                            "shared binding declared as {} but inferred as {}",
                            fmt_type(&declared),
                            fmt_type(&final_ty)
                        ));
                    }
                }
                scopes.last_mut().unwrap().insert(name.clone(), final_ty);
            }
            Stmt::Parasteps(block) => {
                // Parasteps block executes statements in parallel
                // Check that no local_shared variables are captured from outer scope
                for stmt in block {
                    self.check_stmt_parasteps_safe(stmt, scopes);
                }
                // Then type-check all statements
                scopes.push(HashMap::new());
                self.check_block(block, ret, scopes);
                scopes.pop();
            }
            Stmt::Assign { target, value } => {
                let value_ty = self.infer_expr(value, scopes);
                match target {
                    Expr::Ident(name) => {
                        // Check mutability
                        let is_mut = self.mut_vars.iter().rev().any(|scope| {
                            scope.get(name).copied().unwrap_or(false)
                        });
                        if !is_mut {
                            self.emit(format!("cannot assign to immutable variable '{}' (use 'let mut')", name));
                        }
                        let target_ty = self.lookup_var(name, scopes);
                        if !same_type(&target_ty, &value_ty) {
                            self.emit(format!(
                                "cannot assign {} to variable '{}' of type {}",
                                fmt_type(&value_ty),
                                name,
                                fmt_type(&target_ty)
                            ));
                        }
                    }
                    Expr::Unary(UnOp::Deref, inner) => {
                        // *r = value: check that inner is &mut T
                        let inner_ty = self.infer_expr(inner, scopes);
                        match &inner_ty {
                            Type::RefMut(inner_inner) => {
                                if !same_type(&value_ty, inner_inner) {
                                    self.emit(format!(
                                        "cannot assign {} through &mut reference of type {}",
                                        fmt_type(&value_ty),
                                        fmt_type(&inner_ty)
                                    ));
                                }
                            }
                            _ => {
                                self.emit(format!(
                                    "cannot assign through non-mutable reference {}",
                                    fmt_type(&inner_ty)
                                ));
                            }
                        }
                    }
                    Expr::Field(obj, field) => {
                        // Field assignment: check that the object type has that field
                        let obj_ty = self.infer_expr(obj, scopes);
                        // For now just allow it - the type checker will verify field exists
                        let _ = (obj_ty, field);
                    }
                    _ => self.emit("assignment target must be a variable"),
                }
            }
            Stmt::Drop(expr) => {
                // Evaluate the expression to ensure it's valid
                self.infer_expr(expr, scopes);
                // Mark the capability as consumed
                if let Expr::Ident(name) = expr {
                    if let Some(consumed) = self.cap_vars.last_mut().unwrap().get_mut(name) {
                        if *consumed {
                            self.emit(format!(
                                "capability '{}' has already been consumed",
                                name
                            ));
                        } else {
                            *consumed = true;
                        }
                    }
                }
            }
            Stmt::Desc(_) | Stmt::Requires(_) | Stmt::Ensures(_) | Stmt::Math(_) | Stmt::Ellipsis | Stmt::OnFailure(_) | Stmt::MmsBlock(_) => {}
        }
    }

    fn infer_expr(&mut self, expr: &Expr, scopes: &mut Vec<HashMap<String, Type>>) -> Type {
        match expr {
            Expr::Literal(l) => match l {
                Lit::Int(_) => Type::Name("i32".into(), vec![]),
                Lit::Float(_) => Type::Name("f64".into(), vec![]),
                Lit::Bool(_) => Type::Name("bool".into(), vec![]),
                Lit::String(_) => Type::Name("string".into(), vec![]),
                Lit::FString(_) => Type::Name("string".into(), vec![]),
                Lit::Unit => Type::Name("unit".into(), vec![]),
            },
            Expr::Ident(name) => self.lookup_var(name, scopes),
            Expr::Unary(op, e) => {
                let t = self.infer_expr(e, scopes);
                match op {
                    UnOp::Neg => {
                        if is_numeric(&t) {
                            t
                        } else {
                            self.emit(format!("cannot negate {}", fmt_type(&t)));
                            Type::Name("unknown".into(), vec![])
                        }
                    }
                    UnOp::Not => {
                        if is_bool(&t) {
                            t
                        } else {
                            self.emit(format!("cannot apply ! to {}", fmt_type(&t)));
                            Type::Name("unknown".into(), vec![])
                        }
                    }
                    UnOp::Ref => {
                        // Check borrow rules: cannot borrow if already mutably borrowed
                        if let Expr::Ident(name) = e.as_ref() {
                            if let Some(BorrowState::BorrowedMut) = self.lookup_borrow(name) {
                                self.emit(format!("cannot borrow '{}' as immutable because it is already mutably borrowed", name));
                            }
                            self.set_borrow(name, BorrowState::BorrowedImm);
                        }
                        Type::Ref(Box::new(t))
                    }
                    UnOp::RefMut => {
                        // Check borrow rules: cannot &mut if already borrowed (imm or mut)
                        if let Expr::Ident(name) = e.as_ref() {
                            if let Some(state) = self.lookup_borrow(name) {
                                match state {
                                    BorrowState::Unborrowed => {}
                                    BorrowState::BorrowedImm => {
                                        self.emit(format!("cannot borrow '{}' as mutable because it is already immutably borrowed", name));
                                    }
                                    BorrowState::BorrowedMut => {
                                        self.emit(format!("cannot borrow '{}' as mutable because it is already mutably borrowed", name));
                                    }
                                }
                            }
                            self.set_borrow(name, BorrowState::BorrowedMut);
                        }
                        Type::RefMut(Box::new(t))
                    }
                    UnOp::Deref => {
                        match &t {
                            Type::Ref(inner) | Type::RefMut(inner) => (**inner).clone(),
                            _ => {
                                self.emit(format!("cannot dereference {}", fmt_type(&t)));
                                Type::Name("unknown".into(), vec![])
                            }
                        }
                    }
                }
            }
            Expr::Binary(op, l, r) => self.infer_binary(*op, l, r, scopes),
            Expr::Call(callee, args) => {
                match callee.as_ref() {
                    Expr::Ident(name) => self.check_call(name, args, scopes),
                    Expr::Field(obj, method_name) => {
                        // Method call: obj.method(args) or Type.spawn(args)
                        let obj_ty = self.infer_expr(obj, scopes);
                        if let Type::Name(type_name, _) = &obj_ty {
                            // Check if it's an actor spawn call (Type.spawn)
                            if method_name == "spawn" {
                                return Type::Name(type_name.clone(), vec![]);
                            }
                            // Check module-qualified function call: Module::func(args)
                            let qualified_func = format!("{}::{}", type_name, method_name);
                            if self.funcs.contains_key(&qualified_func) {
                                return self.check_call(&qualified_func, args, scopes);
                            }
                            // Check record field access (field is a closure/function)
                            if let Some(tdef) = self.types.get(type_name) {
                                if let TypeDefKind::Record(fields) = &tdef.kind {
                                    if let Some(f) = fields.iter().find(|f| f.name == *method_name) {
                                        // Field access that returns a callable — just return the field type
                                        return self.resolve_type(&f.ty);
                                    }
                                }
                            }
                            // Check trait methods on this type
                            if let Some(methods) = self.type_methods.get(type_name) {
                                if let Some((trait_name, _)) = methods.iter().find(|(_, m)| m == method_name) {
                                    let trait_name = trait_name.clone();
                                    if let Some((params, ret)) = self.trait_method_sigs.get(&(trait_name.clone(), method_name.clone())).cloned() {
                                        // Validate arguments (skip first param which is self)
                                        let user_args = &args;
                                        let method_params = if !params.is_empty() { &params[1..] } else { &params };
                                        if user_args.len() != method_params.len() {
                                            self.emit(format!(
                                                "method '{}' of trait '{}' expects {} arguments, got {}",
                                                method_name, trait_name, method_params.len(), user_args.len()
                                            ));
                                        } else {
                                            for (i, (arg, param)) in user_args.iter().zip(method_params.iter()).enumerate() {
                                                let at = self.infer_expr(arg, scopes);
                                                if !same_type(&at, param) {
                                                    self.emit(format!(
                                                        "argument {} of method '{}' expected {}, found {}",
                                                        i + 1, method_name, fmt_type(param), fmt_type(&at)
                                                    ));
                                                }
                                            }
                                        }
                                        return ret;
                                    }
                                }
                            }
                            // Check if the type has this as a direct method (actor methods)
                            if let Some(actor_def) = self.file.items.iter().find_map(|item| {
                                if let Item::Actor(a) = item { if a.name == *type_name { Some(a) } else { None } } else { None }
                            }) {
                                if let Some(method) = actor_def.methods.iter().find(|m| m.name == *method_name) {
                                    let ret = method.ret.as_ref()
                                        .map(|t| self.resolve_type(t))
                                        .unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                                    return ret;
                                }
                            }
                            self.emit(format!("type '{}' has no method '{}'", type_name, method_name));
                            Type::Name("unknown".into(), vec![])
                        } else {
                            self.emit(format!("method call requires a named type, found {}", fmt_type(&obj_ty)));
                            Type::Name("unknown".into(), vec![])
                        }
                    }
                    _ => {
                        self.emit("callee must be a function name");
                        Type::Name("unknown".into(), vec![])
                    }
                }
            }
            Expr::Tuple(elems) => {
                Type::Tuple(elems.iter().map(|e| self.infer_expr(e, scopes)).collect())
            }
            Expr::List(elems) => {
                let mut elem_ty = Type::Name("unknown".into(), vec![]);
                for (i, e) in elems.iter().enumerate() {
                    let t = self.infer_expr(e, scopes);
                    if i == 0 {
                        elem_ty = t;
                    } else if !same_type(&elem_ty, &t) {
                        self.emit(format!(
                            "list element {} type {} does not match first element {}",
                            i + 1,
                            fmt_type(&t),
                            fmt_type(&elem_ty)
                        ));
                    }
                }
                Type::Name("List".into(), vec![elem_ty])
            }
            Expr::Comprehension { expr, var, iter, guard } => {
                let iter_ty = self.infer_expr(iter, scopes);
                // Check iter is a list
                if let Type::Name(n, args) = &iter_ty {
                    if n != "List" || args.len() != 1 {
                        self.emit(format!("comprehension requires a list, found {}", fmt_type(&iter_ty)));
                    }
                }
                // Infer element type from iter
                let elem_ty = if let Type::Name(_, args) = &iter_ty {
                    if args.len() == 1 { args[0].clone() } else { Type::Name("unknown".into(), vec![]) }
                } else {
                    Type::Name("unknown".into(), vec![])
                };
                // Add var to scope
                scopes.last_mut().unwrap().insert(var.clone(), elem_ty);
                // Infer expression type
                let expr_ty = self.infer_expr(expr, scopes);
                // Check guard if present
                if let Some(g) = guard {
                    let guard_ty = self.infer_expr(g, scopes);
                    if !matches!(&guard_ty, Type::Name(n, _) if n == "bool") {
                        self.emit(format!("comprehension guard must be bool, found {}", fmt_type(&guard_ty)));
                    }
                }
                Type::Name("List".into(), vec![expr_ty])
            }
            Expr::Match(subject, arms) => {
                let subject_ty = self.infer_expr(subject, scopes);
                if arms.is_empty() {
                    self.emit("match expression must have at least one arm");
                    return Type::Name("unknown".into(), vec![]);
                }

                // Get all variants of the subject type for exhaustiveness checking
                let all_variants = self.get_enum_variants(&subject_ty);

                // Track which variants are covered by match arms
                let mut covered_variants: Vec<String> = Vec::new();
                let mut has_catchall = false;
                // Track if any arm has a guard - guards make exhaustiveness checking unreliable
                let mut has_guard = false;

                let mut result_ty: Option<Type> = None;
                for arm in arms {
                    // Check pattern coverage
                    let (pattern_covered, is_catchall) = self.pattern_covers_variants(&arm.pat, &subject_ty);
                    if is_catchall {
                        has_catchall = true;
                    }
                    for variant in pattern_covered {
                        if !covered_variants.contains(&variant) {
                            covered_variants.push(variant);
                        }
                    }

                    scopes.push(HashMap::new());
                    self.check_pattern(&arm.pat, &subject_ty, scopes);
                    if let Some(guard) = &arm.guard {
                        has_guard = true;
                        let gt = self.infer_expr(guard, scopes);
                        if !is_bool(&gt) {
                            self.emit(format!(
                                "match guard must be bool, found {}",
                                fmt_type(&gt)
                            ));
                        }
                    }
                    let body_ty = self.infer_expr(&arm.body, scopes);
                    scopes.pop();
                    match &result_ty {
                        None => result_ty = Some(body_ty),
                        Some(rt) => {
                            if !same_type(rt, &body_ty) {
                                self.emit(format!(
                                    "match arm body type {} does not match previous {}",
                                    fmt_type(&body_ty),
                                    fmt_type(rt)
                                ));
                            }
                        }
                    }
                }

                // Check exhaustiveness: all variants must be covered
                // Skip if: no enum variants, has catchall, or any arm has a guard (undecidable)
                if !all_variants.is_empty() && !has_catchall && !has_guard {
                    for variant in &all_variants {
                        if !covered_variants.contains(variant) {
                            self.emit(format!(
                                "match expression is not exhaustive: missing variant '{}' of '{}'",
                                variant,
                                fmt_type(&subject_ty)
                            ));
                        }
                    }
                }

                result_ty.unwrap_or_else(|| Type::Name("unknown".into(), vec![]))
            }
            Expr::Field(obj, field) => {
                let obj_ty = self.infer_expr(obj, scopes);
                match &obj_ty {
                    Type::Name(name, _) => {
                        // Check if it's an actor type
                        if let Some(actor_def) = self.file.items.iter().find_map(|item| {
                            if let Item::Actor(a) = item {
                                if a.name == *name { Some(a) } else { None }
                            } else { None }
                        }) {
                            // Actor field access
                            if let Some(f) = actor_def.fields.iter().find(|f| f.name == *field) {
                                self.resolve_type(&f.ty)
                            } else {
                                self.emit(format!(
                                    "actor '{}' has no field '{}'",
                                    name, field
                                ));
                                Type::Name("unknown".into(), vec![])
                            }
                        } else if let Some(tdef) = self.types.get(name) {
                            match &tdef.kind {
                                TypeDefKind::Record(fields) => {
                                    if let Some(f) = fields.iter().find(|f| f.name == *field) {
                                        self.resolve_type(&f.ty)
                                    } else if let Some(methods) = self.type_methods.get(name) {
                                        if let Some((trait_name, _)) = methods.iter().find(|(_, m)| m == field) {
                                            let trait_name = trait_name.clone();
                                            if let Some((params, ret)) = self.trait_method_sigs.get(&(trait_name, field.clone())).cloned() {
                                                Type::Func(params, Box::new(ret))
                                            } else {
                                                Type::Name("unknown".into(), vec![])
                                            }
                                        } else {
                                            self.emit(format!(
                                                "type '{}' has no field '{}'",
                                                name, field
                                            ));
                                            Type::Name("unknown".into(), vec![])
                                        }
                                    } else {
                                        self.emit(format!(
                                            "type '{}' has no field '{}'",
                                            name, field
                                        ));
                                        Type::Name("unknown".into(), vec![])
                                    }
                                }
                                _ => {
                                    // Check trait methods for non-record types
                                    if let Some(methods) = self.type_methods.get(name) {
                                        if let Some((trait_name, _)) = methods.iter().find(|(_, m)| m == field) {
                                            let trait_name = trait_name.clone();
                                            if let Some((params, ret)) = self.trait_method_sigs.get(&(trait_name, field.clone())).cloned() {
                                                Type::Func(params, Box::new(ret))
                                            } else {
                                                Type::Name("unknown".into(), vec![])
                                            }
                                        } else {
                                            self.emit(format!("'{}' is not a record type", name));
                                            Type::Name("unknown".into(), vec![])
                                        }
                                    } else {
                                        self.emit(format!("'{}' is not a record type", name));
                                        Type::Name("unknown".into(), vec![])
                                    }
                                }
                            }
                        } else {
                            self.emit(format!("field access on unknown type '{}'", name));
                            Type::Name("unknown".into(), vec![])
                        }
                    }
                    _ => {
                        self.emit(format!(
                            "field access requires a record type, found {}",
                            fmt_type(&obj_ty)
                        ));
                        Type::Name("unknown".into(), vec![])
                    }
                }
            }
            Expr::Record { ty, fields } => {
                let tdef = ty.as_ref().and_then(|n| self.types.get(n)).cloned();
                match tdef {
                    Some(tdef) => {
                        match &tdef.kind {
                            TypeDefKind::Record(expected_fields) => {
                                let expected: HashMap<String, Type> = expected_fields
                                    .iter()
                                    .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                                    .collect();
                                for (name, value) in fields.iter().map(|f| (&f.name, &f.value)) {
                                    if let Some(expected_ty) = expected.get(name) {
                                        let actual_ty = self.infer_expr(value, scopes);
                                        if !same_type(expected_ty, &actual_ty) {
                                            self.emit(format!(
                                                "field '{}' expected {}, found {}",
                                                name,
                                                fmt_type(expected_ty),
                                                fmt_type(&actual_ty)
                                            ));
                                        }
                                    } else {
                                        self.emit(format!(
                                            "type '{}' has no field '{}'",
                                            tdef.name,
                                            name
                                        ));
                                    }
                                }
                                for name in expected.keys() {
                                    if !fields.iter().any(|f| &f.name == name) {
                                        self.emit(format!(
                                            "missing field '{}' in record literal",
                                            name
                                        ));
                                    }
                                }
                                Type::Name(tdef.name.clone(), vec![])
                            }
                            _ => {
                                self.emit(format!("'{}' is not a record type", tdef.name));
                                Type::Name("unknown".into(), vec![])
                            }
                        }
                    }
                    None => {
                        self.emit("cannot infer record type without explicit type name");
                        Type::Name("unknown".into(), vec![])
                    }
                }
            }
            Expr::Index(obj, idx) => {
                let obj_ty = self.infer_expr(obj, scopes);
                let idx_ty = self.infer_expr(idx, scopes);
                if !is_int(&idx_ty) {
                    self.emit(format!("index must be integer, found {}", fmt_type(&idx_ty)));
                }
                match obj_ty {
                    Type::Name(n, args) if n == "List" && args.len() == 1 => args[0].clone(),
                    Type::Name(n, _) if n == "string" => Type::Name("string".into(), vec![]),
                    _ => {
                        self.emit(format!("cannot index {}", fmt_type(&obj_ty)));
                        Type::Name("unknown".into(), vec![])
                    }
                }
            }
            Expr::Try(expr) => {
                let inner_ty = self.infer_expr(expr, scopes);
                match inner_ty {
                    // Built-in Result<T, E> -> ? extracts T
                    Type::Name(n, args) if n == "Result" && args.len() == 2 => {
                        args[0].clone()
                    }
                    // Built-in Option<T> -> ? extracts T
                    Type::Name(n, args) if n == "Option" && args.len() == 1 => {
                        args[0].clone()
                    }
                    // T? syntactic sugar for Option<T>
                    Type::Option(inner) => (*inner).clone(),
                    // For unparameterized enum types like `Res`, look up the type definition
                    Type::Name(name, ref args) if args.is_empty() => {
                        if let Some(tdef) = self.types.get(&name) {
                            match &tdef.kind {
                                TypeDefKind::Enum(variants) if variants.len() == 2 => {
                                    // Try to find Ok/Err or Some/None pattern
                                    let first_variant = &variants[0];
                                    match &first_variant.payload {
                                        Some(VariantPayload::Tuple(types)) if !types.is_empty() => {
                                            types[0].clone()
                                        }
                                        _ => {
                                            self.emit(format!(
                                                "? operator: cannot determine success type from enum '{}'",
                                                name
                                            ));
                                            Type::Name("unknown".into(), vec![])
                                        }
                                    }
                                }
                                _ => {
                                    self.emit(format!(
                                        "? operator requires Result or Option type, found '{}'",
                                        name
                                    ));
                                    Type::Name("unknown".into(), vec![])
                                }
                            }
                        } else {
                            self.emit(format!(
                                "? operator requires Result or Option type, found '{}'",
                                name
                            ));
                            Type::Name("unknown".into(), vec![])
                        }
                    }
                    _ => {
                        self.emit(format!(
                            "? operator requires Result or Option type, found {}",
                            fmt_type(&inner_ty)
                        ));
                        Type::Name("unknown".into(), vec![])
                    }
                }
            }
            Expr::Spawn(_) => {
                // Spawn returns a future/handle type - simplified for now
                Type::Name("Future".into(), vec![])
            }
            Expr::Await(inner) => {
                // Await unwraps the future type
                let inner_ty = self.infer_expr(inner, scopes);
                // For now, just return the inner type
                match inner_ty {
                    Type::Name(n, args) if n == "Future" && !args.is_empty() => args[0].clone(),
                    other => other,
                }
            }
            Expr::Quote(_) | Expr::QuoteInterpolate(_) => {
                // quote! returns an AST value
                Type::Name("AST".into(), vec![])
            }
            Expr::Comptime(block) => {
                // Comptime block: infer type from last expression
                let mut result_type = Type::Name("unit".into(), vec![]);
                for stmt in block {
                    match stmt {
                        Stmt::Expr(e) => result_type = self.infer_expr(e, scopes),
                        Stmt::Return(Some(e)) => { result_type = self.infer_expr(e, scopes); break; }
                        _ => {}
                    }
                }
                result_type
            }
            Expr::TypeOf(_) => {
                // type_of returns a Type descriptor
                Type::Name("Type".into(), vec![])
            }
            Expr::TypeInfo(_) => {
                // type_info returns a record with type metadata
                Type::Name("TypeInfo".into(), vec![])
            }
            Expr::Old(expr) => {
                // old(x) returns the same type as x
                self.infer_expr(expr, scopes)
            }
            Expr::Lambda { params, ret, .. } => {
                let param_types: Vec<Type> = params.iter().map(|p| p.ty.clone()).collect();
                let return_type = ret.clone().unwrap_or_else(|| Type::Name("unit".into(), vec![]));
                Type::Func(param_types, Box::new(return_type))
            }
            Expr::Turbofish(name, type_args, args) => {
                // Turbofish: func::<Type>(args) — explicit type instantiation
                let (params, ret) = match self.funcs.get(name) {
                    Some(sig) => sig.clone(),
                    None => {
                        self.emit(format!("undefined function '{}'", name));
                        return Type::Name("unknown".into(), vec![]);
                    }
                };
                let generics = self.func_generics.get(name).cloned().unwrap_or_default();

                // Build type param map from turbofish type args
                let mut type_map: HashMap<String, Type> = HashMap::new();
                if !generics.is_empty() && !type_args.is_empty() {
                    if type_args.len() != generics.len() {
                        self.emit(format!(
                            "function '{}' expects {} type arguments, got {}",
                            name,
                            generics.len(),
                            type_args.len()
                        ));
                    } else {
                        for (gp, ta) in generics.iter().zip(type_args.iter()) {
                            type_map.insert(gp.name.clone(), ta.clone());
                        }
                    }
                }

                if args.len() != params.len() {
                    self.emit(format!(
                        "function '{}' expects {} arguments, got {}",
                        name,
                        params.len(),
                        args.len()
                    ));
                } else {
                    // Check where constraints (before substitution)
                    if let Some((type_param, bounds)) = self.where_clauses.get(name).cloned() {
                        for (arg, param) in args.iter().zip(params.iter()) {
                            let at = self.infer_expr(arg, scopes);
                            if self.type_uses_type_param(param, &type_param) {
                                for bound in &bounds {
                                    if !self.type_implements_trait(&at, bound) {
                                        self.emit(format!(
                                            "where constraint violated: type '{}' does not implement trait '{}' (required by function '{}')",
                                            fmt_type(&at),
                                            bound,
                                            name
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // Check arguments with substituted types
                    for (i, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
                        let at = self.infer_expr(arg, scopes);
                        let subst_param = if !type_map.is_empty() {
                            subst_type_params(param, &generics, &type_map)
                        } else {
                            param.clone()
                        };
                        if !same_type(&at, &subst_param) {
                            self.emit(format!(
                                "argument {} of '{}' expected {}, found {}",
                                i + 1,
                                name,
                                fmt_type(&subst_param),
                                fmt_type(&at)
                            ));
                        }
                    }
                }
                // Substitute type args into return type
                if !type_map.is_empty() {
                    subst_type_params(&ret, &generics, &type_map)
                } else {
                    ret
                }
            }
        }
    }

    fn check_pattern(
        &mut self,
        pat: &Pattern,
        subject: &Type,
        scopes: &mut Vec<HashMap<String, Type>>,
    ) {
        match pat {
            Pattern::Wildcard => {}
            Pattern::Variable(name) => {
                scopes.last_mut().unwrap().insert(name.clone(), subject.clone());
            }
            Pattern::Literal(l) => {
                let lit_ty = match l {
                    Lit::Int(_) => Type::Name("i32".into(), vec![]),
                    Lit::Float(_) => Type::Name("f64".into(), vec![]),
                    Lit::Bool(_) => Type::Name("bool".into(), vec![]),
                    Lit::String(_) => Type::Name("string".into(), vec![]),
                    Lit::FString(_) => Type::Name("string".into(), vec![]),
                    Lit::Unit => Type::Name("unit".into(), vec![]),
                };
                if !same_type(subject, &lit_ty) {
                    self.emit(format!(
                        "pattern literal type {} does not match subject {}",
                        fmt_type(&lit_ty),
                        fmt_type(subject)
                    ));
                }
            }
            Pattern::Constructor(name, pats) => {
                let def = self.types.values().find(|t| {
                    match &t.kind {
                        TypeDefKind::Enum(variants) => variants.iter().any(|v| v.name == *name),
                        TypeDefKind::Newtype(_) => t.name == *name,
                        _ => false,
                    }
                });
                match def {
                    Some(tdef) => {
                        match &tdef.kind {
                            TypeDefKind::Enum(variants) => {
                                if let Some(variant) = variants.iter().find(|v| v.name == *name) {
                                    match &variant.payload {
                                        None => {
                                            if !pats.is_empty() {
                                                self.emit(format!(
                                                    "variant '{}' takes no arguments",
                                                    name
                                                ));
                                            }
                                        }
                                        Some(VariantPayload::Tuple(types)) => {
                                            let types: Vec<Type> = types.clone();
                                            if pats.len() != types.len() {
                                                self.emit(format!(
                                                    "variant '{}' expects {} arguments, got {}",
                                                    name,
                                                    types.len(),
                                                    pats.len()
                                                ));
                                            } else {
                                                for (p, t) in pats.iter().zip(types.iter()) {
                                                    self.check_pattern(p, &self.resolve_type(t), scopes);
                                                }
                                            }
                                        }
                                        Some(VariantPayload::Record(fields)) => {
                                            if pats.len() != fields.len() {
                                                self.emit(format!(
                                                    "variant '{}' record expects {} fields, got {}",
                                                    name,
                                                    fields.len(),
                                                    pats.len()
                                                ));
                                            } else {
                                                let resolved: Vec<Type> = fields.iter().map(|f| self.resolve_type(&f.ty)).collect();
                                                for (p, t) in pats.iter().zip(resolved.iter()) {
                                                    self.check_pattern(p, t, scopes);
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    self.emit(format!("variant '{}' not found in type '{}'", name, tdef.name));
                                }
                            }
                            TypeDefKind::Newtype(inner) => {
                                if pats.len() != 1 {
                                    self.emit(format!(
                                        "newtype '{}' pattern expects exactly one argument",
                                        name
                                    ));
                                } else {
                                    self.check_pattern(&pats[0], &self.resolve_type(inner), scopes);
                                }
                            }
                            _ => {
                                self.emit(format!("'{}' is not an enum variant", name));
                            }
                        }
                    }
                    None => {
                        self.emit(format!("undefined constructor '{}'", name));
                    }
                }
            }
            Pattern::Tuple(pats) => {
                match subject {
                    Type::Tuple(types) => {
                        if pats.len() != types.len() {
                            self.emit(format!(
                                "tuple pattern expects {} elements, found {}",
                                types.len(),
                                pats.len()
                            ));
                        } else {
                            for (p, t) in pats.iter().zip(types.iter()) {
                                self.check_pattern(p, t, scopes);
                            }
                        }
                    }
                    _ => {
                        self.emit(format!(
                            "cannot match tuple pattern against non-tuple type {}",
                            fmt_type(subject)
                        ));
                    }
                }
            }
        }
    }

    fn infer_binary(
        &mut self,
        op: BinOp,
        l: &Expr,
        r: &Expr,
        scopes: &mut Vec<HashMap<String, Type>>,
    ) -> Type {
        // short-circuit logic
        if op == BinOp::And || op == BinOp::Or {
            let lt = self.infer_expr(l, scopes);
            let rt = self.infer_expr(r, scopes);
            if !is_bool(&lt) || !is_bool(&rt) {
                self.emit(format!(
                    "logical operator requires bool operands, found {} and {}",
                    fmt_type(&lt),
                    fmt_type(&rt)
                ));
            }
            return Type::Name("bool".into(), vec![]);
        }

        let lt = self.infer_expr(l, scopes);
        let rt = self.infer_expr(r, scopes);

        match op {
            BinOp::Add => {
                // String concatenation: string + string -> string
                if is_string(&lt) && is_string(&rt) {
                    Type::Name("string".into(), vec![])
                } else if !same_type(&lt, &rt) || !is_numeric(&lt) {
                    self.emit(format!(
                        "arithmetic operator requires matching numeric types, found {} and {}",
                        fmt_type(&lt),
                        fmt_type(&rt)
                    ));
                    Type::Name("unknown".into(), vec![])
                } else {
                    lt
                }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow => {
                if !same_type(&lt, &rt) || !is_numeric(&lt) {
                    self.emit(format!(
                        "arithmetic operator requires matching numeric types, found {} and {}",
                        fmt_type(&lt),
                        fmt_type(&rt)
                    ));
                    Type::Name("unknown".into(), vec![])
                } else {
                    // Static divide-by-zero detection
                    if op == BinOp::Div || op == BinOp::Mod {
                        if let Expr::Literal(Lit::Int(0)) = r {
                            self.emit(format!("{} by zero literal", if op == BinOp::Div { "division" } else { "modulo" }));
                        }
                    }
                    lt
                }
            }
            BinOp::Mod | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if !same_type(&lt, &rt) || !is_int(&lt) {
                    self.emit(format!(
                        "operator requires matching integer types, found {} and {}",
                        fmt_type(&lt),
                        fmt_type(&rt)
                    ));
                    Type::Name("unknown".into(), vec![])
                } else {
                    // Static modulo-by-zero detection
                    if op == BinOp::Mod {
                        if let Expr::Literal(Lit::Int(0)) = r {
                            self.emit("modulo by zero literal".to_string());
                        }
                    }
                    lt
                }
            }
            BinOp::EqCmp | BinOp::NeCmp => {
                if !same_type(&lt, &rt) {
                    self.emit(format!(
                        "equality requires matching types, found {} and {}",
                        fmt_type(&lt),
                        fmt_type(&rt)
                    ));
                }
                Type::Name("bool".into(), vec![])
            }
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                if !same_type(&lt, &rt) || !(is_numeric(&lt) || is_string(&lt)) {
                    self.emit(format!(
                        "comparison requires matching numeric or string types, found {} and {}",
                        fmt_type(&lt),
                        fmt_type(&rt)
                    ));
                }
                Type::Name("bool".into(), vec![])
            }
            BinOp::And | BinOp::Or => unreachable!("logical operators handled above"),
            BinOp::Assign => {
                self.emit("assignment is not a valid expression in v0.2");
                Type::Name("unknown".into(), vec![])
            }
        }
    }

    fn check_call(
        &mut self,
        name: &str,
        args: &[Expr],
        scopes: &mut Vec<HashMap<String, Type>>,
    ) -> Type {
        // Builtins
        match name {
            "println" => {
                for a in args {
                    self.infer_expr(a, scopes);
                }
                return Type::Name("unit".into(), vec![]);
            }
            "assert" => {
                if args.len() != 1 {
                    self.emit("assert expects 1 argument");
                } else {
                    let t = self.infer_expr(&args[0], scopes);
                    if !is_bool(&t) {
                        self.emit(format!("assert expects bool, found {}", fmt_type(&t)));
                    }
                }
                return Type::Name("unit".into(), vec![]);
            }
            "range" => {
                if args.len() != 2 {
                    self.emit("range expects 2 arguments");
                } else {
                    let t1 = self.infer_expr(&args[0], scopes);
                    let t2 = self.infer_expr(&args[1], scopes);
                    if !is_int(&t1) || !is_int(&t2) {
                        self.emit("range expects integer arguments");
                    }
                }
                return Type::Name("List".into(), vec![Type::Name("i32".into(), vec![])]);
            }
            "sqrt" => {
                if args.len() != 1 {
                    self.emit("sqrt expects 1 argument");
                } else {
                    let t = self.infer_expr(&args[0], scopes);
                    if !is_numeric(&t) {
                        self.emit("sqrt expects a numeric argument");
                    }
                }
                return Type::Name("f64".into(), vec![]);
            }
            _ => {}
        }

        let (params, mut ret) = match self.funcs.get(name) {
            Some(sig) => sig.clone(),
            None => {
                // Try module-qualified lookup via use imports
                for module in self.use_imports.clone() {
                    let qualified = format!("{}::{}", module, name);
                    if self.funcs.contains_key(&qualified) {
                        // Recursively check with qualified name
                        return self.check_call(&qualified, args, scopes);
                    }
                }
                self.emit(format!("undefined function '{}'", name));
                return Type::Name("unknown".into(), vec![]);
            }
        };

        if args.len() != params.len() {
            self.emit(format!(
                "function '{}' expects {} arguments, got {}",
                name,
                params.len(),
                args.len()
            ));
        } else {
            // Check if this is a generic function and build type param map
            let generics = self.func_generics.get(name).cloned().unwrap_or_default();
            let mut type_map: HashMap<String, Type> = HashMap::new();

            if !generics.is_empty() {
                // Infer type parameters from argument types
                for (arg, param) in args.iter().zip(params.iter()) {
                    let at = self.infer_expr(arg, scopes);
                    self.infer_type_params(param, &at, &generics, &mut type_map);
                }

                // Check where constraints (before substitution)
                if let Some((type_param, bounds)) = self.where_clauses.get(name).cloned() {
                    for (arg, param) in args.iter().zip(params.iter()) {
                        let at = self.infer_expr(arg, scopes);
                        if self.type_uses_type_param(param, &type_param) {
                            for bound in &bounds {
                                if !self.type_implements_trait(&at, bound) {
                                    self.emit(format!(
                                        "where constraint violated: type '{}' does not implement trait '{}' (required by function '{}')",
                                        fmt_type(&at),
                                        bound,
                                        name
                                    ));
                                }
                            }
                        }
                    }
                }

                // Check arguments with substituted types
                for (i, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
                    let at = self.infer_expr(arg, scopes);
                    let subst_param = subst_type_params(param, &generics, &type_map);
                    if !same_type(&at, &subst_param) {
                        self.emit(format!(
                            "argument {} of '{}' expected {}, found {}",
                            i + 1,
                            name,
                            fmt_type(&subst_param),
                            fmt_type(&at)
                        ));
                    }
                }

                ret = subst_type_params(&ret, &generics, &type_map);
            } else {
                for (i, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
                    let at = self.infer_expr(arg, scopes);
                    if !same_type(&at, param) {
                        self.emit(format!(
                            "argument {} of '{}' expected {}, found {}",
                            i + 1,
                            name,
                            fmt_type(param),
                            fmt_type(&at)
                        ));
                    }
                }
                // Check where constraints for non-generic functions
                if let Some((type_param, bounds)) = self.where_clauses.get(name).cloned() {
                    for (arg, param) in args.iter().zip(params.iter()) {
                        let at = self.infer_expr(arg, scopes);
                        if self.type_uses_type_param(param, &type_param) {
                            for bound in &bounds {
                                if !self.type_implements_trait(&at, bound) {
                                    self.emit(format!(
                                        "where constraint violated: type '{}' does not implement trait '{}' (required by function '{}')",
                                        fmt_type(&at),
                                        bound,
                                        name
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // Check effects
            if let Some(required_effects) = self.func_effects.get(name).cloned() {
                for effect in &required_effects {
                    if !self.has_effect(effect) {
                        self.emit(format!(
                            "effect '{}' required by function '{}' is not available in current scope",
                            effect, name
                        ));
                    }
                }
            }
        }
        ret
    }

    /// Check if an effect is available in the current scope
    fn has_effect(&self, effect: &str) -> bool {
        for scope in self.available_effects.iter().rev() {
            if scope.contains_key(effect) {
                return true;
            }
        }
        false
    }

    /// Check if a type uses a type parameter
    fn type_uses_type_param(&self, ty: &Type, type_param: &str) -> bool {
        match ty {
            Type::Name(name, _) => name == type_param,
            Type::Ref(inner) | Type::RefMut(inner) | Type::Option(inner) | Type::Shared(inner) | Type::LocalShared(inner) | Type::Weak(inner) => {
                self.type_uses_type_param(inner, type_param)
            }
            Type::Result(ok, err) => {
                self.type_uses_type_param(ok, type_param) || self.type_uses_type_param(err, type_param)
            }
            Type::Tuple(elems) => {
                elems.iter().any(|e| self.type_uses_type_param(e, type_param))
            }
            Type::Func(args, ret) => {
                args.iter().any(|a| self.type_uses_type_param(a, type_param)) || self.type_uses_type_param(ret, type_param)
            }
            Type::Newtype(_, inner) => self.type_uses_type_param(inner, type_param),
            _ => false,
        }
    }

    /// Infer type parameter bindings from a parameter type and actual argument type
    fn infer_type_params(
        &self,
        param: &Type,
        actual: &Type,
        generics: &[GenericParam],
        type_map: &mut HashMap<String, Type>,
    ) {
        match param {
            Type::Name(name, _) if is_type_param(name, generics) => {
                type_map.entry(name.clone()).or_insert_with(|| actual.clone());
            }
            Type::Name(name, p_args) => {
                if let Type::Name(_, a_args) = actual {
                    if name == "List" && p_args.len() == 1 && a_args.len() == 1 {
                        self.infer_type_params(&p_args[0], &a_args[0], generics, type_map);
                    }
                }
            }
            Type::Option(inner) => {
                if let Type::Option(a_inner) = actual {
                    self.infer_type_params(inner, a_inner, generics, type_map);
                }
            }
            Type::Result(p_ok, p_err) => {
                if let Type::Result(a_ok, a_err) = actual {
                    self.infer_type_params(p_ok, a_ok, generics, type_map);
                    self.infer_type_params(p_err, a_err, generics, type_map);
                }
            }
            Type::Tuple(p_elems) => {
                if let Type::Tuple(a_elems) = actual {
                    for (pe, ae) in p_elems.iter().zip(a_elems.iter()) {
                        self.infer_type_params(pe, ae, generics, type_map);
                    }
                }
            }
            _ => {}
        }
    }

    fn lookup_var(&mut self, name: &str, scopes: &mut [HashMap<String, Type>]) -> Type {
        for scope in scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return t.clone();
            }
        }
        // Check if it's a module-qualified name via use imports
        for module in &self.use_imports.clone() {
            let qualified = format!("{}::{}", module, name);
            if let Some((params, ret)) = self.funcs.get(&qualified) {
                return Type::Func(params.clone(), Box::new(ret.clone()));
            }
        }
        // Check if it's an actor type name
        if let Some(tdef) = self.types.get(name) {
            if matches!(tdef.kind, TypeDefKind::Record(_)) {
                // This is an actor type - return it as a type
                return Type::Name(name.into(), vec![]);
            }
        }
        self.emit(format!("undefined variable '{}'", name));
        Type::Name("unknown".into(), vec![])
    }

    /// Get all variant names for an enum type
    fn get_enum_variants(&self, ty: &Type) -> Vec<String> {
        match ty {
            Type::Name(name, _) => {
                if let Some(tdef) = self.types.get(name) {
                    match &tdef.kind {
                        TypeDefKind::Enum(variants) => {
                            variants.iter().map(|v| v.name.clone()).collect()
                        }
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// Determine which variants a pattern covers.
    /// Returns (list of covered variant names, whether this is a catch-all pattern)
    fn pattern_covers_variants(&self, pat: &Pattern, subject_ty: &Type) -> (Vec<String>, bool) {
        match pat {
            Pattern::Wildcard => {
                // Wildcard covers all variants
                let all = self.get_enum_variants(subject_ty);
                (all, true)
            }
            Pattern::Variable(_) => {
                // Variable pattern covers all variants
                let all = self.get_enum_variants(subject_ty);
                (all, true)
            }
            Pattern::Literal(_) => {
                // Literal patterns don't cover enum variants
                (Vec::new(), false)
            }
            Pattern::Constructor(name, _) => {
                // Constructor pattern covers only that specific variant
                (vec![name.clone()], false)
            }
            Pattern::Tuple(pats) => {
                // Tuple pattern - for enum matching, this doesn't directly cover variants
                // but we need to handle nested tuple patterns that might contain constructors
                let mut covered = Vec::new();
                // For tuple patterns matching against enum types, we need the tuple element types
                if let Type::Tuple(elem_types) = subject_ty {
                    for (i, p) in pats.iter().enumerate() {
                        if i < elem_types.len() {
                            let (vars, _) = self.pattern_covers_variants(p, &elem_types[i]);
                            for v in vars {
                                if !covered.contains(&v) {
                                    covered.push(v);
                                }
                            }
                        }
                    }
                }
                (covered, false)
            }
        }
    }
}

/// Check if a type name is a generic type parameter
fn is_type_param(name: &str, generics: &[GenericParam]) -> bool {
    generics.iter().any(|g| g.name == name)
}

/// Substitute type parameters in a type
fn subst_type_params(ty: &Type, generics: &[GenericParam], type_map: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Name(name, args) => {
            if is_type_param(name, generics) {
                if let Some(concrete) = type_map.get(name) {
                    concrete.clone()
                } else {
                    ty.clone()
                }
            } else {
                let new_args: Vec<Type> = args.iter()
                    .map(|a| subst_type_params(a, generics, type_map))
                    .collect();
                Type::Name(name.clone(), new_args)
            }
        }
        Type::Ref(inner) => Type::Ref(Box::new(subst_type_params(inner, generics, type_map))),
        Type::RefMut(inner) => Type::RefMut(Box::new(subst_type_params(inner, generics, type_map))),
        Type::Option(inner) => Type::Option(Box::new(subst_type_params(inner, generics, type_map))),
        Type::Result(ok, err) => Type::Result(
            Box::new(subst_type_params(ok, generics, type_map)),
            Box::new(subst_type_params(err, generics, type_map)),
        ),
        Type::Tuple(elems) => Type::Tuple(
            elems.iter().map(|e| subst_type_params(e, generics, type_map)).collect(),
        ),
        Type::Func(args, ret) => Type::Func(
            args.iter().map(|a| subst_type_params(a, generics, type_map)).collect(),
            Box::new(subst_type_params(ret, generics, type_map)),
        ),
        Type::Shared(inner) => Type::Shared(Box::new(subst_type_params(inner, generics, type_map))),
        Type::LocalShared(inner) => Type::LocalShared(Box::new(subst_type_params(inner, generics, type_map))),
        Type::Weak(inner) => Type::Weak(Box::new(subst_type_params(inner, generics, type_map))),
        Type::Newtype(name, inner) => Type::Newtype(name.clone(), Box::new(subst_type_params(inner, generics, type_map))),
        Type::Cap(_) | Type::Nothing => ty.clone(),
    }
}

fn same_type(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Name(na, aa), Type::Name(nb, ab)) => na == nb && aa.len() == ab.len() && aa.iter().zip(ab.iter()).all(|(x, y)| same_type(x, y)),
        (Type::Ref(a), Type::Ref(b)) => same_type(a, b),
        (Type::RefMut(a), Type::RefMut(b)) => same_type(a, b),
        (Type::Option(a), Type::Option(b)) => same_type(a, b),
        (Type::Result(a1, a2), Type::Result(b1, b2)) => same_type(a1, b1) && same_type(a2, b2),
        (Type::Tuple(a), Type::Tuple(b)) => a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| same_type(x, y)),
        (Type::Func(a_args, a_ret), Type::Func(b_args, b_ret)) => {
            a_args.len() == b_args.len()
                && a_args.iter().zip(b_args.iter()).all(|(x, y)| same_type(x, y))
                && same_type(a_ret, b_ret)
        }
        (Type::Cap(a), Type::Cap(b)) => a == b,
        (Type::Shared(a), Type::Shared(b)) => same_type(a, b),
        (Type::LocalShared(a), Type::LocalShared(b)) => same_type(a, b),
        (Type::Weak(a), Type::Weak(b)) => same_type(a, b),
        // Newtypes with same name and same inner type are equal
        (Type::Newtype(n1, a), Type::Newtype(n2, b)) => n1 == n2 && same_type(a, b),
        // A named type matches a newtype with the same inner type name
        (Type::Name(n, _), Type::Newtype(n2, _)) | (Type::Newtype(n2, _), Type::Name(n, _)) => {
            n == n2
        }
        _ => false,
    }
}

fn is_int(t: &Type) -> bool {
    matches!(t, Type::Name(n, _) if n == "i32" || n == "i64")
}

fn is_numeric(t: &Type) -> bool {
    matches!(t, Type::Name(n, _) if n == "i32" || n == "i64" || n == "f64")
}

fn is_bool(t: &Type) -> bool {
    matches!(t, Type::Name(n, _) if n == "bool")
}

fn is_string(t: &Type) -> bool {
    matches!(t, Type::Name(n, _) if n == "string")
}

fn fmt_type(t: &Type) -> String {
    match t {
        Type::Name(n, args) if args.is_empty() => n.clone(),
        Type::Name(n, args) => format!("{}<{}>", n, args.iter().map(fmt_type).collect::<Vec<_>>().join(", ")),
        Type::Ref(inner) => format!("&{}", fmt_type(inner)),
        Type::RefMut(inner) => format!("&mut {}", fmt_type(inner)),
        Type::Option(inner) => format!("{}?", fmt_type(inner)),
        Type::Result(ok, err) => format!("Result<{}, {}>", fmt_type(ok), fmt_type(err)),
        Type::Tuple(elems) => format!("({})", elems.iter().map(fmt_type).collect::<Vec<_>>().join(", ")),
        Type::Func(args, ret) => format!(
            "fn({}) -> {}",
            args.iter().map(fmt_type).collect::<Vec<_>>().join(", "),
            fmt_type(ret)
        ),
        Type::Cap(name) => format!("cap {}", name),
        Type::Shared(inner) => format!("shared {}", fmt_type(inner)),
        Type::LocalShared(inner) => format!("local_shared {}", fmt_type(inner)),
        Type::Weak(inner) => format!("weak {}", fmt_type(inner)),
        Type::Newtype(name, inner) => format!("newtype {} {}", name, fmt_type(inner)),
        Type::Nothing => "nothing".to_string(),
    }
}
