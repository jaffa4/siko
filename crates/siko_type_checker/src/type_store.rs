use super::type_variable::TypeVariable;
use crate::types::Type;
use siko_ir::class::ClassId;
use siko_util::format_list;
use siko_util::Collector;
use siko_util::Counter;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub struct CloneContext<'a> {
    vars: BTreeMap<TypeVariable, TypeVariable>,
    args: BTreeMap<usize, usize>,
    indices: BTreeMap<TypeIndex, TypeIndex>,
    pub type_store: &'a mut TypeStore,
    verbose: bool,
}

impl<'a> CloneContext<'a> {
    pub fn new(type_store: &'a mut TypeStore, verbose: bool) -> CloneContext<'a> {
        CloneContext {
            vars: BTreeMap::new(),
            args: BTreeMap::new(),
            indices: BTreeMap::new(),
            type_store: type_store,
            verbose: verbose,
        }
    }

    pub fn var(&mut self, var: TypeVariable) -> TypeVariable {
        let r = CloneContext::build_var(self.type_store, &mut self.vars, var);
        if self.verbose {
            println!("Cloning var {} -> {}", var, r);
        }
        r
    }

    pub fn arg(&mut self, arg: usize) -> usize {
        let r = CloneContext::build_arg(self.type_store, &mut self.args, arg);
        if self.verbose {
            println!("Cloning arg {} -> {}", arg, r);
        }
        r
    }

    pub fn index(&mut self, index: TypeIndex) -> TypeIndex {
        let r = CloneContext::build_index(self.type_store, &mut self.indices, index);
        if self.verbose {
            println!("Cloning index {} -> {}", index.id, r.id);
        }
        r
    }

    pub fn clone_var(&mut self, var: TypeVariable) -> TypeVariable {
        match self.vars.get(&var) {
            Some(v) => *v,
            None => {
                let new_var = self.var(var);
                let index = self.type_store.get_index(&var);
                let index = match self.indices.get(&index) {
                    Some(i) => *i,
                    None => {
                        let new_index = self.index(index);
                        let ty = self
                            .type_store
                            .indices
                            .get(&index)
                            .expect("type not found")
                            .clone();
                        let new_ty = ty.clone_type(self);
                        self.type_store.indices.insert(new_index, new_ty);
                        new_index
                    }
                };
                self.type_store.variables.insert(new_var, index);
                new_var
            }
        }
    }

    fn build_var(
        type_store: &mut TypeStore,
        vars: &mut BTreeMap<TypeVariable, TypeVariable>,
        var: TypeVariable,
    ) -> TypeVariable {
        let new_var = vars.entry(var).or_insert_with(|| type_store.allocate_var());
        *new_var
    }

    fn build_arg(
        type_store: &mut TypeStore,
        args: &mut BTreeMap<usize, usize>,
        arg: usize,
    ) -> usize {
        let new_arg = args
            .entry(arg)
            .or_insert_with(|| type_store.get_unique_type_arg());
        *new_arg
    }

    fn build_index(
        type_store: &mut TypeStore,
        indices: &mut BTreeMap<TypeIndex, TypeIndex>,
        index: TypeIndex,
    ) -> TypeIndex {
        let new_index = indices
            .entry(index)
            .or_insert_with(|| type_store.allocate_index());
        *new_index
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TypeIndex {
    pub id: usize,
}

impl TypeIndex {
    pub fn new(id: usize) -> TypeIndex {
        TypeIndex { id: id }
    }
}

pub struct TypeStore {
    variables: BTreeMap<TypeVariable, TypeIndex>,
    indices: BTreeMap<TypeIndex, Type>,
    var_counter: Counter,
    index_counter: Counter,
    arg_counter: Counter,
    pub class_names: BTreeMap<ClassId, String>,
}

impl TypeStore {
    pub fn new() -> TypeStore {
        TypeStore {
            variables: BTreeMap::new(),
            indices: BTreeMap::new(),
            var_counter: Counter::new(),
            index_counter: Counter::new(),
            arg_counter: Counter::new(),
            class_names: BTreeMap::new(),
        }
    }

    pub fn get_unique_type_arg(&mut self) -> usize {
        self.arg_counter.next()
    }

    pub fn get_unique_type_arg_type(&mut self) -> Type {
        let ty = Type::TypeArgument(self.arg_counter.next(), vec![]);
        ty
    }

    fn allocate_var(&mut self) -> TypeVariable {
        let type_var = TypeVariable::new(self.var_counter.next());
        type_var
    }

    fn allocate_index(&mut self) -> TypeIndex {
        let index = TypeIndex::new(self.index_counter.next());
        index
    }

    pub fn add_type(&mut self, ty: Type) -> TypeVariable {
        let type_var = self.allocate_var();
        let index = self.allocate_index();
        self.indices.insert(index, ty);
        self.variables.insert(type_var, index);
        type_var
    }

    pub fn add_var_and_type(&mut self, type_var: TypeVariable, ty: Type, index: TypeIndex) {
        self.indices.insert(index, ty);
        self.variables.insert(type_var, index);
    }

    pub fn get_index(&self, var: &TypeVariable) -> TypeIndex {
        self.variables
            .get(var)
            .expect("invalid type variable")
            .clone()
    }

    pub fn merge(&mut self, from: &TypeVariable, to: &TypeVariable) {
        let from_index = self.get_index(from);
        let to_index = self.get_index(to);
        for (_, value) in self.variables.iter_mut() {
            if *value == to_index {
                *value = from_index;
            }
        }
    }

    pub fn has_class_instance(&self, var: &TypeVariable, class_id: &ClassId) -> bool {
        false
    }

    pub fn unify(&mut self, primary: &TypeVariable, secondary: &TypeVariable) -> bool {
        let primary_type = self.get_type(primary);
        let secondary_type = self.get_type(secondary);

        let index1 = self.get_index(primary);
        let index2 = self.get_index(secondary);
        /*
        println!(
            "Unify vars t1:({}),{:?},{} t2:({}),{:?},{}",
            primary, primary_type, index1.id, secondary, secondary_type, index2.id,
        );
        */
        if index1 == index2 {
            return true;
        }
        match (&primary_type, &secondary_type) {
            (Type::Int, Type::Int) => {}
            (Type::String, Type::String) => {}
            (Type::Bool, Type::Bool) => {}
            (
                Type::TypeArgument(_, primary_constraints),
                Type::TypeArgument(_, secondary_constraints),
            ) => {
                let mut merged_constraints = primary_constraints.clone();
                merged_constraints.extend(secondary_constraints);
                merged_constraints.sort();
                merged_constraints.dedup();
                let merged_type =
                    Type::TypeArgument(self.get_unique_type_arg(), merged_constraints);
                let merged_type_var = self.add_type(merged_type);
                self.merge(&merged_type_var, primary);
                self.merge(&merged_type_var, secondary);
            }
            (
                Type::TypeArgument(_, primary_constraints),
                Type::FixedTypeArgument(_, _, secondary_constraints),
            ) => {
                for c in primary_constraints {
                    if !secondary_constraints.contains(c) {
                        return false;
                    }
                }
                self.merge(secondary, primary);
            }
            (
                Type::FixedTypeArgument(_, _, primary_constraints),
                Type::TypeArgument(_, secondary_constraints),
            ) => {
                for c in secondary_constraints {
                    if !primary_constraints.contains(c) {
                        return false;
                    }
                }
                self.merge(primary, secondary);
            }
            (Type::TypeArgument(_, constraints), _) => {
                for c in constraints {
                    if !self.has_class_instance(secondary, c) {
                        return false;
                    }
                }
                self.merge(secondary, primary);
            }
            (_, Type::TypeArgument(_, constraints)) => {
                for c in constraints {
                    if !self.has_class_instance(primary, c) {
                        return false;
                    }
                }
                self.merge(primary, secondary);
            }
            (Type::Tuple(type_vars1), Type::Tuple(type_vars2)) => {
                if type_vars1.len() != type_vars2.len() {
                    return false;
                } else {
                    for (v1, v2) in type_vars1.iter().zip(type_vars2.iter()) {
                        if !self.unify(v1, v2) {
                            return false;
                        }
                    }
                }
            }
            (Type::Function(f1), Type::Function(f2)) => {
                if !self.unify(&f1.from, &f2.from) {
                    return false;
                }
                if !self.unify(&f1.to, &f2.to) {
                    return false;
                }
            }
            (Type::Named(_, id1, type_vars1), Type::Named(_, id2, type_vars2)) => {
                if id1 != id2 {
                    return false;
                }
                if type_vars1.len() != type_vars2.len() {
                    return false;
                } else {
                    for (v1, v2) in type_vars1.iter().zip(type_vars2.iter()) {
                        if !self.unify(v1, v2) {
                            return false;
                        }
                    }
                }
            }
            _ => {
                return false;
            }
        }
        return true;
    }

    pub fn get_type(&self, var: &TypeVariable) -> Type {
        let index = self.get_index(var);
        self.indices
            .get(&index)
            .expect("invalid type index")
            .clone()
    }

    pub fn get_resolved_type_string(&self, var: &TypeVariable) -> String {
        if self.is_recursive(*var) {
            return format!("<recursive type>");
        }
        let ty = self.get_type(var);
        let mut vars = BTreeSet::new();
        let mut args = BTreeSet::new();
        let mut indices = BTreeSet::new();
        let mut constraints = Collector::new();
        ty.collect(&mut vars, &mut args, &mut indices, &mut constraints, self);
        let mut type_args = BTreeMap::new();
        let mut next_char = 'a' as u32;
        for arg in args {
            let c = std::char::from_u32(next_char).expect("Invalid char");
            type_args.insert(arg, format!("{}", c));
            next_char += 1;
        }
        let mut constraint_strings = Vec::new();
        for (c, classes) in constraints.items {
            for class_id in classes {
                let class_name = self
                    .class_names
                    .get(&class_id)
                    .expect("Class name not found");
                let c_str = format!(
                    "{} {}",
                    class_name,
                    type_args.get(&c).expect("Type arg not found")
                );
                constraint_strings.push(c_str);
            }
        }

        let prefix = if constraint_strings.is_empty() {
            format!("")
        } else {
            format!("({}) => ", format_list(&constraint_strings[..]))
        };
        let type_str = ty.as_string(self, false, &type_args);
        format!("{}{}", prefix, type_str)
    }

    pub fn get_new_type_var(&mut self) -> TypeVariable {
        let ty = self.get_unique_type_arg_type();
        let var = self.add_type(ty);
        var
    }

    pub fn create_clone_context(&mut self, verbose: bool) -> CloneContext {
        CloneContext::new(self, verbose)
    }

    pub fn clone_type_var_simple(&mut self, var: TypeVariable) -> TypeVariable {
        let mut context = self.create_clone_context(false);
        let ty = context.type_store.get_type(&var);
        let new_ty = ty.clone_type(&mut context);
        context.type_store.add_type(new_ty)
    }

    pub fn is_recursive(&self, var: TypeVariable) -> bool {
        let ty = self.get_type(&var);
        let vars = vec![var];
        ty.check_recursion(&vars, self)
    }

    #[allow(unused)]
    pub fn dump(&self) {
        for (var, idx) in &self.variables {
            let ty = self.indices.get(idx).unwrap();
            println!("{} {} {}", var, idx.id, ty);
        }
    }

    #[allow(unused)]
    pub fn debug_var(&self, var: &TypeVariable) -> String {
        let ty = self.get_type(var);
        format!(
            "{}:{}({})",
            var,
            self.get_index(var).id,
            ty.debug_dump(self)
        )
    }
}
