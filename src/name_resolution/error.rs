use crate::location_info::item::LocationId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Debug)]
pub enum InternalModuleConflict {
    ItemConflict(String, Vec<LocationId>),
}

#[derive(Debug)]
pub enum ResolverError {
    ModuleConflict(BTreeMap<String, BTreeSet<LocationId>>),
    InternalModuleConflicts(BTreeMap<String, Vec<InternalModuleConflict>>),
    ImportedModuleNotFound(String, LocationId),
    ImportedSymbolNotExportedByModule(String, String, LocationId),
    UnknownTypeName(String, LocationId),
    TypeArgumentConflict(Vec<String>, LocationId),
    ArgumentConflict(Vec<String>, LocationId),
    LambdaArgumentConflict(Vec<String>, LocationId),
    UnknownFunction(String, LocationId),
    AmbiguousName(String, LocationId),
    FunctionTypeNameMismatch(String, String, LocationId),
    UnusedTypeArgument(Vec<String>, LocationId),
    RecordTypeNameMismatch(String, String, LocationId),
    RecordFieldNotUnique(String, String, LocationId),
    VariantNotUnique(String, String, LocationId),
    ExportedEntityDoesNotExist(String, String, LocationId),
    IncorrectNameInExportedTypeConstructor(String, String, LocationId),
    ExportedRecordFieldDoesNotExist(String, String, LocationId),
    ExportedAdtVariantDoesNotExist(String, String, LocationId),
    ExplicitlyImportedItemHidden(String, String, LocationId),
    ExplicitlyImportedRecordFieldHidden(String, String, LocationId),
    ExplicitlyImportedTypeHidden(String, String, LocationId),
    IncorrectNameInImportedTypeConstructor(String, String, LocationId),
    ImportedRecordFieldNotExported(String, String, LocationId),
    ExplicitlyImportedAdtVariantdHidden(String, String, LocationId),
    ImportedAdtVariantNotExported(String, String, LocationId),
}
