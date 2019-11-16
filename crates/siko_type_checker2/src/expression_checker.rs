use crate::common::FunctionTypeInfoStore;
use crate::common::RecordTypeInfo;
use crate::dependency_processor::DependencyGroup;
use crate::error::TypecheckError;
use crate::type_store::TypeStore;
use crate::type_var_generator::TypeVarGenerator;
use crate::types::ResolverContext;
use crate::types::Type;
use crate::unifier::Unifier;
use crate::util::get_list_type;
use siko_ir::expr::Expr;
use siko_ir::expr::ExprId;
use siko_ir::function::FunctionId;
use siko_ir::pattern::Pattern;
use siko_ir::pattern::PatternId;
use siko_ir::program::Program;
use siko_ir::types::TypeDefId;
use siko_ir::walker::Visitor;
use siko_location_info::item::LocationId;
use siko_util::format_list;
use std::collections::BTreeMap;

pub struct ExpressionChecker<'a> {
    program: &'a Program,
    group: &'a DependencyGroup<FunctionId>,
    type_store: &'a mut TypeStore,
    type_var_generator: TypeVarGenerator,
    function_type_info_store: &'a mut FunctionTypeInfoStore,
    record_type_info_map: &'a BTreeMap<TypeDefId, RecordTypeInfo>,
    errors: &'a mut Vec<TypecheckError>,
}

impl<'a> ExpressionChecker<'a> {
    pub fn new(
        program: &'a Program,
        group: &'a DependencyGroup<FunctionId>,
        type_store: &'a mut TypeStore,
        type_var_generator: TypeVarGenerator,
        function_type_info_store: &'a mut FunctionTypeInfoStore,
        record_type_info_map: &'a BTreeMap<TypeDefId, RecordTypeInfo>,
        errors: &'a mut Vec<TypecheckError>,
    ) -> ExpressionChecker<'a> {
        ExpressionChecker {
            program: program,
            group: group,
            type_store: type_store,
            type_var_generator: type_var_generator,
            function_type_info_store: function_type_info_store,
            record_type_info_map: record_type_info_map,
            errors: errors,
        }
    }

    fn unify(&mut self, ty1: &Type, ty2: &Type, location: LocationId) {
        let mut unifier = Unifier::new(self.type_var_generator.clone());
        if unifier.unify(ty1, ty2).is_err() {
            let ty_str1 = ty1.get_resolved_type_string(self.program);
            let ty_str2 = ty2.get_resolved_type_string(self.program);
            let err = TypecheckError::TypeMismatch(location, ty_str1, ty_str2);
            self.errors.push(err);
        } else {
            let cs = unifier.get_constraints();
            //dbg!(cs);
            self.type_store.apply(&unifier);
            for id in &self.group.items {
                let info = self.function_type_info_store.get_mut(id);
                info.apply(&unifier);
            }
        }
    }

    pub fn match_expr_with(&mut self, expr_id: ExprId, ty: &Type) {
        let expr_ty = self.type_store.get_expr_type(&expr_id).clone();
        let location = self.program.exprs.get(&expr_id).location_id;
        self.unify(ty, &expr_ty, location);
    }

    fn match_pattern_with(&mut self, pattern_id: PatternId, ty: &Type) {
        let pattern_ty = self.type_store.get_pattern_type(&pattern_id).clone();
        let location = self.program.patterns.get(&pattern_id).location_id;
        self.unify(ty, &pattern_ty, location);
    }

    fn match_expr_with_pattern(&mut self, expr_id: ExprId, pattern_id: PatternId) {
        let expr_ty = self.type_store.get_expr_type(&expr_id).clone();
        let pattern_ty = self.type_store.get_pattern_type(&pattern_id).clone();
        let location = self.program.patterns.get(&pattern_id).location_id;
        self.unify(&expr_ty, &pattern_ty, location);
    }

    fn match_exprs(&mut self, expr_id1: ExprId, expr_id2: ExprId) {
        let expr_ty1 = self.type_store.get_expr_type(&expr_id1).clone();
        let expr_ty2 = self.type_store.get_expr_type(&expr_id2).clone();
        let location = self.program.exprs.get(&expr_id2).location_id;
        self.unify(&expr_ty1, &expr_ty2, location);
    }

    fn check_function_call(&mut self, expr_id: ExprId, args: &Vec<ExprId>) {
        let func_type = self.type_store.get_func_type_for_expr(&expr_id);
        let arg_count = func_type.get_arg_count();
        if arg_count >= args.len() {
            let mut arg_types = Vec::new();
            func_type.get_args(&mut arg_types);
            for (arg, arg_type) in args.iter().zip(arg_types.iter()) {
                self.match_expr_with(*arg, arg_type);
            }
        } else {
            let mut context = ResolverContext::new(self.program);
            let function_type_string =
                func_type.get_resolved_type_string_with_context(&mut context);
            let arg_type_strings: Vec<_> = args
                .iter()
                .map(|arg| {
                    let ty = self.type_store.get_expr_type(arg);
                    ty.get_resolved_type_string_with_context(&mut context)
                })
                .collect();
            let location = self.program.exprs.get(&expr_id).location_id;
            let arguments = format_list(&arg_type_strings[..]);
            let err =
                TypecheckError::FunctionArgumentMismatch(location, arguments, function_type_string);
            self.errors.push(err);
        }
    }

    fn get_bool_type(&self) -> Type {
        let id = self.program.get_named_type("Data.Bool", "Bool");
        Type::Named("Bool".to_string(), id, Vec::new())
    }
}

