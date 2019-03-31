use crate::compiler::file_manager::FileManager;
use crate::location_info::filepath::FilePath;
use crate::location_info::location::Location;
use crate::location_info::location_info::LocationInfo;
use crate::location_info::location_set::LocationSet;
use crate::name_resolution::error::ResolverError;
use crate::parser::error::LexerError;
use crate::typechecker::error::TypecheckError;
use crate::util::format_list;
use colored::*;
use std::convert::From;
use std::io::Error as IoError;

fn s_from_range(chars: &[char], start: usize, end: usize) -> String {
    let subs = &chars[start..end];
    let s: String = subs.iter().collect();
    s
}

fn print_location_set(file_manager: &FileManager, location_set: &LocationSet) {
    let input = file_manager.content(&location_set.file_path);
    let lines: Vec<_> = input.lines().collect();
    let mut first = true;
    let pipe = "|";
    let mut last_line = 0;
    for (line_index, ranges) in &location_set.lines {
        last_line = *line_index;
        if first {
            first = false;
            println!(
                "{}{}:{}",
                "-- ".blue(),
                location_set.file_path.path.green(),
                format!("{}", line_index + 1).green()
            );
            if *line_index != 0 {
                let line = &lines[*line_index - 1];
                println!("{} {}", pipe.blue(), line);
            }
        }
        let line = &lines[*line_index];
        let chars: Vec<_> = line.chars().collect();
        let first = s_from_range(&chars[..], 0, ranges[0].start);
        print!("{} {}", pipe.blue(), first);
        for (index, range) in ranges.iter().enumerate() {
            let s = s_from_range(&chars[..], range.start, range.end);
            print!("{}", s.yellow());
            if index < ranges.len() - 1 {
                let s = s_from_range(&chars[..], range.end, ranges[index + 1].start);
                print!("{}", s);
            }
        }
        let last = s_from_range(&chars[..], ranges[ranges.len() - 1].end, chars.len());
        println!("{}", last);
    }
    if last_line + 1 < lines.len() {
        let line = &lines[last_line + 1];
        println!("{} {}", pipe.blue(), line);
    }
}

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    LexerError(Vec<LexerError>),
    ParseError(String, FilePath, Location),
    ResolverError(Vec<ResolverError>),
    TypecheckError(Vec<TypecheckError>),
    RuntimeError(String),
}

impl Error {
    pub fn get_single_lexer(self) -> LexerError {
        if let Error::LexerError(mut errs) = self {
            assert_eq!(errs.len(), 1);
            errs.pop().expect("err was empty")
        } else {
            unreachable!()
        }
    }

    pub fn lexer_err(s: String, file_path: FilePath, location: Location) -> LexerError {
        LexerError::General(s, file_path, location)
    }

    pub fn parse_err(s: String, file_path: FilePath, location: Location) -> Error {
        Error::ParseError(s, file_path, location)
    }

    pub fn resolve_err(errors: Vec<ResolverError>) -> Error {
        Error::ResolverError(errors)
    }

    pub fn typecheck_err(errors: Vec<TypecheckError>) -> Error {
        Error::TypecheckError(errors)
    }

    pub fn runtime_err(s: String) -> Error {
        Error::RuntimeError(s)
    }
    fn report_location(file_manager: &FileManager, file_path: &FilePath, location: &Location) {
        let input = file_manager.content(file_path);
        let lines: Vec<_> = input.lines().collect();
        println!(
            "--{}:{}",
            file_path.path.green(),
            format!("{}", location.line + 1).green()
        );
        let line = &lines[location.line];
        let chars: Vec<_> = line.chars().collect();
        let first = s_from_range(&chars[..], 0, location.span.start);
        print!("{}", first);
        let s = s_from_range(&chars[..], location.span.start, location.span.end);
        print!("{}", s.red());
        let last = s_from_range(&chars[..], location.span.end, chars.len());
        println!("{}", last);
    }

