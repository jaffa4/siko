use crate::error::Error;
use crate::ir::expr::Expr;
use crate::ir::expr::ExprId;
use crate::ir::function::FunctionId;
use crate::ir::function::FunctionInfo;
use crate::ir::program::Program;
use crate::ir::types::TypeSignature;
use crate::ir::types::TypeSignatureId;
use crate::typechecker::error::TypecheckError;
use crate::typechecker::function_type::FunctionType;
use crate::typechecker::type_store::TypeStore;
use crate::typechecker::type_variable::TypeVariable;
use crate::typechecker::types::Type;
use crate::util::format_list_simple;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

struct FunctionDependencyInfo {
    function_deps: BTreeSet<FunctionId>,
}

impl FunctionDependencyInfo {
    fn new() -> FunctionDependencyInfo {
        FunctionDependencyInfo {
            function_deps: BTreeSet::new(),
        }
    }
}

struct FunctionInfoCollector<'a> {
    function_type_info: &'a mut FunctionDependencyInfo,
}

impl<'a> FunctionInfoCollector<'a> {
    fn new(function_type_info: &'a mut FunctionDependencyInfo) -> FunctionInfoCollector<'a> {
        FunctionInfoCollector {
            function_type_info: function_type_info,
        }
    }
}

impl<'a> Collector for FunctionInfoCollector<'a> {
    fn process(&mut self, program: &Program, expr: &Expr, id: ExprId) {
        match expr {
            Expr::StaticFunctionCall(func_id, _) => {
                self.function_type_info.function_deps.insert(*func_id);
            }
            Expr::LambdaFunction(func_id, _) => {
                self.function_type_info.function_deps.insert(*func_id);
            }
            _ => {}
        }
    }
}

struct TypeProcessor<'a> {
    type_store: &'a mut TypeStore,
    function_type_map: &'a BTreeMap<FunctionId, TypeVariable>,
    type_vars: BTreeMap<ExprId, TypeVariable>,
    function_args: BTreeMap<FunctionId, Vec<TypeVariable>>,
    captured_function_args: BTreeMap<FunctionId, Vec<TypeVariable>>,
}

