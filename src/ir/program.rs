use crate::ir::expr::Expr;
use crate::ir::expr::ExprId;
use crate::ir::expr::ExprInfo;
use crate::ir::function::Function;
use crate::ir::function::FunctionId;
use crate::ir::types::Adt;
use crate::ir::types::Record;
use crate::ir::types::TypeDef;
use crate::ir::types::TypeDefId;
use crate::ir::types::TypeInfo;
use crate::ir::types::TypeSignature;
use crate::ir::types::TypeSignatureId;
use crate::location_info::item::LocationId;

use crate::util::Counter;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Program {
    pub type_signatures: BTreeMap<TypeSignatureId, TypeInfo>,
    pub exprs: BTreeMap<ExprId, ExprInfo>,
    pub functions: BTreeMap<FunctionId, Function>,
    pub typedefs: BTreeMap<TypeDefId, TypeDef>,
    type_signature_id: Counter,
    expr_id: Counter,
    function_id: Counter,
    typedef_id: Counter,
}

impl Program {
    pub fn new() -> Program {
        Program {
            type_signatures: BTreeMap::new(),
            exprs: BTreeMap::new(),
            functions: BTreeMap::new(),
            typedefs: BTreeMap::new(),
            type_signature_id: Counter::new(),
            expr_id: Counter::new(),
            function_id: Counter::new(),
            typedef_id: Counter::new(),
        }
    }

    pub fn get_type_signature_id(&mut self) -> TypeSignatureId {
        TypeSignatureId {
            id: self.type_signature_id.next(),
        }
    }

    pub fn get_expr_id(&mut self) -> ExprId {
        ExprId {
            id: self.expr_id.next(),
        }
    }

    pub fn get_function_id(&mut self) -> FunctionId {
        FunctionId {
            id: self.function_id.next(),
        }
    }

    pub fn get_typedef_id(&mut self) -> TypeDefId {
        TypeDefId {
            id: self.typedef_id.next(),
        }
    }

    pub fn add_type_signature(&mut self, id: TypeSignatureId, type_info: TypeInfo) {
        self.type_signatures.insert(id, type_info);
    }

    pub fn get_type_signature(&self, id: &TypeSignatureId) -> &TypeSignature {
        &self
            .type_signatures
            .get(id)
            .expect("TypeSignature not found")
            .type_signature
    }

    pub fn get_type_signature_location(&self, id: &TypeSignatureId) -> LocationId {
        self.type_signatures
            .get(id)
            .expect("TypeSignature not found")
            .location_id
    }

    pub fn add_expr(&mut self, id: ExprId, expr_info: ExprInfo) {
        self.exprs.insert(id, expr_info);
    }

    pub fn get_expr(&self, id: &ExprId) -> &Expr {
        &self.exprs.get(id).expect("Expr not found").expr
    }

    pub fn get_expr_location(&self, id: &ExprId) -> LocationId {
        self.exprs.get(id).expect("Expr not found").location_id
    }

    pub fn add_function(&mut self, id: FunctionId, function: Function) {
        self.functions.insert(id, function);
    }

    pub fn get_function(&self, id: &FunctionId) -> &Function {
        &self.functions.get(id).expect("Function not found")
    }

    pub fn add_typedef(&mut self, id: TypeDefId, typedef: TypeDef) {
        self.typedefs.insert(id, typedef);
    }

    pub fn get_adt(&self, id: &TypeDefId) -> &Adt {
        if let TypeDef::Adt(adt) = self.typedefs.get(id).expect("TypeDefId not found") {
            adt
        } else {
            unreachable!()
        }
    }

    pub fn get_record(&self, id: &TypeDefId) -> &Record {
        if let TypeDef::Record(record) = self.typedefs.get(id).expect("TypeDefId not found") {
            record
        } else {
            unreachable!()
        }
    }
}
