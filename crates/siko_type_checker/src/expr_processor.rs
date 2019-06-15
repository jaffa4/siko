use crate::common::create_general_function_type;
use crate::common::ClassMemberTypeInfo;
use crate::common::DependencyGroup;
use crate::common::FunctionTypeInfo;
use crate::common::RecordTypeInfo;
use crate::common::VariantTypeInfo;
use crate::error::TypecheckError;
use crate::type_processor::process_type_signature;
use crate::type_store::TypeStore;
use crate::type_variable::TypeVariable;
use crate::types::Type;
use crate::walker::walk_expr;
use crate::walker::Visitor;
use siko_ir::class::ClassMemberId;
use siko_ir::expr::Expr;
use siko_ir::expr::ExprId;
use siko_ir::expr::FieldAccessInfo;
use siko_ir::function::FunctionId;
use siko_ir::pattern::Pattern;
use siko_ir::pattern::PatternId;
use siko_ir::program::Program;
use siko_ir::types::TypeDefId;
use siko_location_info::item::LocationId;
use siko_util::format_list;
use std::collections::BTreeMap;

struct TypeVarCreator<'a, 'b> {
    expr_processor: &'a mut ExprProcessor<'b>,
}

impl<'a, 'b: 'a> TypeVarCreator<'a, 'b> {
    fn new(expr_processor: &'a mut ExprProcessor<'b>) -> TypeVarCreator<'a, 'b> {
        TypeVarCreator {
            expr_processor: expr_processor,
        }
    }
}

impl<'a, 'b> Visitor for TypeVarCreator<'a, 'b> {
    fn get_program(&self) -> &Program {
        &self.expr_processor.program
    }

    fn visit_expr(&mut self, expr_id: ExprId, _: &Expr) {
        self.expr_processor.create_type_var_for_expr(expr_id);
    }

    fn visit_pattern(&mut self, pattern_id: PatternId, _: &Pattern) {
        self.expr_processor.create_type_var_for_pattern(pattern_id);
    }
}

struct Unifier<'a, 'b> {
    expr_processor: &'a mut ExprProcessor<'b>,
    errors: &'a mut Vec<TypecheckError>,
    group: &'a DependencyGroup,
}

impl<'a, 'b: 'a> Unifier<'a, 'b> {
    fn new(
        expr_processor: &'a mut ExprProcessor<'b>,
        errors: &'a mut Vec<TypecheckError>,
        group: &'a DependencyGroup,
    ) -> Unifier<'a, 'b> {
        Unifier {
            expr_processor: expr_processor,
            errors: errors,
            group: group,
        }
    }
}

impl<'a, 'b> Unifier<'a, 'b> {
    fn get_function_type_var(&mut self, function_id: &FunctionId) -> TypeVariable {
        let type_info = self
            .expr_processor
            .function_type_info_map
            .get(function_id)
            .expect("Type info not found");
        if self.group.functions.contains(function_id) {
            return type_info.function_type;
        }
        let mut context = self.expr_processor.type_store.create_clone_context(false);
        context.clone_var(type_info.function_type)
    }

    fn get_class_member_type_var(&mut self, class_member_id: &ClassMemberId) -> TypeVariable {
        let type_info = self
            .expr_processor
            .class_member_type_info_map
            .get(class_member_id)
            .expect("Type info not found");
        let mut context = self.expr_processor.type_store.create_clone_context(false);
        context.clone_var(type_info.member_type_var)
    }

    fn check_literal_expr(&mut self, expr_id: ExprId, ty: Type) {
        let literal_var = self.expr_processor.type_store.add_type(ty);
        let var = self.expr_processor.lookup_type_var_for_expr(&expr_id);
        let location = self.expr_processor.program.exprs.get(&expr_id).location_id;
        self.expr_processor
            .unify_variables(&var, &literal_var, location, self.errors);
    }

