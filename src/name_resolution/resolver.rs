use crate::error::Error;
use crate::ir::function::Function as IrFunction;
use crate::ir::function::FunctionId as IrFunctionId;
use crate::ir::function::FunctionInfo;
use crate::ir::function::NamedFunctionInfo;
use crate::ir::program::Program as IrProgram;
use crate::ir::types::Adt;
use crate::ir::types::Record;
use crate::ir::types::TypeDef;
use crate::name_resolution::environment::Environment;
use crate::name_resolution::error::ResolverError;
use crate::name_resolution::export_processor::process_exports;
use crate::name_resolution::expr_processor::process_expr;
use crate::name_resolution::import_processor::process_imports;
use crate::name_resolution::item::Item;
use crate::name_resolution::lambda_helper::LambdaHelper;
use crate::name_resolution::module::Module;
use crate::name_resolution::type_processor::process_func_type;
use crate::syntax::function::FunctionBody as AstFunctionBody;
use crate::syntax::function::FunctionId as AstFunctionId;
use crate::syntax::module::Module as AstModule;
use crate::syntax::program::Program;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct Resolver {
    modules: BTreeMap<String, Module>,
}

impl Resolver {
    pub fn new() -> Resolver {
        Resolver {
            modules: BTreeMap::new(),
        }
    }

    fn register_module(
        &mut self,
        ast_module: &AstModule,
        modules: &mut BTreeMap<String, Vec<Module>>,
    ) {
        let mut module = Module::new(
            ast_module.id,
            ast_module.name.clone(),
            ast_module.location_id,
        );

        let mods = modules
            .entry(ast_module.name.get())
            .or_insert_with(Vec::new);
        mods.push(module);
    }

    fn process_module_conflicts(
        &mut self,
        modules: BTreeMap<String, Vec<Module>>,
    ) -> Result<(), Error> {
        let mut errors = Vec::new();
        let mut module_conflicts = BTreeMap::new();

        for (name, modules) in modules.iter() {
            if modules.len() > 1 {
                let ids = modules.iter().map(|m| m.location_id).collect();
                module_conflicts.insert(name.clone(), ids);
            }
        }

        if !module_conflicts.is_empty() {
            let e = ResolverError::ModuleConflict(module_conflicts);
            errors.push(e);
        }

        if errors.is_empty() {
            for (name, mods) in modules {
                let modules: Vec<Module> = mods;
                self.modules
                    .insert(name, modules.into_iter().next().expect("Empty module set"));
            }
            Ok(())
        } else {
            return Err(Error::resolve_err(errors));
        }
    }

    fn resolve_named_function_id(&self, named_id: &(String, String)) -> IrFunctionId {
        /*
        let m = self.modules.get(&named_id.0).expect("Module not found");
        let f = m
            .exported_functions
            .get(&named_id.1)
            .expect("Function not found");
        let ast_id = f[0].id.clone();
        let ir_function_id = self
            .function_map
            .get(&ast_id)
            .expect("Ir function not found");
        ir_function_id.clone()
        */
        unreachable!()
    }

    fn process_items_and_types(
        &mut self,
        program: &Program,
        errors: &mut Vec<ResolverError>,
        ir_program: &mut IrProgram,
    ) {
        for (name, module) in &mut self.modules {
            let ast_module = program.modules.get(&module.id).expect("Module not found");
            for record_id in &ast_module.records {
                let record = program.records.get(record_id).expect("Record not found");
                let ir_typedef_id = ir_program.get_typedef_id();
                let ir_record = Record {
                    name: record.name.clone(),
                    ast_record_id: *record_id,
                    id: ir_typedef_id,
                };
                let typedef = TypeDef::Record(ir_record);
                ir_program.add_typedef(ir_typedef_id, typedef);
                let items = module
                    .items
                    .entry(record.name.clone())
                    .or_insert_with(|| Vec::new());
                items.push(Item::Record(*record_id, ir_typedef_id));
            }
            for adt_id in &ast_module.adts {
                let adt = program.adts.get(adt_id).expect("Adt not found");
                let ir_typedef_id = ir_program.get_typedef_id();
                let ir_adt = Adt {
                    name: adt.name.clone(),
                    ast_adt_id: *adt_id,
                    id: ir_typedef_id,
                };
                let typedef = TypeDef::Adt(ir_adt);
                ir_program.add_typedef(ir_typedef_id, typedef);
                let items = module
                    .items
                    .entry(adt.name.clone())
                    .or_insert_with(|| Vec::new());
                items.push(Item::Adt(*adt_id, ir_typedef_id));
            }
            for function_id in &ast_module.functions {
                let function = program
                    .functions
                    .get(function_id)
                    .expect("Function not found");
                let ir_function_id = ir_program.get_function_id();
                let items = module
                    .items
                    .entry(function.name.clone())
                    .or_insert_with(|| Vec::new());
                items.push(Item::Function(function.id, ir_function_id));
            }
        }

        for (_, module) in &self.modules {
            for (name, items) in &module.items {
                if items.len() > 1 {
                    let mut locations = Vec::new();
                    for item in items {
                        match item {
                            Item::Function(id, _) => {
                                let function =
                                    program.functions.get(id).expect("Function not found");
                                locations.push(function.location_id);
                            }
                            Item::Record(id, _) => {
                                let record = program.records.get(id).expect("Record not found");
                                locations.push(record.location_id);
                            }
                            Item::Adt(id, _) => {
                                let adt = program.adts.get(id).expect("Adt not found");
                                locations.push(adt.location_id);
                            }
                        }
                    }
                    let err = ResolverError::InternalModuleConflicts(
                        module.name.get(),
                        name.clone(),
                        locations,
                    );
                    errors.push(err);
                }
            }
        }

        for (_, record) in &program.records {
            if record.name != record.data_name {
                let err = ResolverError::RecordTypeNameMismatch(
                    record.name.clone(),
                    record.data_name.clone(),
                    record.location_id,
                );
                errors.push(err);
            }
            let mut field_names = BTreeSet::new();
            for field in &record.fields {
                if !field_names.insert(field.name.clone()) {
                    let err = ResolverError::RecordFieldNotUnique(
                        record.name.clone(),
                        field.name.clone(),
                        record.location_id,
                    );
                    errors.push(err);
                }
            }
        }

        for (_, adt) in &program.adts {
            let mut variant_names = BTreeSet::new();
            for variant_id in &adt.variants {
                let variant = program.variants.get(variant_id).expect("Variant not found");
                if !variant_names.insert(variant.name.clone()) {
                    let err = ResolverError::VariantNotUnique(
                        adt.name.clone(),
                        variant.name.clone(),
                        adt.location_id,
                    );
                    errors.push(err);
                }
            }
        }
    }