impl<'a> TypeProcessor<'a> {
    fn new(
        type_store: &'a mut TypeStore,
        function_type_map: &'a BTreeMap<FunctionId, TypeVariable>,
        function_id: FunctionId,
        args: Vec<TypeVariable>,
    ) -> TypeProcessor<'a> {
        let mut function_args = BTreeMap::new();
        function_args.insert(function_id, args);
        TypeProcessor {
            type_store: type_store,
            function_type_map: function_type_map,
            type_vars: BTreeMap::new(),
            function_args: function_args,
            captured_function_args: BTreeMap::new(),
        }
    }

    fn get_type_var_for_expr(&self, id: &ExprId) -> TypeVariable {
        self.type_vars
            .get(id)
            .expect("Sub expr type var not found")
            .clone()
    }

    fn process_function_call(
        &mut self,
        ty: &Type,
        function_type: &FunctionType,
        args: &[ExprId],
        id: ExprId,
        program: &Program,
        errors: &mut Vec<TypecheckError>,
        name: String,
    ) {
        let cloned_function_type = self.type_store.clone_type(&ty);
        let types = if let Type::Function(ft) = cloned_function_type {
            ft.types
        } else {
            unreachable!();
        };
        if args.len() > types.len() - 1 {
            let ast_id = program.get_ast_expr_id(&id);
            let err = TypecheckError::TooManyArguments(*ast_id, name, types.len() - 1, args.len());
            errors.push(err);
        } else {
            let mut mismatch = false;
            for (index, arg) in args.iter().enumerate() {
                let arg_var = self.get_type_var_for_expr(arg);
                let type_var = types[index].get_inner_type_var();
                if !self.type_store.unify_vars(arg_var, type_var) {
                    mismatch = true;
                    break;
                }
            }
            if mismatch {
                let ast_id = program.get_ast_expr_id(&id);
                let mut arg_types = Vec::new();
                for arg in args {
                    let arg_var = self.get_type_var_for_expr(arg);
                    let ty = self.type_store.get_resolved_type(&arg_var);
                    arg_types.push(format!("{}", ty));
                }
                let arg_types = format_list_simple(&arg_types[..]);
                let func_type = function_type.as_string(self.type_store);
                let err = TypecheckError::FunctionArgumentMismatch(*ast_id, arg_types, func_type);
                errors.push(err);
            } else {
                let call_var = self.get_type_var_for_expr(&id);
                let rest: Vec<Type> = types[args.len()..].to_vec();
                let result_var = if rest.len() == 1 {
                    rest[0].get_inner_type_var()
                } else {
                    let closure_type = FunctionType::new(rest);
                    let ty = Type::Function(closure_type);
                    self.type_store.add_var(ty)
                };
                if !self.type_store.unify_vars(call_var, result_var) {
                    let ast_id = program.get_ast_expr_id(&id);
                    let call_type = self.type_store.get_resolved_type(&call_var);
                    let call_type = format!("{}", call_type);
                    let result_type = self.type_store.get_resolved_type(&result_var);
                    let result_type = format!("{}", result_type);
                    let err = TypecheckError::TypeMismatch(*ast_id, call_type, result_type);
                    errors.push(err);
                }
            }
        }
    }

    fn check_constraints(&mut self, program: &Program, errors: &mut Vec<TypecheckError>) {
        for (id, ty_var) in self.type_vars.clone() {
            let expr = program.get_expr(&id);
            match expr {
                Expr::IntegerLiteral(_) => {}
                Expr::FloatLiteral(_) => {}
                Expr::BoolLiteral(_) => {}
                Expr::StringLiteral(_) => {}
                Expr::If(cond, true_branch, false_branch) => {
                    let cond_var = self.get_type_var_for_expr(cond);
                    let true_var = self.get_type_var_for_expr(true_branch);
                    let false_var = self.get_type_var_for_expr(false_branch);
                    let cond_ty = self.type_store.get_type(&cond_var);
                    if cond_ty != Type::Bool {
                        let var = self.type_store.add_var(Type::Bool);
                        if !self.type_store.unify_vars(var, cond_var) {
                            let ast_id = program.get_ast_expr_id(cond);
                            let cond_ty = self.type_store.get_resolved_type(&cond_var);
                            let bool_ty = format!("{}", Type::Bool);
                            let cond_ty = format!("{}", cond_ty);
                            let err = TypecheckError::TypeMismatch(*ast_id, bool_ty, cond_ty);
                            errors.push(err);
                        }
                    }
                    if !self.type_store.unify_vars(true_var, false_var) {
                        let ast_id = program.get_ast_expr_id(&false_branch);
                        let true_type = self.type_store.get_resolved_type(&true_var);
                        let false_type = self.type_store.get_resolved_type(&false_var);
                        let true_type = format!("{}", true_type);
                        let false_type = format!("{}", false_type);
                        let err = TypecheckError::TypeMismatch(*ast_id, true_type, false_type);
                        errors.push(err);
                    }
                }
                Expr::StaticFunctionCall(function_id, args) => {
                    let target_func_type_var = self
                        .function_type_map
                        .get(function_id)
                        .expect("Function type not found");
                    let ty = self.type_store.get_type(target_func_type_var);
                    match &ty {
                        Type::Function(function_type) => {
                            let f = program.get_function(function_id);
                            let name = format!("{}", f.info);
                            self.process_function_call(
                                &ty,
                                function_type,
                                args,
                                id,
                                program,
                                errors,
                                name,
                            );
                        }
                        _ => {
                            if !args.is_empty() {
                                let f = program.get_function(function_id);
                                let name = format!("{}", f.info);
                                let ast_id = program.get_ast_expr_id(&id);
                                let err =
                                    TypecheckError::TooManyArguments(*ast_id, name, 0, args.len());
                                errors.push(err);
                            } else {
                                let call_var = self.get_type_var_for_expr(&id);
                                if !self.type_store.unify_vars(call_var, *target_func_type_var) {
                                    let ast_id = program.get_ast_expr_id(&id);
                                    let call_type = self.type_store.get_resolved_type(&call_var);
                                    let func_type =
                                        self.type_store.get_resolved_type(target_func_type_var);
                                    let call_type = format!("{}", call_type);
                                    let func_type = format!("{}", func_type);
                                    let err =
                                        TypecheckError::TypeMismatch(*ast_id, call_type, func_type);
                                    errors.push(err);
                                }
                            }
                        }
                    }
                }
                Expr::Tuple(_) => {}
                Expr::Do(_) => {}
                Expr::Bind(_, _) => {}
                Expr::ExprValue(_) => {}
                Expr::DynamicFunctionCall(func_expr_id, args) => {
                    let type_var = self.get_type_var_for_expr(func_expr_id);
                    let ty = self.type_store.get_type(&type_var);
                    let resolved_type = self.type_store.get_resolved_type(&type_var);
                    let name = format!("closure({})", resolved_type);
                    match &ty {
                        Type::Function(function_type) => {
                            self.process_function_call(
                                &ty,
                                &function_type,
                                args,
                                id,
                                program,
                                errors,
                                name,
                            );
                        }
                        _ => {
                            let ast_id = program.get_ast_expr_id(&id);
                            let err = TypecheckError::NotCallableType(
                                *ast_id,
                                format!("{}", resolved_type),
                            );
                            errors.push(err);
                        }
                    }
                }
                Expr::ArgRef(_) => {}
                Expr::LambdaFunction(lambda_id, _) => {
                    let type_var = self.get_type_var_for_expr(&id);
                }
                Expr::LambdaCapturedArgRef(arg_ref) => {}
            }
        }
    }

    fn dump_types(&self, program: &Program) {
        for (id, var) in &self.type_vars {
            let expr = program.get_expr(id);
            let ty = self.type_store.get_resolved_type(var);
            println!("{} {} => {}", id, expr, ty);
        }
    }
}