    fn check_literal_pattern(&mut self, pattern_id: PatternId, ty: Type) {
        let literal_var = self.expr_processor.type_store.add_type(ty);
        let var = self.expr_processor.lookup_type_var_for_pattern(&pattern_id);
        let location = self
            .expr_processor
            .program
            .patterns
            .get(&pattern_id)
            .location_id;
        self.expr_processor
            .unify_variables(&var, &literal_var, location, self.errors);
    }

    #[allow(unused)]
    fn print_type(&self, msg: &str, var: &TypeVariable) {
        let ty = self.expr_processor.type_store.get_resolved_type_string(var);
        println!("{}: {}", msg, ty);
    }

    fn get_type_string(&self, var: &TypeVariable) -> String {
        self.expr_processor.type_store.get_resolved_type_string(var)
    }


    fn get_record_type_info(&mut self, record_id: &TypeDefId) -> RecordTypeInfo {
        let mut record_type_info = self
            .expr_processor
            .record_type_info_map
            .get(record_id)
            .expect("record tyoe info not found")
            .clone();
        let mut clone_context = self.expr_processor.type_store.create_clone_context(false);
        record_type_info.record_type = clone_context.clone_var(record_type_info.record_type);
        for field_type_var in &mut record_type_info.field_types {
            *field_type_var = clone_context.clone_var(*field_type_var);
        }
        record_type_info
    }

    fn match_patterns(&mut self, first: &PatternId, second: &PatternId) {
        let first_pattern_var = self.expr_processor.lookup_type_var_for_pattern(first);
        let second_pattern_var = self.expr_processor.lookup_type_var_for_pattern(second);
        let location = self.expr_processor.program.patterns.get(first).location_id;
        self.expr_processor.unify_variables(
            &first_pattern_var,
            &second_pattern_var,
            location,
            self.errors,
        );
    }

    fn match_pattern_with(&mut self, pattern: &PatternId, var: &TypeVariable) {
        let pattern_var = self.expr_processor.lookup_type_var_for_pattern(pattern);
        let location = self
            .expr_processor
            .program
            .patterns
            .get(pattern)
            .location_id;
        self.expr_processor
            .unify_variables(&var, &pattern_var, location, self.errors);
    }

    fn match_expr_with(&mut self, expr: &ExprId, var: &TypeVariable) {
        let expr_var = self.expr_processor.lookup_type_var_for_expr(expr);
        let location = self.expr_processor.program.exprs.get(expr).location_id;
        self.expr_processor
            .unify_variables(&var, &expr_var, location, self.errors);
    }

    fn static_function_call(
        &mut self,
        orig_function_type_var: TypeVariable,
        args: &Vec<ExprId>,
        expr_id: ExprId,
    ) {
        let mut function_type_var = orig_function_type_var;
        let orig_arg_vars: Vec<_> = args
            .iter()
            .map(|arg| self.expr_processor.lookup_type_var_for_expr(arg))
            .collect();
        let mut arg_vars = orig_arg_vars.clone();
        let mut failed = false;
        while !arg_vars.is_empty() {
            if let Type::Function(func_type) =
                self.expr_processor.type_store.get_type(&function_type_var)
            {
                let first_arg = arg_vars.first().unwrap();
                if !self
                    .expr_processor
                    .type_store
                    .unify(&func_type.from, first_arg)
                {
                    failed = true;
                    break;
                } else {
                    function_type_var = func_type.to;
                    arg_vars.remove(0);
                }
            } else {
                failed = true;
                break;
            }
        }
        let expr_var = self.expr_processor.lookup_type_var_for_expr(&expr_id);
        let location = self.expr_processor.program.exprs.get(&expr_id).location_id;
        if failed {
            let function_type_string = self.get_type_string(&orig_function_type_var);
            let arg_type_strings: Vec<_> = orig_arg_vars
                .iter()
                .map(|arg_var| self.get_type_string(arg_var))
                .collect();
            let arguments = format_list(&arg_type_strings[..]);
            let err =
                TypecheckError::FunctionArgumentMismatch(location, arguments, function_type_string);
            self.errors.push(err);
        } else {
            self.expr_processor.unify_variables(
                &expr_var,
                &function_type_var,
                location,
                self.errors,
            );
        }
    }
}

