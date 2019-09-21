use crate::class::ClassId;
use crate::class::InstanceId;
use crate::function::FunctionId;
use siko_location_info::item::LocationId;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeSignatureId {
    pub id: usize,
}

impl From<usize> for TypeSignatureId {
    fn from(id: usize) -> TypeSignatureId {
        TypeSignatureId { id: id }
    }
}

#[derive(Debug, Clone)]
pub enum TypeSignature {
    Tuple(Vec<TypeSignatureId>),
    Function(TypeSignatureId, TypeSignatureId),
    TypeArgument(usize, String, Vec<ClassId>),
    Named(String, TypeDefId, Vec<TypeSignatureId>),
    Variant(String, Vec<TypeSignatureId>),
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct RecordField {
    pub name: String,
    pub type_signature_id: TypeSignatureId,
}

#[derive(Debug, Clone)]
pub struct Record {
    pub name: String,
    pub id: TypeDefId,
    pub type_args: Vec<usize>,
    pub fields: Vec<RecordField>,
    pub constructor: FunctionId,
    pub location_id: LocationId,
}

#[derive(Debug, Clone)]
pub struct VariantItem {
    pub type_signature_id: TypeSignatureId,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub items: Vec<VariantItem>,
    pub type_signature_id: TypeSignatureId,
    pub constructor: FunctionId,
}

#[derive(Debug, Clone)]
pub struct Adt {
    pub name: String,
    pub id: TypeDefId,
    pub type_args: Vec<usize>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone)]
pub enum TypeDef {
    Record(Record),
    Adt(Adt),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeDefId {
    pub id: usize,
}

impl fmt::Display for TypeDefId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TypeDefId({})", self.id)
    }
}

impl From<usize> for TypeDefId {
    fn from(id: usize) -> TypeDefId {
        TypeDefId { id: id }
    }
}

impl TypeDef {
    pub fn get_adt(&self) -> &Adt {
        if let TypeDef::Adt(adt) = self {
            &adt
        } else {
            unreachable!()
        }
    }

    pub fn get_record(&self) -> &Record {
        if let TypeDef::Record(record) = self {
            &record
        } else {
            unreachable!()
        }
    }

    pub fn get_mut_adt(&mut self) -> &mut Adt {
        if let TypeDef::Adt(ref mut adt) = self {
            adt
        } else {
            unreachable!()
        }
    }

    pub fn get_mut_record(&mut self) -> &mut Record {
        if let TypeDef::Record(ref mut record) = self {
            record
        } else {
            unreachable!()
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeId {
    pub id: usize,
}

impl fmt::Display for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TypeId({})", self.id)
    }
}

impl From<usize> for TypeId {
    fn from(id: usize) -> TypeId {
        TypeId { id: id }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ConcreteType {
    Tuple(Vec<ConcreteType>),
    Named(String, TypeDefId, Vec<ConcreteType>),
    Function(Box<ConcreteType>, Box<ConcreteType>),
}

#[derive(Debug)]
pub struct TypeInstanceResolver {
    pub instance_map: BTreeMap<ClassId, BTreeMap<ConcreteType, InstanceId>>,
}

impl TypeInstanceResolver {
    pub fn new() -> TypeInstanceResolver {
        TypeInstanceResolver {
            instance_map: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, class_id: ClassId, ty: ConcreteType, instance_id: InstanceId) {
        let types = self
            .instance_map
            .entry(class_id)
            .or_insert_with(|| BTreeMap::new());
        types.insert(ty, instance_id);
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Type {
    Tuple(Vec<TypeId>),
    Function(TypeId, TypeId),
    TypeArgument(usize, Vec<ClassId>),
    Named(String, TypeDefId, Vec<TypeId>),
}