impl<'a> Collector for TypeProcessor<'a> {
    fn process(&mut self, program: &Program, expr: &Expr, id: ExprId) {
        match expr {
            Expr::IntegerLiteral(_) => {
                let ty = Type::Int;
                let var = self.type_store.add_var(ty);
                self.type_vars.insert(id, var);
            }
            Expr::FloatLiteral(_) => {
                let ty = Type::Float;
                let var = self.type_store.add_var(ty);
                self.type_vars.insert(id, var);
            }
            Expr::BoolLiteral(_) => {
                let ty = Type::Bool;
                let var = self.type_store.add_var(ty);
                self.type_vars.insert(id, var);
            }
            Expr::StringLiteral(_) => {
                let ty = Type::String;
                let var = self.type_store.add_var(ty);
                self.type_vars.insert(id, var);
            }
            Expr::If(_, true_branch, _) => {
                let true_var = self.get_type_var_for_expr(true_branch);
                self.type_vars.insert(id, true_var);
            }
            Expr::StaticFunctionCall(_, _) => {
                let ty = Type::TypeArgument(self.type_store.get_unique_type_arg());
                let result_var = self.type_store.add_var(ty);
                self.type_vars.insert(id, result_var);
            }
            Expr::Tuple(items) => {
                let items: Vec<_> = items
                    .iter()
                    .map(|i| Type::TypeVar(self.get_type_var_for_expr(i)))
                    .collect();
                let ty = Type::Tuple(items);
                let var = self.type_store.add_var(ty);
                self.type_vars.insert(id, var);
            }
            Expr::Do(items) => {
                let last = items.last().expect("Empty do");
                let var = self.get_type_var_for_expr(last);
                self.type_vars.insert(id, var);
            }
            Expr::Bind(_, _) => {
                let ty = Type::Tuple(vec![]);
                let var = self.type_store.add_var(ty);
                self.type_vars.insert(id, var);
            }
            Expr::ExprValue(expr_id) => {
                let var = self.get_type_var_for_expr(expr_id);
                self.type_vars.insert(id, var);
            }
            Expr::DynamicFunctionCall(_, _) => {
                let ty = Type::TypeArgument(self.type_store.get_unique_type_arg());
                let result_var = self.type_store.add_var(ty);
                self.type_vars.insert(id, result_var);
            }
            Expr::ArgRef(index) => {
                self.type_vars.insert(
                    id,
                    self.function_args.get(&index.id).expect("Missing arg set")[index.index],
                );
            }
            Expr::LambdaFunction(lambda_id, captures) => {
                let captured_vars: Vec<_> = captures
                    .iter()
                    .map(|c| self.get_type_var_for_expr(c))
                    .collect();
                self.captured_function_args
                    .insert(*lambda_id, captured_vars);
                let lambda_info = program.get_function(lambda_id);
                let mut args = Vec::new();
                let mut types = Vec::new();
                for _ in 0..lambda_info.arg_count {
                    let ty = Type::TypeArgument(self.type_store.get_unique_type_arg());
                    types.push(ty.clone());
                    let var = self.type_store.add_var(ty);
                    args.push(var);
                }
                let lambda_result_type = Type::TypeArgument(self.type_store.get_unique_type_arg());
                types.push(lambda_result_type);
                self.function_args.insert(*lambda_id, args);
                let lambda_function_type = FunctionType::new(types);
                let ty = Type::Function(lambda_function_type);
                let result_var = self.type_store.add_var(ty);
                self.type_vars.insert(id, result_var);
            }
            Expr::LambdaCapturedArgRef(arg_ref) => {
                let var = self
                    .captured_function_args
                    .get(&arg_ref.id)
                    .expect("Missing lambda arg set")[arg_ref.index];
                self.type_vars.insert(id, var);
            }
        }
    }
}

