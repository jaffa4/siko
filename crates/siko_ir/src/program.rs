use crate::class::Class;
use crate::class::ClassId;
use crate::class::ClassMember;
use crate::class::ClassMemberId;
use crate::class::Instance;
use crate::class::InstanceId;
use crate::data::Adt;
use crate::data::TypeDef;
use crate::data::TypeDefId;
use crate::expr::Expr;
use crate::expr::ExprId;
use crate::function::Function;
use crate::function::FunctionId;
use crate::instance_resolution_cache::InstanceResolutionCache;
use crate::pattern::Pattern;
use crate::pattern::PatternId;
use crate::type_signature::TypeSignature;
use crate::type_signature::TypeSignatureId;
use crate::type_var_generator::TypeVarGenerator;
use crate::types::Type;
use crate::unifier::Unifier;
use siko_constants::BOOL_MODULE_NAME;
use siko_constants::BOOL_TYPE_NAME;
use siko_constants::INT_MODULE_NAME;
use siko_constants::INT_TYPE_NAME;
use siko_constants::FLOAT_MODULE_NAME;
use siko_constants::FLOAT_TYPE_NAME;
use siko_constants::LIST_MODULE_NAME;
use siko_constants::LIST_TYPE_NAME;
use siko_constants::OPTION_MODULE_NAME;
use siko_constants::OPTION_TYPE_NAME;
use siko_constants::ORDERING_MODULE_NAME;
use siko_constants::ORDERING_TYPE_NAME;
use siko_constants::STRING_MODULE_NAME;
use siko_constants::STRING_TYPE_NAME;
use siko_location_info::item::ItemInfo;
use siko_util::ItemContainer;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug)]
pub struct Program {
    pub type_signatures: ItemContainer<TypeSignatureId, ItemInfo<TypeSignature>>,
    pub exprs: ItemContainer<ExprId, ItemInfo<Expr>>,
    pub functions: ItemContainer<FunctionId, Function>,
    pub typedefs: ItemContainer<TypeDefId, TypeDef>,
    pub patterns: ItemContainer<PatternId, ItemInfo<Pattern>>,
    pub classes: ItemContainer<ClassId, Class>,
    pub class_members: ItemContainer<ClassMemberId, ClassMember>,
    pub instances: ItemContainer<InstanceId, Instance>,
    pub instance_resolution_cache: Rc<RefCell<InstanceResolutionCache>>,
    pub expr_types: BTreeMap<ExprId, Type>,
    pub function_types: BTreeMap<FunctionId, Type>,
    pub class_names: BTreeMap<String, ClassId>,
    pub class_member_types: BTreeMap<ClassMemberId, (Type, Type)>,
    pub named_types: BTreeMap<String, BTreeMap<String, TypeDefId>>,
    pub type_var_generator: TypeVarGenerator,
}

impl Program {
    pub fn new(type_var_generator: TypeVarGenerator) -> Program {
        Program {
            type_signatures: ItemContainer::new(),
            exprs: ItemContainer::new(),
            functions: ItemContainer::new(),
            typedefs: ItemContainer::new(),
            patterns: ItemContainer::new(),
            classes: ItemContainer::new(),
            class_members: ItemContainer::new(),
            instances: ItemContainer::new(),
            instance_resolution_cache: Rc::new(RefCell::new(InstanceResolutionCache::new())),
            expr_types: BTreeMap::new(),
            function_types: BTreeMap::new(),
            class_names: BTreeMap::new(),
            class_member_types: BTreeMap::new(),
            named_types: BTreeMap::new(),
            type_var_generator: type_var_generator,
        }
    }

    pub fn get_list_type_id(&self) -> TypeDefId {
        let id = self.get_named_type(LIST_MODULE_NAME, LIST_TYPE_NAME);
        id
    }

    pub fn get_list_type(&self, ty: Type) -> Type {
        let id = self.get_list_type_id();
        Type::Named(LIST_TYPE_NAME.to_string(), id, vec![ty])
    }

    pub fn get_string_type(&self) -> Type {
        let id = self.get_named_type(STRING_MODULE_NAME, STRING_TYPE_NAME);
        Type::Named(STRING_TYPE_NAME.to_string(), id, Vec::new())
    }

    pub fn get_bool_type(&self) -> Type {
        let id = self.get_named_type(BOOL_MODULE_NAME, BOOL_TYPE_NAME);
        Type::Named(BOOL_TYPE_NAME.to_string(), id, Vec::new())
    }

    pub fn get_float_type(&self) -> Type {
        let id = self.get_named_type(FLOAT_MODULE_NAME, FLOAT_TYPE_NAME);
        Type::Named(FLOAT_TYPE_NAME.to_string(), id, Vec::new())
    }

    pub fn get_int_type(&self) -> Type {
        let id = self.get_named_type(INT_MODULE_NAME, INT_TYPE_NAME);
        Type::Named(INT_TYPE_NAME.to_string(), id, Vec::new())
    }

    pub fn get_ordering_type(&self) -> Type {
        let id = self.get_named_type(ORDERING_MODULE_NAME, ORDERING_TYPE_NAME);
        Type::Named(ORDERING_TYPE_NAME.to_string(), id, Vec::new())
    }

    pub fn get_option_type(&self, ty: Type) -> Type {
        let id = self.get_named_type(OPTION_MODULE_NAME, OPTION_TYPE_NAME);
        Type::Named(OPTION_TYPE_NAME.to_string(), id, vec![ty])
    }

    pub fn get_show_type(&self) -> Type {
    let class_id = self
        .class_names
        .get("Show")
        .expect("Show not found")
        .clone();
    let mut var = self.type_var_generator.clone();
    let index = var.get_new_index();
    Type::Var(index, vec![class_id])
    }

    pub fn get_adt_by_name(&self, module: &str, name: &str) -> &Adt {
        let id = self
            .named_types
            .get(module)
            .expect("Module not found")
            .get(name)
            .expect("Typedef not found");
        if let TypeDef::Adt(adt) = self.typedefs.get(id) {
            adt
        } else {
            unreachable!()
        }
    }

    pub fn get_named_type(&self, module: &str, name: &str) -> TypeDefId {
        self.named_types
            .get(module)
            .expect("Module not found")
            .get(name)
            .expect("Typedef not found")
            .clone()
    }

    pub fn get_module_and_name(&self, typedef_id: TypeDefId) -> (String, String) {
        let typedef = self.typedefs.get(&typedef_id);
        let (module, name) = match typedef {
            TypeDef::Adt(adt) => (adt.module.clone(), adt.name.clone()),
            TypeDef::Record(record) => (record.module.clone(), record.name.clone()),
        };
        (module, name)
    }

    pub fn get_unifier(&self) -> Unifier {
        Unifier::new(self.type_var_generator.clone())
    }
}
