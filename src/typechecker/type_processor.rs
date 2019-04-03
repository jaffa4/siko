use crate::ir::expr::Expr;
use crate::ir::expr::ExprId;
use crate::ir::function::FunctionId;
use crate::ir::program::Program;
use crate::typechecker::collector::Collector;
use crate::typechecker::error::TypecheckError;
use crate::typechecker::function_type::FunctionType;
use crate::typechecker::type_store::TypeStore;
use crate::typechecker::type_variable::TypeVariable;
use crate::typechecker::types::Type;
use crate::util::format_list_simple;
use std::collections::BTreeMap;

pub struct TypeProcessor<'a> {
    type_store: &'a mut TypeStore,
    type_of_exprs: BTreeMap<ExprId, TypeVariable>,
    function_args: BTreeMap<FunctionId, Vec<TypeVariable>>,
    captured_function_args: BTreeMap<FunctionId, Vec<TypeVariable>>,
    function_id: FunctionId,
    function_type_map: &'a BTreeMap<FunctionId, TypeVariable>,
    function_call_copied_types: BTreeMap<ExprId, TypeVariable>,
}

impl<'a> TypeProcessor<'a> {
    pub fn new(
        type_store: &'a mut TypeStore,
        function_type_map: &'a BTreeMap<FunctionId, TypeVariable>,
        function_id: FunctionId,
        args: Vec<TypeVariable>,
    ) -> TypeProcessor<'a> {
        let mut function_args = BTreeMap::new();
        function_args.insert(function_id, args);
        TypeProcessor {
            type_store: type_store,
            type_of_exprs: BTreeMap::new(),
            function_args: function_args,
            captured_function_args: BTreeMap::new(),
            function_id: function_id,
            function_type_map: function_type_map,
            function_call_copied_types: BTreeMap::new(),
        }
    }

    fn get_cloned_function_type(
        &mut self,
        function_id: FunctionId,
        expr_id: ExprId,
    ) -> TypeVariable {
        match self.function_call_copied_types.get(&expr_id) {
            Some(c) => c.clone(),
            None => {
                let orig_type_var = self
                    .function_type_map
                    .get(&function_id)
                    .expect("Function not found");
                let orig_ty = self.type_store.get_type(&orig_type_var);
                let cloned_ty = self.type_store.clone_type(&orig_ty);
                let cloned_var = self.type_store.add_var(cloned_ty);
                self.function_call_copied_types
                    .insert(expr_id, cloned_var.clone());
                cloned_var
            }
        }
    }

    fn get_type_var_for_expr(&self, id: &ExprId) -> TypeVariable {
        self.type_of_exprs
            .get(id)
            .expect("Sub expr type var not found")
            .clone()
    }

    fn function_type_part(&mut self, type_vars: &[TypeVariable]) -> TypeVariable {
        if type_vars.len() < 2 {
            return type_vars[0];
        } else {
            let from = type_vars[0];
            let to = self.function_type_part(&type_vars[1..]);
            let function_type = Type::Function(FunctionType::new(from, to));
            return self.type_store.add_var(function_type);
        }
    }

    pub fn get_function_type(&mut self, body: &ExprId) -> TypeVariable {
        let args = self
            .function_args
            .get(&self.function_id)
            .expect("Function args not found");
        let body_var = self
            .type_of_exprs
            .get(body)
            .expect("Body expr var not found");
        if args.is_empty() {
            *body_var
        } else {
            let mut type_vars = args.clone();
            type_vars.push(*body_var);
            self.function_type_part(&type_vars[..])
        }
    }

    fn unify_variables(
        &mut self,
        expected: &TypeVariable,
        found: &TypeVariable,
        program: &Program,
        id: ExprId,
        unified_variables: &mut bool,
        errors: &mut Vec<TypecheckError>,
    ) {
        if !self.type_store.unify(&expected, &found, unified_variables) {
            let location_id = program.get_expr_location(&id);
            let expected_type = self.type_store.get_resolved_type_string(&expected);
            let found_type = self.type_store.get_resolved_type_string(&found);
            let err = TypecheckError::TypeMismatch(location_id, expected_type, found_type);
            errors.push(err);
        }
    }

    fn apply_function(
        &mut self,
        function_var: &TypeVariable,
        args: &[ExprId],
        id: ExprId,
        program: &Program,
        errors: &mut Vec<TypecheckError>,
        name: String,
        unified_variables: &mut bool,
    ) {
        if args.is_empty() {
            let result_var = self.get_type_var_for_expr(&id);
            self.unify_variables(
                &function_var,
                &result_var,
                program,
                id,
                unified_variables,
                errors,
            );
        } else {
            let function_type = self.type_store.get_type(function_var);
            if let Type::Function(function_type) = function_type {
                let first_arg = &args[0];
                let first_arg_var = self.get_type_var_for_expr(first_arg);
                self.unify_variables(
                    &function_type.from,
                    &first_arg_var,
                    program,
                    id,
                    unified_variables,
                    errors,
                );
                if let Type::Function(_) = self.type_store.get_type(&function_type.to) {
                    if args[1..].len() > 0 {
                        self.apply_function(
                            &function_type.to,
                            &args[1..],
                            id,
                            program,
                            errors,
                            name,
                            unified_variables,
                        );
                    }
                } else {
                    if args[1..].len() > 0 {
                        self.ensure_callable(
                            &args[0],
                            &id,
                            &function_type.to,
                            program,
                            unified_variables,
                            errors,
                        );
                    } else {
                        let result_var = self.get_type_var_for_expr(&id);
                        self.unify_variables(
                            &function_type.to,
                            &result_var,
                            program,
                            id,
                            unified_variables,
                            errors,
                        );
                    }
                }
            } else {
                self.ensure_callable(
                    &args[0],
                    &id,
                    function_var,
                    program,
                    unified_variables,
                    errors,
                );
            }
        }
    }

    pub fn check_constraints(&mut self, program: &Program, errors: &mut Vec<TypecheckError>) {
        let mut unified_variables = true;
        while unified_variables && errors.is_empty() {
            unified_variables = false;
            self.check_constraints_inner(program, errors, &mut unified_variables, false);
        }
        if errors.is_empty() {
            self.check_constraints_inner(program, errors, &mut unified_variables, true);
        }
    }

    fn ensure_callable(
        &mut self,
        from: &ExprId,
        to: &ExprId,
        current_func_var: &TypeVariable,
        program: &Program,
        unified_variables: &mut bool,
        errors: &mut Vec<TypecheckError>,
    ) {
        let from_var = self.get_type_var_for_expr(from);
        let to_var = self.get_type_var_for_expr(&to);
        let new_function = Type::Function(FunctionType::new(from_var, to_var));
        let new_function_var = self.type_store.add_var(new_function);
        if !self
            .type_store
            .unify(&new_function_var, &current_func_var, unified_variables)
        {
            let location_id = program.get_expr_location(&to);
            let call_type = self.type_store.get_resolved_type_string(&new_function_var);
            let func_type = self.type_store.get_resolved_type_string(&current_func_var);
            let err = TypecheckError::TypeMismatch(location_id, call_type, func_type);
            errors.push(err);
        }
    }

    fn check_constraints_inner(
        &mut self,
        program: &Program,
        errors: &mut Vec<TypecheckError>,
        unified_variables: &mut bool,
        final_round: bool,
    ) {
        for (id, _) in self.type_of_exprs.clone() {
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
                        if !self.type_store.unify(&var, &cond_var, unified_variables) {
                            let location_id = program.get_expr_location(cond);
                            let cond_ty = self.type_store.get_resolved_type_string(&cond_var);
                            let bool_ty = format!("{}", Type::Bool);
                            let err = TypecheckError::TypeMismatch(location_id, bool_ty, cond_ty);
                            errors.push(err);
                        }
                    }
                    if !self
                        .type_store
                        .unify(&true_var, &false_var, unified_variables)
                    {
                        let location_id = program.get_expr_location(&false_branch);
                        let true_type = self.type_store.get_resolved_type_string(&true_var);
                        let false_type = self.type_store.get_resolved_type_string(&false_var);
                        let err = TypecheckError::TypeMismatch(location_id, true_type, false_type);
                        errors.push(err);
                    }
                }
                Expr::StaticFunctionCall(function_id, args) => {
                    let target_func_type_var = self.get_cloned_function_type(*function_id, id);
                    let f = program.get_function(function_id);
                    let name = format!("{}", f.info);
                    self.apply_function(
                        &target_func_type_var,
                        &args[..],
                        id,
                        program,
                        errors,
                        name,
                        unified_variables,
                    );
                }
                Expr::Tuple(_) => {}
                Expr::Do(_) => {}
                Expr::Bind(_, _) => {}
                Expr::ExprValue(_) => {}
                Expr::DynamicFunctionCall(func_expr_id, args) => {
                    let type_var = self.get_type_var_for_expr(func_expr_id);
                    let resolved_type = self.type_store.get_resolved_type_string(&type_var);
                    let name = format!("closure({})", resolved_type);
                    self.apply_function(
                        &type_var,
                        &args[..],
                        id,
                        program,
                        errors,
                        name,
                        unified_variables,
                    );
                }
                Expr::ArgRef(_) => {}
                Expr::LambdaFunction(lambda_id, _) => {
                    let type_var = self.get_type_var_for_expr(&id);
                    let ty = self.type_store.get_type(&type_var);
                    if let Type::Function(function_type) = ty {
                        let return_type_var = function_type.get_return_type(&self.type_store);
                        let lambda_info = program.get_function(lambda_id);
                        let body_id = lambda_info.info.body();
                        let body_var = self.get_type_var_for_expr(&body_id);
                        self.type_store
                            .unify(&body_var, &return_type_var, unified_variables);
                    } else {
                        panic!("Type of lambda is not a function {}", ty);
                    }
                }
                Expr::LambdaCapturedArgRef(_) => {}
                Expr::FieldAccess(..) => {}
                Expr::TupleFieldAccess(..) => {}
            }
        }
    }

    pub fn dump_types(&self, program: &Program) {
        for (id, var) in &self.type_of_exprs {
            let expr = program.get_expr(id);
            let ty = self.type_store.get_resolved_type_string(var);
            println!("{},{:?} {} => {}", id, var, expr, ty);
        }
    }
}