impl<'a, 'b> Visitor for Unifier<'a, 'b> {
    fn get_program(&self) -> &Program {
        &self.expr_processor.program
    }

    fn visit_expr(&mut self, expr_id: ExprId, expr: &Expr) {
        match expr {
            Expr::IntegerLiteral(_) => self.check_literal_expr(expr_id, Type::Int),
            Expr::StringLiteral(_) => self.check_literal_expr(expr_id, Type::String),
            Expr::BoolLiteral(_) => self.check_literal_expr(expr_id, Type::Bool),
            Expr::FloatLiteral(_) => self.check_literal_expr(expr_id, Type::Float),
            Expr::If(cond, true_branch, false_branch) => {
                let bool_var = self.expr_processor.type_store.add_type(Type::Bool);
                let false_var = self.expr_processor.lookup_type_var_for_expr(false_branch);
                self.match_expr_with(cond, &bool_var);
                self.match_expr_with(true_branch, &false_var);
                self.match_expr_with(&expr_id, &false_var);
            }
            Expr::StaticFunctionCall(function_id, args) => {
                let orig_function_type_var = self.get_function_type_var(function_id);
                self.static_function_call(orig_function_type_var, args, expr_id);
            }
            Expr::DynamicFunctionCall(func_expr, args) => {
                let mut gen_args = Vec::new();
                let (gen_func, gen_result) = create_general_function_type(
                    args.len(),
                    &mut gen_args,
                    &mut self.expr_processor.type_store,
                );
                let mut failed = false;
                let func_expr_var = self.expr_processor.lookup_type_var_for_expr(func_expr);
                let arg_vars: Vec<_> = args
                    .iter()
                    .map(|arg| self.expr_processor.lookup_type_var_for_expr(arg))
                    .collect();
                if !self
                    .expr_processor
                    .type_store
                    .unify(&func_expr_var, &gen_func)
                {
                    failed = true;
                } else {
                    for (arg, gen_arg) in arg_vars.iter().zip(gen_args.iter()) {
                        if !self.expr_processor.type_store.unify(arg, gen_arg) {
                            failed = true;
                            break;
                        }
                    }
                }
                let expr_var = self.expr_processor.lookup_type_var_for_expr(&expr_id);
                let location = self.expr_processor.program.exprs.get(&expr_id).location_id;
                if failed {
                    let function_type_string = self.get_type_string(&gen_func);
                    let arg_type_strings: Vec<_> = arg_vars
                        .iter()
                        .map(|arg_var| self.get_type_string(arg_var))
                        .collect();
                    let arguments = format_list(&arg_type_strings[..]);
                    let err = TypecheckError::FunctionArgumentMismatch(
                        location,
                        arguments,
                        function_type_string,
                    );
                    self.errors.push(err);
                } else {
                    self.expr_processor.unify_variables(
                        &expr_var,
                        &gen_result,
                        location,
                        self.errors,
                    );
                }
            }
            Expr::ArgRef(arg_ref) => {
                let func = self.expr_processor.program.functions.get(&arg_ref.id);
                let index = if arg_ref.captured {
                    arg_ref.index
                } else {
                    func.implicit_arg_count + arg_ref.index
                };
                let type_info = self
                    .expr_processor
                    .function_type_info_map
                    .get(&arg_ref.id)
                    .expect("Type info not found");
                let arg_var = type_info.args[index];
                self.match_expr_with(&expr_id, &arg_var);
            }
            Expr::Do(items) => {
                let last_item = items[items.len() - 1];
                let last_item_var = self.expr_processor.lookup_type_var_for_expr(&last_item);
                self.match_expr_with(&expr_id, &last_item_var);
            }
            Expr::Tuple(items) => {
                let vars: Vec<_> = items
                    .iter()
                    .map(|i| self.expr_processor.lookup_type_var_for_expr(i))
                    .collect();
                let tuple_ty = Type::Tuple(vars);
                let tuple_var = self.expr_processor.type_store.add_type(tuple_ty);
                self.match_expr_with(&expr_id, &tuple_var);
            }
            Expr::TupleFieldAccess(index, tuple_expr) => {
                let tuple_var = self.expr_processor.lookup_type_var_for_expr(tuple_expr);
                let tuple_ty = self.expr_processor.type_store.get_type(&tuple_var);
                let var = self.expr_processor.lookup_type_var_for_expr(&expr_id);
                let location = self.expr_processor.program.exprs.get(&expr_id).location_id;
                if let Type::Tuple(items) = tuple_ty {
                    if items.len() > *index {
                        self.expr_processor.unify_variables(
                            &items[*index],
                            &var,
                            location,
                            self.errors,
                        );
                        return;
                    }
                }
                let expected_type = format!("<tuple with at least {} item(s)>", index + 1);
                let found_type = self.get_type_string(&tuple_var);
                let err = TypecheckError::TypeMismatch(location, expected_type, found_type);
                self.errors.push(err);
            }
            Expr::Bind(pattern_id, rhs) => {
                let rhs_var = self.expr_processor.lookup_type_var_for_expr(rhs);
                self.match_pattern_with(pattern_id, &rhs_var);
                let tuple_ty = Type::Tuple(vec![]);
                let tuple_var = self.expr_processor.type_store.add_type(tuple_ty);
                self.match_expr_with(&expr_id, &tuple_var);
            }
            Expr::ExprValue(_, pattern_id) => {
                let expr_var = self.expr_processor.lookup_type_var_for_expr(&expr_id);
                self.match_pattern_with(pattern_id, &expr_var);
            }
            Expr::Formatter(fmt, args) => {
                let subs: Vec<_> = fmt.split("{}").collect();
                if subs.len() != args.len() + 1 {
                    let location = self.expr_processor.program.exprs.get(&expr_id).location_id;
                    let err = TypecheckError::InvalidFormatString(location);
                    self.errors.push(err);
                }
            }
            Expr::FieldAccess(infos, record_expr) => {
                let mut possible_records = Vec::new();
                let mut all_records = Vec::new();
                let record_expr_var = self.expr_processor.lookup_type_var_for_expr(record_expr);
                let location = self.expr_processor.program.exprs.get(&expr_id).location_id;
                let mut matches: Vec<(RecordTypeInfo, FieldAccessInfo)> = Vec::new();
                for info in infos {
                    let test_record_type_info = self.get_record_type_info(&info.record_id);
                    let record_type_info = self.get_record_type_info(&info.record_id);
                    let record = self
                        .expr_processor
                        .program
                        .typedefs
                        .get(&info.record_id)
                        .get_record();
                    all_records.push(record.name.clone());
                    let test_record_expr_var = self
                        .expr_processor
                        .type_store
                        .clone_type_var_simple(record_expr_var);
                    if self
                        .expr_processor
                        .type_store
                        .unify(&test_record_expr_var, &test_record_type_info.record_type)
                    {
                        possible_records.push(record.name.clone());
                        matches.push((record_type_info, info.clone()));
                    }
                }
                match matches.len() {
                    0 => {
                        let expected_type = format!("{}", all_records.join(" or "));
                        let found_type = self.get_type_string(&record_expr_var);
                        let err = TypecheckError::TypeMismatch(location, expected_type, found_type);
                        self.errors.push(err);
                    }
                    1 => {
                        let (record_type_info, field_info) = &matches[0];
                        let field_type_var = record_type_info.field_types[field_info.index];
                        self.match_expr_with(record_expr, &record_type_info.record_type);
                        self.match_expr_with(&expr_id, &field_type_var);
                    }
                    _ => {
                        let err = TypecheckError::AmbiguousFieldAccess(location, possible_records);
                        self.errors.push(err);
                    }
                }
            }
            Expr::CaseOf(body, cases) => {
                let body_var = self.expr_processor.lookup_type_var_for_expr(&body);
                for case in cases {
                    self.match_pattern_with(&case.pattern_id, &body_var);
                    let case_body_var = self.expr_processor.lookup_type_var_for_expr(&case.body);
                    self.match_expr_with(&expr_id, &case_body_var);
                }
            }
            Expr::RecordInitialization(type_id, items) => {
                let record_type_info = self.get_record_type_info(type_id);
                self.match_expr_with(&expr_id, &record_type_info.record_type);
                for (index, item) in items.iter().enumerate() {
                    let field_type_var = record_type_info.field_types[index];
                    self.match_expr_with(&item.expr_id, &field_type_var);
                }
            }
            Expr::RecordUpdate(record_expr_id, record_updates) => {
                let location_id = self.expr_processor.program.exprs.get(&expr_id).location_id;
                let record_expr_var = self.expr_processor.lookup_type_var_for_expr(record_expr_id);
                let record_expr_type = self.expr_processor.type_store.get_type(&record_expr_var);
                let real_record_type = if let Type::Named(_, id, _) = record_expr_type {
                    Some(id)
                } else {
                    None
                };
                let mut expected_records = Vec::new();
                let mut matching_update = None;
                for record_update in record_updates {
                    let record = self
                        .expr_processor
                        .program
                        .typedefs
                        .get(&record_update.record_id)
                        .get_record();
                    expected_records.push(record.name.clone());
                    if let Some(id) = real_record_type {
                        if record_update.record_id == id {
                            matching_update = Some(record_update);
                        }
                    }
                }
                match matching_update {
                    Some(update) => {
                        let record_type_info = self.get_record_type_info(&update.record_id);
                        self.match_expr_with(record_expr_id, &record_type_info.record_type);
                        for field_update in &update.items {
                            let field_var = record_type_info.field_types[field_update.index];
                            self.match_expr_with(&field_update.expr_id, &field_var);
                        }
                        self.match_expr_with(&expr_id, &record_type_info.record_type);
                    }
                    None => {
                        let expected_type = format!("{}", expected_records.join(" or "));
                        let found_type = self.get_type_string(&record_expr_var);
                        let err =
                            TypecheckError::TypeMismatch(location_id, expected_type, found_type);
                        self.errors.push(err);
                    }
                }
            }
            Expr::ClassFunctionCall(class_member_id, args) => {
                let orig_function_type_var = self.get_class_member_type_var(class_member_id);
                self.static_function_call(orig_function_type_var, args, expr_id);
            }
        }
    }

