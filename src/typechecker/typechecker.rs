use crate::constants;
use crate::error::Error;
use crate::ir::function::FunctionInfo;
use crate::ir::program::Program;
use crate::typechecker::error::TypecheckError;
use crate::typechecker::expr_processor::ExprProcessor;
use crate::typechecker::function_processor::FunctionProcessor;

pub struct Typechecker {}

impl Typechecker {
    pub fn new() -> Typechecker {
        Typechecker {}
    }

    fn check_main(&self, program: &Program, errors: &mut Vec<TypecheckError>) {
        let mut main_found = false;

        for (_, function) in &program.functions {
            match &function.info {
                FunctionInfo::NamedFunction(info) => {
                    if info.module == constants::MAIN_MODULE
                        && info.name == constants::MAIN_FUNCTION
                    {
                        main_found = true;
                    }
                }
                _ => {}
            }
        }

        if !main_found {
            errors.push(TypecheckError::MainNotFound);
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), Error> {
        let mut errors = Vec::new();

        let function_processor = FunctionProcessor::new();

        let (type_store, function_type_info_map) =
            function_processor.process_functions(program, &mut errors);

        let mut expr_processor = ExprProcessor::new(type_store, function_type_info_map);

        expr_processor.process_expr_and_create_vars(program);

        expr_processor.check_constraints(program, &mut errors);

        expr_processor.dump_everything(program);

        self.check_main(program, &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::typecheck_err(errors))
        }
    }
}