    fn report_error_base(
        msg: &str,
        file_manager: &FileManager,
        file_path: &FilePath,
        location: &Location,
    ) {
        let error = "ERROR:";
        println!("{} {}", error.red(), msg);
        Error::report_location(file_manager, file_path, location);
    }

    pub fn report_error(&self, file_manager: &FileManager, location_info: &LocationInfo) {
        let error = "ERROR:";
        match self {
            Error::LexerError(errors) => {
                for err in errors {
                    match err {
                        LexerError::General(msg, file_path, location) => {
                            Error::report_error_base(msg, file_manager, file_path, location);
                        }
                        LexerError::UnsupportedCharacter(c, location) => {
                            Error::report_error_base(
                                &format!(
                                    "{} unsupported character {}",
                                    error.red(),
                                    format!("{}", c).yellow()
                                ),
                                file_manager,
                                &location.file_path,
                                &location.location,
                            );
                        }
                        LexerError::InvalidIdentifier(identifier, location) => {
                            Error::report_error_base(
                                &format!(
                                    "{} invalid identifier {}",
                                    error.red(),
                                    identifier.yellow()
                                ),
                                file_manager,
                                &location.file_path,
                                &location.location,
                            );
                        }
                    }
                }
            }
            Error::ParseError(msg, file_path, location) => {
                Error::report_error_base(msg, file_manager, file_path, location);
            }
            Error::ResolverError(errs) => {
                for err in errs {
                    match err {
                        ResolverError::ModuleConflict(errors) => {
                            for (name, ids) in errors.iter() {
                                println!(
                                    "{} module name {} defined more than once",
                                    error.red(),
                                    name.yellow()
                                );
                                for id in ids.iter() {
                                    let location_set = location_info.get_item_location(id);
                                    print_location_set(file_manager, location_set);
                                }
                            }
                        }
                        ResolverError::ImportedModuleNotFound(name, id) => {
                            println!(
                                "{} imported module {} does not exist",
                                error.red(),
                                name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ImportedSymbolNotExportedByModule(name, module_name, id) => {
                            println!(
                                "{} imported symbol {} not exported by module {}",
                                error.red(),
                                name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::UnknownTypeName(var_name, id) => {
                            println!("{} unknown type name {}", error.red(), var_name.yellow());
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::TypeArgumentConflict(args, id) => {
                            println!(
                                "{} type argument(s) {} are not unique",
                                error.red(),
                                format_list(args).yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ArgumentConflict(args, id) => {
                            println!(
                                "{} argument(s) {} are not unique",
                                error.red(),
                                format_list(args).yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::LambdaArgumentConflict(args, id) => {
                            println!(
                                "{} lambda argument(s) {} are not unique",
                                error.red(),
                                format_list(args).yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::UnknownFunction(var_name, id) => {
                            println!("{} unknown function {}", error.red(), var_name.yellow());
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::AmbiguousName(var_name, id) => {
                            println!("{} ambiguous name {}", error.red(), var_name.yellow());
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::FunctionTypeNameMismatch(n1, n2, id) => {
                            println!(
                                "{} name mismatch in function type signature, {} != {}",
                                error.red(),
                                n1.yellow(),
                                n2.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::UnusedTypeArgument(args, id) => {
                            println!(
                                "{} unused type argument(s): {}",
                                error.red(),
                                format_list(args).yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::InternalModuleConflicts(module_name, name, locations) => {
                            println!(
                                "{} conflicting items named {} in module {}",
                                error.red(),
                                name.yellow(),
                                module_name.yellow()
                            );
                            for id in locations {
                                let location_set = location_info.get_item_location(id);
                                print_location_set(file_manager, location_set);
                            }
                        }
                        ResolverError::RecordFieldNotUnique(record_name, item_name, id) => {
                            println!(
                                "{} item name {} is not unique in record {}",
                                error.red(),
                                item_name.yellow(),
                                record_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::VariantNotUnique(adt_name, variant_name, id) => {
                            println!(
                                "{} variant name {} is not unique in type {}",
                                error.red(),
                                variant_name.yellow(),
                                adt_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ExportNoMatch(module_name, entity_name, id) => {
                            println!(
                                "{} item {} does not export anything in module {}",
                                error.red(),
                                entity_name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ExplicitlyImportedItemHidden(item_name, module_name, id) => {
                            println!(
                                "{} explicitly imported item {} is also hidden in module {}",
                                error.red(),
                                item_name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::IncorrectNameInImportedTypeConstructor(
                            module_name,
                            type_name,
                            id,
                        ) => {
                            println!(
                                "{} imported type {} is not exported by module {}",
                                error.red(),
                                type_name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ImportedRecordFieldNotExported(
                            record_name,
                            field_name,
                            id,
                        ) => {
                            println!(
                                "{} imported field {} is not exported for record {}",
                                error.red(),
                                field_name.yellow(),
                                record_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ExplicitlyImportedRecordFieldHidden(
                            field_name,
                            module_name,
                            id,
                        ) => {
                            println!(
                                "{} explicitly imported record field {} is also hidden in module {}",
                                error.red(),
                                field_name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ExplicitlyImportedTypeHidden(type_name, module_name, id) => {
                            println!(
                                "{} explicitly imported type {} is also hidden in module {}",
                                error.red(),
                                type_name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ExplicitlyImportedAdtVariantdHidden(
                            variant_name,
                            module_name,
                            id,
                        ) => {
                            println!(
                                "{} explicitly imported variant {} is also hidden in module {}",
                                error.red(),
                                variant_name.yellow(),
                                module_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::ImportedAdtVariantNotExported(
                            type_name,
                            variant_name,
                            id,
                        ) => {
                            println!(
                                "{} imported variant {} is not exported for type {}",
                                error.red(),
                                variant_name.yellow(),
                                type_name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::IncorrectTypeArgumentCount(
                            type_name,
                            expected,
                            found,
                            id,
                        ) => {
                            println!(
                                "{} incorrect type argument count for type {}",
                                error.red(),
                                type_name.yellow(),
                            );
                            let expected = format!("{}", expected);
                            let found = format!("{}", found);
                            println!("Expected: {}", expected.yellow());
                            println!("Found:    {}", found.yellow());
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        ResolverError::NameNotType(name, id) => {
                            println!("{} name is not a type {}", error.red(), name.yellow(),);
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                    }
                }
            }
            Error::RuntimeError(err) => {
                println!("Error: {}", err);
            }
            Error::TypecheckError(errs) => {
                for err in errs {
                    match err {
                        TypecheckError::UntypedExternFunction(name, id) => {
                            println!(
                                "{} extern function {} must have a type signature",
                                error.red(),
                                name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                        TypecheckError::FunctionTypeDependencyLoop => {
                            println!("{} function type dependency loop detected", error.red());
                        }
                        TypecheckError::TooManyArguments(id, name, expected, found) => {
                            println!(
                                "{} too many arguments given for {}",
                                error.red(),
                                name.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                            println!("Expected: {}", format!("{}", expected).yellow());
                            println!("Found: {}", format!("{}", found).yellow());
                        }
                        TypecheckError::TypeMismatch(id, expected, found) => {
                            println!("{} type mismatch in expression", error.red());
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                            println!("Expected: {}", expected.yellow());
                            println!("Found:    {}", found.yellow());
                        }
                        TypecheckError::FunctionArgumentMismatch(id, args, func) => {
                            println!("{} invalid argument(s)", error.red());
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                            println!("Argument(s):      {}", args.yellow());
                            println!("Function type:    {}", func.yellow());
                        }
                        TypecheckError::NotCallableType(id, ty) => {
                            println!(
                                "{} trying to call a non-callable type {}",
                                error.red(),
                                ty.yellow()
                            );
                            let location_set = location_info.get_item_location(id);
                            print_location_set(file_manager, location_set);
                        }
                    }
                }
            }
            _ => unimplemented!(),
        }
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::IoError(e)
    }
}