    fn visit_pattern(&mut self, pattern_id: PatternId, pattern: &Pattern) {
        match pattern {
            Pattern::Binding(_) => {}
            Pattern::Tuple(items) => {
                let vars: Vec<_> = items
                    .iter()
                    .map(|i| self.expr_processor.lookup_type_var_for_pattern(i))
                    .collect();
                let tuple_ty = Type::Tuple(vars);
                let tuple_var = self.expr_processor.type_store.add_type(tuple_ty);
                self.match_pattern_with(&pattern_id, &tuple_var);
            }
            Pattern::Record(typedef_id, items) => {
                let record_type_info = self.get_record_type_info(typedef_id);
                self.match_pattern_with(&pattern_id, &record_type_info.record_type);
                if record_type_info.field_types.len() != items.len() {
                    let location = self
                        .expr_processor
                        .program
                        .patterns
                        .get(&pattern_id)
                        .location_id;
                    let record = self
                        .expr_processor
                        .program
                        .typedefs
                        .get(typedef_id)
                        .get_record();
                    let err = TypecheckError::InvalidRecordPattern(
                        location,
                        record.name.clone(),
                        record_type_info.field_types.len(),
                        items.len(),
                    );
                    self.errors.push(err);
                } else {
                    for (index, item) in items.iter().enumerate() {
                        let field_var = record_type_info.field_types[index];
                        self.match_pattern_with(item, &field_var);
                    }
                }
            }
            Pattern::Variant(typedef_id, index, items) => {
                let variant_type_info = self
                    .expr_processor
                    .variant_type_info_map
                    .get(&(*typedef_id, *index))
                    .expect("Record type info not found");
                let mut clone_context = self.expr_processor.type_store.create_clone_context(false);
                let variant_var = clone_context.clone_var(variant_type_info.variant_type);
                let item_vars: Vec<_> = variant_type_info
                    .item_types
                    .iter()
                    .map(|v| clone_context.clone_var(*v))
                    .collect();
                self.match_pattern_with(&pattern_id, &variant_var);
                let location = self
                    .expr_processor
                    .program
                    .patterns
                    .get(&pattern_id)
                    .location_id;
                if item_vars.len() != items.len() {
                    let adt = self
                        .expr_processor
                        .program
                        .typedefs
                        .get(typedef_id)
                        .get_adt();
                    let variant = &adt.variants[*index];
                    let err = TypecheckError::InvalidVariantPattern(
                        location,
                        variant.name.clone(),
                        item_vars.len(),
                        items.len(),
                    );
                    self.errors.push(err);
                } else {
                    for (index, item) in items.iter().enumerate() {
                        let variant_item_var = item_vars[index];
                        self.match_pattern_with(item, &variant_item_var);
                    }
                }
            }
            Pattern::Guarded(inner, guard_expr_id) => {
                self.match_patterns(inner, &pattern_id);
                let bool_var = self.expr_processor.type_store.add_type(Type::Bool);
                self.match_expr_with(guard_expr_id, &bool_var);
            }
            Pattern::Wildcard => {}
            Pattern::IntegerLiteral(_) => {
                self.check_literal_pattern(pattern_id, Type::Int);
            }
            Pattern::FloatLiteral(_) => {
                self.check_literal_pattern(pattern_id, Type::Float);
            }
            Pattern::StringLiteral(_) => {
                self.check_literal_pattern(pattern_id, Type::String);
            }
            Pattern::BoolLiteral(_) => {
                self.check_literal_pattern(pattern_id, Type::Bool);
            }
            Pattern::Typed(inner, type_signature_id) => {
                self.match_patterns(inner, &pattern_id);
                let mut arg_map = BTreeMap::new();
                let pattern_signature_type_var = process_type_signature(
                    &mut self.expr_processor.type_store,
                    type_signature_id,
                    self.expr_processor.program,
                    &mut arg_map,
                );
                self.match_pattern_with(inner, &pattern_signature_type_var);
            }
        }
    }
}

