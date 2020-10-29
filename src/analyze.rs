use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins::builtin_fn;
use crate::builtins::expand_macro;
use crate::environment::*;
use crate::eval::*;
use crate::gc::*;
use crate::types::*;

#[derive(Clone, Debug)]
struct Symbols {
    syms: HashMap<&'static str, usize>,
    count: usize,
    outer: Option<Rc<RefCell<Scope>>>,
}

impl Symbols {
    pub fn new() -> Symbols {
        Symbols {
            syms: HashMap::new(),
            count: 0,
            outer: None,
        }
    }

    pub fn new_with_outer(outer: Option<Rc<RefCell<Scope>>>) -> Symbols {
        Symbols {
            syms: HashMap::new(),
            count: 0,
            outer,
        }
    }

    pub fn contains_symbol(&self, key: &str) -> bool {
        self.syms.contains_key(key)
    }

    pub fn symbols(&self) -> std::collections::hash_map::Keys<'_, &'static str, usize> {
        self.syms.keys()
    }

    pub fn outer(&self) -> Option<Rc<RefCell<Scope>>> {
        self.outer.clone()
    }

    pub fn get(&self, key: &str) -> Option<usize> {
        if let Some(idx) = self.syms.get(key) {
            Some(*idx)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.syms.clear();
    }

    pub fn insert(&mut self, key: &'static str) {
        self.syms.insert(key, self.count);
        self.count += 1;
    }
}

pub fn analyze(
    environment: &mut Environment,
    expression_in: &Expression,
) -> Result<Expression, LispError> {
    let mut expression = expression_in.clone_root();
    // If we have a macro expand it and replace the expression with the expansion.
    if let Some(exp) = expand_macro(environment, &expression, false, 0)? {
        let mut nv: Vec<Handle> = Vec::new();
        let mut macro_replace = true;
        if let ExpEnum::Vector(list) = &exp.get().data {
            for item in list {
                let item: Expression = item.into();
                let item = item.resolve(environment)?;
                nv.push(item.into());
            }
        } else if let ExpEnum::Pair(_, _) = &exp.get().data {
            for item in exp.iter() {
                let item = item.resolve(environment)?;
                nv.push(item.into());
            }
        } else {
            expression = exp.clone();
            macro_replace = false;
        }
        if macro_replace {
            let mut exp_mut = expression.get_mut();
            match exp_mut.data {
                ExpEnum::Vector(_) => {
                    exp_mut.data.replace(ExpEnum::Vector(nv));
                    drop(exp_mut);
                    gc_mut().down_root(&expression);
                }
                ExpEnum::Pair(_, _) => {
                    exp_mut.data.replace(ExpEnum::cons_from_vec(&mut nv));
                    drop(exp_mut);
                    gc_mut().down_root(&expression);
                }
                _ => {}
            }
        }
    }
    let exp_a = expression.get();
    let exp_d = &exp_a.data;
    let ret = match exp_d {
        ExpEnum::Vector(v) => {
            if let Some((car, cdr)) = v.split_first() {
                let car: Expression = car.into();
                let car_d = car.get();
                if let ExpEnum::Symbol(_, _) = &car_d.data {
                    let form = get_expression(environment, car.clone());
                    if let Some(exp) = form {
                        if let ExpEnum::DeclareFn = &exp.exp.get().data {
                            let lambda = {
                                let mut ib = box_slice_it(cdr);
                                builtin_fn(environment, &mut ib)?
                            };
                            drop(exp_a);
                            expression
                                .get_mut()
                                .data
                                .replace(ExpEnum::Wrapper(lambda.into()));
                        } else if let ExpEnum::Macro(_) = &exp.exp.get().data {
                            panic!("Macros should have been expanded at this point!");
                        }
                    }
                } else if let ExpEnum::DeclareFn = &car_d.data {
                    let lambda = {
                        let mut ib = box_slice_it(cdr);
                        builtin_fn(environment, &mut ib)?
                    };
                    drop(exp_a);
                    expression
                        .get_mut()
                        .data
                        .replace(ExpEnum::Wrapper(lambda.into()));
                } else if let ExpEnum::Macro(_) = &car_d.data {
                    panic!("Macros should have been expanded at this point!");
                }
            }
            Ok(expression.clone())
        }
        ExpEnum::Pair(car, cdr) => {
            let car: Expression = car.into();
            if let ExpEnum::Symbol(_, _) = &car.get().data {
                let form = get_expression(environment, car.clone());
                if let Some(exp) = form {
                    if let ExpEnum::DeclareFn = &exp.exp.get().data {
                        let cdr: Expression = cdr.into();
                        let lambda = builtin_fn(environment, &mut cdr.iter())?;
                        drop(exp_a);
                        expression
                            .get_mut()
                            .data
                            .replace(ExpEnum::Wrapper(lambda.into()));
                    }
                }
            } else if let ExpEnum::DeclareFn = &car.get().data {
                let cdr: Expression = cdr.into();
                let lambda = builtin_fn(environment, &mut cdr.iter())?;
                drop(exp_a);
                expression
                    .get_mut()
                    .data
                    .replace(ExpEnum::Wrapper(lambda.into()));
            }
            Ok(expression.clone())
        }
        ExpEnum::Values(_v) => Ok(expression.clone()),
        ExpEnum::Nil => Ok(expression.clone()),
        ExpEnum::Symbol(_s, _) => Ok(expression.clone()),
        ExpEnum::HashMap(_) => Ok(expression.clone()),
        ExpEnum::String(_, _) => Ok(expression.clone()),
        ExpEnum::True => Ok(expression.clone()),
        ExpEnum::Float(_) => Ok(expression.clone()),
        ExpEnum::Int(_) => Ok(expression.clone()),
        ExpEnum::Char(_) => Ok(expression.clone()),
        ExpEnum::CodePoint(_) => Ok(expression.clone()),
        ExpEnum::Lambda(_) => Ok(expression.clone()),
        ExpEnum::Macro(_) => Ok(expression.clone()),
        ExpEnum::Function(_) => Ok(expression.clone()),
        ExpEnum::Process(_) => Ok(expression.clone()),
        ExpEnum::File(_) => Ok(expression.clone()),
        ExpEnum::LazyFn(_, _) => Ok(expression.clone()),
        ExpEnum::Wrapper(_exp) => Ok(expression.clone()),
        ExpEnum::DeclareDef => Ok(expression.clone()),
        ExpEnum::DeclareVar => Ok(expression.clone()),
        ExpEnum::DeclareFn => panic!("Invalid fn in analyze!"),
    };
    match ret {
        Ok(ret) => Ok(ret.clone_root()),
        Err(err) => Err(err),
    }
}