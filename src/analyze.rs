use crate::builtins::expand_macro;
use crate::environment::*;
use crate::eval::*;
use crate::gc::*;
use crate::symbols::*;
use crate::types::*;

pub fn make_fn(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = Expression>,
    outer_syms: &Option<Symbols>,
) -> Result<Lambda, LispError> {
    if let Some(params) = args.next() {
        let (first, second) = (args.next(), args.next());
        let body = if let Some(first) = first {
            if let Some(second) = second {
                let mut body: Vec<Handle> = Vec::new();
                body.push(Expression::alloc_data_h(ExpEnum::Symbol(
                    "do",
                    SymLoc::None,
                )));
                body.push(first.into());
                body.push(second.into());
                for a in args {
                    body.push(a.into());
                }
                Expression::with_list(body)
            } else {
                first
            }
        } else {
            Expression::make_nil()
        };
        let params_d = params.get();
        let p_iter = if let ExpEnum::Vector(vec) = &params_d.data {
            Box::new(ListIter::new_list(&vec))
        } else {
            params.iter()
        };
        let mut params = Vec::new();
        let mut syms = Symbols::with_frame(environment, outer_syms);
        for p in p_iter {
            if let ExpEnum::Symbol(s, _) = p.get().data {
                params.push(s);
                if s != "&rest" {
                    syms.insert(s);
                }
            } else {
                return Err(LispError::new("fn: parameters must be symbols"));
            }
        }
        syms.insert("this-fn");
        analyze(environment, &body, &mut Some(syms.clone()))?;
        let body = body.handle_no_root();
        return Ok(Lambda {
            params,
            body,
            syms,
            namespace: environment.namespace.clone(),
        });
    }
    Err(LispError::new("fn: needs at least one form"))
}

fn declare_var(
    args: &mut dyn Iterator<Item = Expression>,
    syms: &mut Option<Symbols>,
) -> Result<(), LispError> {
    if let Some(syms) = syms {
        if let Some(key) = args.next() {
            let mut key_d = key.get_mut();
            match &mut key_d.data {
                ExpEnum::Symbol(s, location) => {
                    if syms.contains_symbol(s) {
                        Err(LispError::new(format!(
                            "var: Symbol {} already defined in scope.",
                            s
                        )))
                    } else {
                        let idx = syms.insert(*s);
                        location.replace(SymLoc::Stack(idx));
                        Ok(())
                    }
                }
                _ => Err(LispError::new(
                    "var: First form (binding key) must be a symbol",
                )),
            }
        } else {
            Err(LispError::new("var: Requires a symbol."))
        }
    } else {
        Err(LispError::new(
            "var: Using var outside a lambda not allowed.",
        ))
    }
}

fn patch_symbol(
    environment: &mut Environment,
    syms: &mut Option<Symbols>,
    name: &'static str,
    location: &mut SymLoc,
) {
    if let SymLoc::None = location {
        if let Some(syms) = syms {
            if let Some(idx) = syms.get(name) {
                location.replace(SymLoc::Stack(idx));
            } else if syms.can_capture(name) {
                let idx = syms.insert_capture(name);
                location.replace(SymLoc::Stack(idx));
            } else if let Some(exp) = syms.namespace().borrow().get_with_outer(name) {
                location.replace(SymLoc::Ref(exp));
            }
        } else if let Some(r) = environment.namespace.borrow().get_with_outer(name) {
            location.replace(SymLoc::Ref(r));
        }
    }
}

fn backquote_syms(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = Expression>,
    syms: &mut Option<Symbols>,
) {
    let mut last_unquote = false;
    for exp in args {
        let mut arg_d = exp.get_mut();
        match &mut arg_d.data {
            ExpEnum::Symbol(s, loc) => {
                if last_unquote {
                    last_unquote = false;
                    patch_symbol(environment, syms, s, loc);
                } else if s == &"," || s == &",@" {
                    last_unquote = true;
                }
            }
            ExpEnum::Vector(v) => {
                let mut ib = box_slice_it(v);
                backquote_syms(environment, &mut ib, syms);
            }
            ExpEnum::Pair(_, _) => {
                drop(arg_d);
                backquote_syms(environment, &mut exp.iter(), syms);
            }
            _ => {}
        }
    }
}