    fn process_function(
        &self,
        program: &Program,
        ir_program: &mut IrProgram,
        function_id: &AstFunctionId,
        ir_function_id: IrFunctionId,
        module: &Module,
        errors: &mut Vec<ResolverError>,
    ) {
        let function = program
            .functions
            .get(function_id)
            .expect("Function not found");
        let mut type_signature_id = None;
        let mut body = None;
        if let Some(ty) = &function.func_type {
            if ty.name != function.name {
                let err = ResolverError::FunctionTypeNameMismatch(
                    ty.name.clone(),
                    function.name.clone(),
                    ty.location_id,
                );
                errors.push(err);
            }
            type_signature_id = process_func_type(ty, program, ir_program, module, errors);
        }
        if let AstFunctionBody::Expr(id) = function.body {
            let mut environment = Environment::new();
            let mut arg_names = BTreeSet::new();
            let mut conflicting_names = BTreeSet::new();
            for (index, arg) in function.args.iter().enumerate() {
                if !arg_names.insert(arg.clone()) {
                    conflicting_names.insert(arg.clone());
                }
                environment.add_arg(arg.clone(), ir_function_id, index);
            }
            if !conflicting_names.is_empty() {
                let err = ResolverError::ArgumentConflict(
                    conflicting_names.into_iter().collect(),
                    function.location_id.clone(),
                );
                errors.push(err);
            }
            let host_function = format!("{}/{}", module.name.get(), function.name);
            let mut lambda_helper = LambdaHelper::new(
                0,
                host_function,
                LambdaHelper::new_counter(),
                ir_function_id,
            );
            let body_id = process_expr(
                id,
                program,
                module,
                &mut environment,
                ir_program,
                errors,
                &mut lambda_helper,
            );
            body = Some(body_id);
        }

        let named_info = NamedFunctionInfo {
            body: body,
            name: function.name.clone(),
            module: module.name.get(),
            type_signature: type_signature_id,
            ast_function_id: function.id,
            location_id: function.location_id,
        };

        let ir_function = IrFunction {
            id: ir_function_id,
            arg_count: function.args.len(),
            info: FunctionInfo::NamedFunction(named_info),
        };
        ir_program.add_function(ir_function_id, ir_function);
    }

    pub fn resolve(&mut self, program: &Program) -> Result<IrProgram, Error> {
        let mut errors = Vec::new();

        let mut modules = BTreeMap::new();

        for ast_module in program.modules.values() {
            self.register_module(ast_module, &mut modules);
        }

        self.process_module_conflicts(modules)?;

        let mut ir_program = IrProgram::new();

        self.process_items_and_types(program, &mut errors, &mut ir_program);

        if !errors.is_empty() {
            return Err(Error::resolve_err(errors));
        }

        process_exports(&mut self.modules, program, &mut errors);

        if !errors.is_empty() {
            return Err(Error::resolve_err(errors));
        }

        process_imports(&mut self.modules, program, &mut errors);

        if !errors.is_empty() {
            return Err(Error::resolve_err(errors));
        }

        for (_, module) in &self.modules {
            for (_, items) in &module.items {
                for item in items {
                    match item {
                        Item::Function(ast_function_id, ir_function_id) => self.process_function(
                            program,
                            &mut ir_program,
                            ast_function_id,
                            *ir_function_id,
                            module,
                            &mut errors,
                        ),
                        _ => {}
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(Error::resolve_err(errors));
        }

        Ok(ir_program)
    }
}