impl<'a> Collector for TypeProcessor<'a> {
    fn process(&mut self, program: &Program, expr: &Expr, id: ExprId) {
        match expr {
            Expr::IntegerLiteral(_) => {
                let ty = Type::Int;
                let var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, var);
            }
            Expr::FloatLiteral(_) => {
                let ty = Type::Float;
                let var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, var);
            }
            Expr::BoolLiteral(_) => {
                let ty = Type::Bool;
                let var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, var);
            }
            Expr::StringLiteral(_) => {
                let ty = Type::String;
                let var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, var);
            }
            Expr::If(_, true_branch, _) => {
                let true_var = self.get_type_var_for_expr(true_branch);
                self.type_of_exprs.insert(id, true_var);
            }
            Expr::StaticFunctionCall(_, _) => {
                let ty = self.type_store.get_unique_type_arg_type();
                let result_var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, result_var);
            }
            Expr::Tuple(items) => {
                let items: Vec<_> = items
                    .iter()
                    .map(|i| self.get_type_var_for_expr(i))
                    .collect();
                let ty = Type::Tuple(items);
                let var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, var);
            }
            Expr::Do(items) => {
                let last = items.last().expect("Empty do");
                let var = self.get_type_var_for_expr(last);
                self.type_of_exprs.insert(id, var);
            }
            Expr::Bind(_, _) => {
                let ty = Type::Tuple(vec![]);
                let var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, var);
            }
            Expr::ExprValue(expr_id) => {
                let var = self.get_type_var_for_expr(expr_id);
                self.type_of_exprs.insert(id, var);
            }
            Expr::DynamicFunctionCall(_, _) => {
                let ty = self.type_store.get_unique_type_arg_type();
                let result_var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, result_var);
            }
            Expr::ArgRef(index) => {
                let arg_var =
                    self.function_args.get(&index.id).expect("Missing arg set")[index.index];
                self.type_of_exprs.insert(id, arg_var);
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
                let mut type_vars = Vec::new();
                for _ in 0..lambda_info.arg_count {
                    let ty = self.type_store.get_unique_type_arg_type();
                    let var = self.type_store.add_var(ty);
                    type_vars.push(var);
                    args.push(var);
                }
                let lambda_result_type = self.type_store.get_unique_type_arg_type();
                let lambda_result_type_var = self.type_store.add_var(lambda_result_type);
                type_vars.push(lambda_result_type_var);
                self.function_args.insert(*lambda_id, args);
                /*
                let lambda_function_type = FunctionType::new(type_vars);
                let ty = Type::Function(lambda_function_type);
                let result_var = self.type_store.add_var(ty);
                self.type_of_exprs.insert(id, result_var);
                */
                unimplemented!()
            }
            Expr::LambdaCapturedArgRef(arg_ref) => {
                let var = self
                    .captured_function_args
                    .get(&arg_ref.id)
                    .expect("Missing lambda arg set")[arg_ref.index];
                self.type_of_exprs.insert(id, var);
            }
            Expr::FieldAccess(..) => {}
            Expr::TupleFieldAccess(..) => {}
        }
    }
}