pub struct ExprProcessor<'a> {
    type_store: TypeStore,
    expression_type_var_map: BTreeMap<ExprId, TypeVariable>,
    pattern_type_var_map: BTreeMap<PatternId, TypeVariable>,
    function_type_info_map: BTreeMap<FunctionId, FunctionTypeInfo>,
    record_type_info_map: BTreeMap<TypeDefId, RecordTypeInfo>,
    variant_type_info_map: BTreeMap<(TypeDefId, usize), VariantTypeInfo>,
    class_member_type_info_map: BTreeMap<ClassMemberId, ClassMemberTypeInfo>,
    program: &'a Program,
}

impl<'a> ExprProcessor<'a> {
    pub fn new(
        type_store: TypeStore,
        function_type_info_map: BTreeMap<FunctionId, FunctionTypeInfo>,
        record_type_info_map: BTreeMap<TypeDefId, RecordTypeInfo>,
        variant_type_info_map: BTreeMap<(TypeDefId, usize), VariantTypeInfo>,
        class_member_type_info_map: BTreeMap<ClassMemberId, ClassMemberTypeInfo>,
        program: &'a Program,
    ) -> ExprProcessor<'a> {
        ExprProcessor {
            type_store: type_store,
            expression_type_var_map: BTreeMap::new(),
            pattern_type_var_map: BTreeMap::new(),
            function_type_info_map: function_type_info_map,
            record_type_info_map: record_type_info_map,
            variant_type_info_map: variant_type_info_map,
            class_member_type_info_map: class_member_type_info_map,
            program: program,
        }
    }

    fn create_type_var_for_expr(&mut self, expr_id: ExprId) -> TypeVariable {
        let var = self.type_store.get_new_type_var();
        self.expression_type_var_map.insert(expr_id, var);
        var
    }

    fn create_type_var_for_pattern(&mut self, pattern_id: PatternId) -> TypeVariable {
        let var = self.type_store.get_new_type_var();
        self.pattern_type_var_map.insert(pattern_id, var);
        var
    }

    pub fn lookup_type_var_for_expr(&self, expr_id: &ExprId) -> TypeVariable {
        *self
            .expression_type_var_map
            .get(expr_id)
            .expect("Type var for expr not found")
    }

    pub fn lookup_type_var_for_pattern(&self, pattern_id: &PatternId) -> TypeVariable {
        *self
            .pattern_type_var_map
            .get(pattern_id)
            .expect("Type var for pattern not found")
    }

    pub fn process_dep_group(&mut self, group: &DependencyGroup, errors: &mut Vec<TypecheckError>) {
        for function in &group.functions {
            self.process_function(function, errors, group);
        }
    }

    pub fn process_function(
        &mut self,
        function_id: &FunctionId,
        errors: &mut Vec<TypecheckError>,
        group: &DependencyGroup,
    ) {
        let type_info = self
            .function_type_info_map
            .get(function_id)
            .expect("Function type info not found");
        let body = type_info.body.expect("body not found");
        let result_var = type_info.result;
        let mut type_var_creator = TypeVarCreator::new(self);
        walk_expr(&body, &mut type_var_creator);
        let mut unifier = Unifier::new(self, errors, group);
        walk_expr(&body, &mut unifier);
        let body_var = self.lookup_type_var_for_expr(&body);
        let body_location = self.program.exprs.get(&body).location_id;
        self.unify_variables(&result_var, &body_var, body_location, errors);
    }

    #[allow(unused)]
    pub fn dump_expression_types(&self, program: &Program) {
        for (expr_id, expr_info) in &program.exprs.items {
            let var = self.lookup_type_var_for_expr(expr_id);
            println!(
                "Expr: {}: {} -> {}",
                expr_id,
                expr_info.item,
                self.type_store.get_resolved_type_string(&var)
            );
        }
    }

    #[allow(unused)]
    pub fn dump_function_types(&self) {
        for (id, info) in &self.function_type_info_map {
            if info.body.is_none() {
                // continue;
            }
            println!(
                "{}/{}: {}",
                id,
                info.displayed_name,
                self.type_store
                    .get_resolved_type_string(&info.function_type)
            );
        }
    }

    pub fn check_recursive_types(&self, errors: &mut Vec<TypecheckError>) {
        for (_, info) in &self.function_type_info_map {
            if self.type_store.is_recursive(info.function_type) {
                let err = TypecheckError::RecursiveType(info.location_id);
                errors.push(err);
            }
        }
    }

    fn unify_variables(
        &mut self,
        expected: &TypeVariable,
        found: &TypeVariable,
        location: LocationId,
        errors: &mut Vec<TypecheckError>,
    ) -> bool {
        if !self.type_store.unify(&expected, &found) {
            let expected_type = self.type_store.get_resolved_type_string(&expected);
            let found_type = self.type_store.get_resolved_type_string(&found);
            let err = TypecheckError::TypeMismatch(location, expected_type, found_type);
            errors.push(err);
            false
        } else {
            true
        }
    }
}