fn analyze_seq(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = Expression>,
    syms: &mut Option<Symbols>,
) -> Result<(Option<ExpEnum>, bool), LispError> {
    if let Some(car) = args.next() {
        let car_d = car.get();
        if let ExpEnum::Symbol(_s, _location) = &car_d.data {
            drop(car_d);
            let form = get_expression_look(environment, car.clone(), true);
            if let Some(form_exp) = form {
                let exp_d = form_exp.get();
                match &exp_d.data {
                    ExpEnum::DeclareFn => {
                        let lambda = make_fn(environment, args, syms)?;
                        let lambda: Expression = ExpEnum::Lambda(lambda).into();
                        return Ok((Some(ExpEnum::Wrapper(lambda.into())), false));
                    }
                    ExpEnum::DeclareMacro => {
                        let lambda = make_fn(environment, args, syms)?;
                        let lambda: Expression = ExpEnum::Macro(lambda).into();
                        return Ok((Some(ExpEnum::Wrapper(lambda.into())), false));
                    }
                    ExpEnum::DeclareDef => {}
                    ExpEnum::DeclareVar => {
                        declare_var(args, syms)?;
                    }
                    ExpEnum::Quote => {
                        if let ExpEnum::Symbol(name, loc) = &mut car.get_mut().data {
                            // Patch the 'quote' symbol so eval will work.
                            patch_symbol(environment, syms, name, loc);
                        }
                        if let Some(exp) = args.next() {
                            if args.next().is_none() {
                                // Don't need to analyze something quoted.
                                *exp.get().analyzed.borrow_mut() = true;
                                return Ok((None, false));
                            }
                        }
                        return Err(LispError::new("quote: Takes one form."));
                    }
                    ExpEnum::BackQuote => {
                        if let ExpEnum::Symbol(name, loc) = &mut car.get_mut().data {
                            patch_symbol(environment, syms, name, loc);
                        }
                        if let Some(arg) = args.next() {
                            match &arg.get().data {
                                ExpEnum::Symbol(s, _) if s == &"," => {
                                    if let Some(exp) = args.next() {
                                        if let ExpEnum::Symbol(s, loc) = &mut exp.get_mut().data {
                                            patch_symbol(environment, syms, s, loc);
                                        }
                                    } else {
                                        return Err(LispError::new(
                                            "back-quote: unquote with no form",
                                        ));
                                    }
                                }
                                ExpEnum::Vector(v) => {
                                    let mut ib = box_slice_it(v);
                                    backquote_syms(environment, &mut ib, syms);
                                }
                                ExpEnum::Pair(_, _) => {
                                    backquote_syms(environment, &mut arg.iter(), syms);
                                }
                                _ => {}
                            }
                        } else {
                            return Err(LispError::new("back-quote: takes one form"));
                        };
                        if args.next().is_some() {
                            return Err(LispError::new("back-quote: takes one form"));
                        }
                        return Ok((None, false));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok((None, true))
}

pub fn analyze(
    environment: &mut Environment,
    expression_in: &Expression,
    syms: &mut Option<Symbols>,
) -> Result<(), LispError> {
    if *expression_in.get().analyzed.borrow() {
        return Ok(());
    }
    let expression = expression_in.clone_root();
    if let Some(exp) = expand_macro(environment, &expression, false, 0)? {
        let mut exp_d = expression.get_mut();
        exp_d.meta = exp.get().meta;
        exp_d.meta_tags = exp.get().meta_tags.clone();
        exp_d.data.replace(exp.into());
    }
    {
        let exp_d = expression.get();
        if let ExpEnum::Vector(v) = &exp_d.data {
            let (exp_enum, do_list) = {
                let mut ib = box_slice_it(v);
                analyze_seq(environment, &mut ib, syms)?
            };
            if let Some(exp_enum) = exp_enum {
                drop(exp_d);
                expression.get_mut().data.replace(exp_enum);
            } else if do_list {
                for exp in v {
                    analyze(environment, &exp.into(), syms)?;
                }
            }
        } else if let ExpEnum::Pair(_car, _cdr) = &exp_d.data {
            drop(exp_d);
            let (exp_enum, do_list) = analyze_seq(environment, &mut expression.iter(), syms)?;
            if let Some(exp_enum) = exp_enum {
                expression.get_mut().data.replace(exp_enum);
            } else if do_list {
                for exp in expression.iter() {
                    analyze(environment, &exp, syms)?;
                }
            }
        } else if let ExpEnum::Symbol(_, _) = &exp_d.data {
            drop(exp_d);
            if let ExpEnum::Symbol(s, location) = &mut expression.get_mut().data {
                patch_symbol(environment, syms, s, location);
            }
        }
    }

    *expression.get().analyzed.borrow_mut() = true;
    Ok(())
}