trait Collector {
    fn process(&mut self, program: &Program, expr: &Expr, id: ExprId);
}

fn walker(program: &Program, id: &ExprId, collector: &mut Collector) {
    let expr = program.get_expr(id);
    println!("TC: {}: Processing expr {}", id, expr);
    match expr {
        Expr::StaticFunctionCall(_, args) => {
            for arg in args {
                walker(program, arg, collector);
            }
        }
        Expr::LambdaFunction(lambda_id, captures) => {
            let lambda = program.get_function(lambda_id);
            for captured in captures {
                walker(program, captured, collector);
            }
            collector.process(program, expr, *id);
            if let FunctionInfo::Lambda(info) = &lambda.info {
                walker(program, &info.body, collector);
            } else {
                unreachable!()
            }
            return;
        }
        Expr::DynamicFunctionCall(id, args) => {
            walker(program, id, collector);
            for arg in args {
                walker(program, arg, collector);
            }
        }
        Expr::If(cond, true_branch, false_branch) => {
            walker(program, cond, collector);
            walker(program, true_branch, collector);
            walker(program, false_branch, collector);
        }
        Expr::Tuple(items) => {
            for item in items {
                walker(program, item, collector)
            }
        }
        Expr::IntegerLiteral(_) => {}
        Expr::FloatLiteral(_) => {}
        Expr::BoolLiteral(_) => {}
        Expr::StringLiteral(_) => {}
        Expr::Do(items) => {
            for item in items {
                walker(program, item, collector)
            }
        }
        Expr::Bind(_, expr) => walker(program, expr, collector),
        Expr::ArgRef(_) => {}
        Expr::ExprValue(_) => {}
        Expr::LambdaCapturedArgRef(_) => {}
    }
    collector.process(program, &expr, *id);
}

pub struct Typechecker {
    function_info_map: BTreeMap<FunctionId, FunctionDependencyInfo>,
    function_type_map: BTreeMap<FunctionId, TypeVariable>,
    type_store: TypeStore,
}

impl Typechecker {
    pub fn new() -> Typechecker {
        Typechecker {
            function_info_map: BTreeMap::new(),
            function_type_map: BTreeMap::new(),
            type_store: TypeStore::new(),
        }
    }

    fn check_untyped_function(
        &mut self,
        id: FunctionId,
        program: &Program,
        errors: &mut Vec<TypecheckError>,
    ) {
        let function = program.get_function(&id);
        println!("Checking untyped {},{}", id, function.info);
        let mut args = Vec::new();
        for _ in 0..function.arg_count {
            let ty_arg = self.type_store.get_unique_type_arg();
            let ty = Type::TypeArgument(ty_arg);
            let var = self.type_store.add_var(ty);
            args.push(var);
        }
        let body = function.info.body();
        let mut type_processor =
            TypeProcessor::new(&mut self.type_store, &self.function_type_map, id, args);
        walker(program, &body, &mut type_processor);
        type_processor.check_constraints(program, errors);
        type_processor.dump_types(program);
    }

    fn check_function_deps(
        &self,
        mut untyped_functions: BTreeSet<FunctionId>,
        errors: &mut Vec<TypecheckError>,
        program: &Program,
    ) -> Vec<FunctionId> {
        let mut untyped_check_order = Vec::new();
        while !untyped_functions.is_empty() {
            let mut processed = Vec::new();
            for id in &untyped_functions {
                let info = self
                    .function_info_map
                    .get(id)
                    .expect("Function info not found");
                let mut dep_is_untyped = false;
                for dep in &info.function_deps {
                    if untyped_functions.contains(dep) {
                        dep_is_untyped = true;
                        break;
                    }
                }
                if dep_is_untyped {
                    continue;
                } else {
                    untyped_check_order.push(*id);
                    processed.push(*id);
                }
            }
            if processed.is_empty() {
                for id in &untyped_functions {
                    let f = program.get_function(id);
                    println!("Untyped function: {}", f.info);
                }
                let err = TypecheckError::FunctionTypeDependencyLoop;
                errors.push(err);
                break;
            } else {
                for id in processed {
                    untyped_functions.remove(&id);
                }
            }
        }
        untyped_check_order
    }