impl<'a> Visitor for ExpressionChecker<'a> {
    fn get_program(&self) -> &Program {
        &self.program
    }

    fn visit_expr(&mut self, expr_id: ExprId, expr: &Expr) {
        //self.expr_processor.create_type_var_for_expr(expr_id);
        //println!("C {} {}", expr_id, expr);
        match expr {
            Expr::ArgRef(arg_ref) => {
                let func = self.program.functions.get(&arg_ref.id);
                let index = if arg_ref.captured {
                    arg_ref.index
                } else {
                    func.implicit_arg_count + arg_ref.index
                };
                let func_type_info = self.function_type_info_store.get(&arg_ref.id);
                let arg_ty = func_type_info.args[index].clone();
                self.match_expr_with(expr_id, &arg_ty);
            }
            Expr::Bind(pattern_id, rhs) => {
                self.match_expr_with_pattern(*rhs, *pattern_id);
                let expr_ty = Type::Tuple(Vec::new());
                self.match_expr_with(expr_id, &expr_ty);
            }
            Expr::BoolLiteral(_) => {}
            Expr::CaseOf(case_expr, cases) => {
                if let Some(first) = cases.first() {
                    self.match_exprs(expr_id, first.body);
                    for case in cases {
                        self.match_expr_with_pattern(*case_expr, case.pattern_id);
                        self.match_exprs(expr_id, case.body);
                    }
                }
            }
            Expr::ClassFunctionCall(_, args) => {
                self.check_function_call(expr_id, args);
            }
            Expr::DynamicFunctionCall(func_expr_id, args) => {
                let func_type = self.type_store.get_func_type_for_expr(&expr_id).clone();
                self.match_expr_with(*func_expr_id, &func_type);
                self.check_function_call(expr_id, args);
            }
            Expr::Do(items) => {
                let last_expr_id = items[items.len() - 1];
                self.match_exprs(expr_id, last_expr_id);
            }
            Expr::ExprValue(_, pattern_id) => {
                self.match_expr_with_pattern(expr_id, *pattern_id);
                let ty = self.type_store.get_expr_type(&expr_id).clone();
                //println!("expr value {} {}", ty, expr_id);
            }
            Expr::FieldAccess(infos, receiver_expr_id) => {
                let mut failed = true;
                let receiver_ty = self.type_store.get_expr_type(receiver_expr_id).clone();
                if let Type::Named(_, id, _) = receiver_ty {
                    if let Some(record_type_info) = self.record_type_info_map.get(&id) {
                        for info in infos {
                            if info.record_id == id {
                                let mut record_type_info =
                                    record_type_info.duplicate(&mut self.type_var_generator);
                                let mut unifier = Unifier::new(self.type_var_generator.clone());
                                if unifier
                                    .unify(&record_type_info.record_type, &receiver_ty)
                                    .is_ok()
                                {
                                    record_type_info.apply(&unifier);
                                    let field_ty = &record_type_info.field_types[info.index].0;
                                    self.match_expr_with(expr_id, field_ty);
                                    failed = false;
                                }
                            }
                        }
                    }
                }
                if failed {
                    let location = self.program.exprs.get(&receiver_expr_id).location_id;
                    let err = TypecheckError::TypeAnnotationNeeded(location);
                    self.errors.push(err);
                }
            }
            Expr::FloatLiteral(_) => {}
            Expr::If(cond, true_branch, false_branch) => {
                let bool_ty = self.get_bool_type();
                self.match_expr_with(*cond, &bool_ty);
                self.match_exprs(*true_branch, *false_branch);
                self.match_exprs(expr_id, *true_branch);
            }
            Expr::IntegerLiteral(_) => {}
            Expr::List(items) => {
                if let Some(first) = items.first() {
                    let ty = self.type_store.get_expr_type(first).clone();
                    let ty = get_list_type(self.program, ty);
                    self.match_expr_with(expr_id, &ty);
                    for item in items {
                        self.match_exprs(*first, *item);
                    }
                }
            }
            Expr::StaticFunctionCall(_, args) => {
                self.check_function_call(expr_id, args);
            }
            Expr::StringLiteral(_) => {}
            Expr::RecordInitialization(_, values) => {
                let record_type_info = self
                    .type_store
                    .get_record_type_info_for_expr(&expr_id)
                    .clone();
                for value in values {
                    let field_type = &record_type_info.field_types[value.index];
                    self.match_expr_with(value.expr_id, &field_type.0);
                }
            }
            Expr::Tuple(items) => {
                let item_types: Vec<_> = items
                    .iter()
                    .map(|item| self.type_store.get_expr_type(item).clone())
                    .collect();
                let tuple_ty = Type::Tuple(item_types);
                self.match_expr_with(expr_id, &tuple_ty);
            }
            _ => unimplemented!(),
        }
    }

    fn visit_pattern(&mut self, pattern_id: PatternId, pattern: &Pattern) {
        //println!("C {} {:?}", pattern_id, pattern);
        let ty = self.type_store.get_pattern_type(&pattern_id).clone();
        match pattern {
            Pattern::Binding(_) => {}
            Pattern::BoolLiteral(_) => {}
            Pattern::Tuple(items) => {
                if let Type::Tuple(item_types) = ty {
                    for (item, item_ty) in items.iter().zip(item_types.iter()) {
                        self.match_pattern_with(*item, item_ty);
                    }
                }
            }
            Pattern::Variant(_, _, args) => {}
            Pattern::Wildcard => {}
            _ => unimplemented!(),
        }
    }
}