    fn process_type_signature(
        &mut self,
        type_signature_id: &TypeSignatureId,
        program: &Program,
        arg_map: &mut BTreeMap<usize, TypeVariable>,
    ) -> TypeVariable {
        let type_signature = program.get_type_signature(type_signature_id);
        match type_signature {
            TypeSignature::Bool => {
                let ty = Type::Bool;
                return self.type_store.add_var(ty);
            }
            TypeSignature::Int => {
                let ty = Type::Int;
                return self.type_store.add_var(ty);
            }
            TypeSignature::String => {
                let ty = Type::String;
                return self.type_store.add_var(ty);
            }
            TypeSignature::Nothing => {
                let ty = Type::Nothing;
                return self.type_store.add_var(ty);
            }
            TypeSignature::Tuple(items) => {
                let items: Vec<_> = items
                    .iter()
                    .map(|i| Type::TypeVar(self.process_type_signature(i, program, arg_map)))
                    .collect();
                let ty = Type::Tuple(items);
                return self.type_store.add_var(ty);
            }
            TypeSignature::Function(items) => {
                let items: Vec<_> = items
                    .iter()
                    .map(|i| Type::TypeVar(self.process_type_signature(i, program, arg_map)))
                    .collect();
                let ty = Type::Function(FunctionType::new(items));
                return self.type_store.add_var(ty);
            }
            TypeSignature::TypeArgument(index) => {
                let var = arg_map.entry(*index).or_insert_with(|| {
                    let arg = self.type_store.get_unique_type_arg();
                    let ty = Type::TypeArgument(arg);
                    self.type_store.add_var(ty)
                });
                *var
            }
        }
    }

    fn add_type_signature(
        &mut self,
        type_signature_id: TypeSignatureId,
        function_id: FunctionId,
        program: &Program,
    ) {
        let mut arg_map = BTreeMap::new();
        let var = self.process_type_signature(&type_signature_id, program, &mut arg_map);
        /*println!(
            "Registering function {} with type {}",
            function_id,
            self.type_store.get_resolved_type(&var)
        );*/
        self.function_type_map.insert(function_id, var);
    }

    pub fn check(&mut self, program: &Program) -> Result<(), Error> {
        let mut errors = Vec::new();
        let mut untyped_functions = BTreeSet::new();
        let mut typed_functions = BTreeSet::new();
        println!("All function count {}", program.functions.len());
        for (id, function) in &program.functions {
            let mut function_info = FunctionDependencyInfo::new();
            let mut function_info_collector = FunctionInfoCollector::new(&mut function_info);
            match &function.info {
                FunctionInfo::Lambda(i) => {
                    println!("Skipping lambda {},{}", id, i);
                    //walker(program, &e, &mut function_info_collector);
                    //untyped_functions.insert(*id);
                }
                FunctionInfo::NamedFunction(i) => {
                    let untyped = match i.type_signature {
                        Some(type_signature) => {
                            self.add_type_signature(type_signature, *id, program);
                            false
                        }
                        None => true,
                    };
                    if untyped {
                        untyped_functions.insert(*id);
                    }
                    if let Some(body) = i.body {
                        walker(program, &body, &mut function_info_collector);
                        if !untyped {
                            typed_functions.insert(*id);
                        }
                    } else {
                        if untyped {
                            let err = TypecheckError::UntypedExternFunction(
                                i.name.clone(),
                                i.ast_function_id,
                            );
                            errors.push(err)
                        }
                    }
                }
            }
            self.function_info_map.insert(*id, function_info);
        }

        let untyped_check_order = self.check_function_deps(untyped_functions, &mut errors, program);

        if !errors.is_empty() {
            return Err(Error::typecheck_err(errors));
        }

        println!(
            "Typed {}, untyped {}",
            typed_functions.len(),
            untyped_check_order.len()
        );

        for function_id in typed_functions {
            println!("Checking typed {}", function_id);
        }

        for function_id in untyped_check_order {
            self.check_untyped_function(function_id, program, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::typecheck_err(errors))
        }
    }
}
