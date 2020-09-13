use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use crate::builtins::expand_macro;
use crate::builtins_util::*;
use crate::environment::*;
use crate::gc::*;
use crate::process::*;
use crate::reader::read;
use crate::types::*;

pub fn call_lambda(
    environment: &mut Environment,
    lambda_exp: Expression,
    args: &mut dyn Iterator<Item = Expression>,
    eval_args: bool,
) -> Result<Expression, LispError> {
    let lambda_d = lambda_exp.get();
    let lambda = if let ExpEnum::Atom(Atom::Lambda(l)) = &lambda_d.data {
        l
    } else {
        return Err(LispError::new("Lambda required."));
    };
    // DO NOT use ? in here, need to make sure the new_scope is popped off the
    // current_scope list before ending.
    let mut body: Expression = lambda.body.clone_root().into();
    let mut params: Expression = lambda.params.clone_root().into();
    let mut looping = true;
    let new_scope = build_new_scope(Some(lambda.capture.clone()));
    if let Err(err) = setup_args(
        environment,
        Some(&mut new_scope.borrow_mut()),
        &params,
        args,
        eval_args,
    ) {
        return Err(err);
    }
    environment.scopes.push(new_scope);
    set_expression_current(environment, "this-fn", None, lambda_exp.clone());
    let old_loose = environment.loose_symbols;
    environment.loose_symbols = false;
    let mut lambda; // = lambda;
    let mut lambda_int;
    let mut llast_eval: Option<Expression> = None;
    while looping {
        if environment.sig_int.load(Ordering::Relaxed) {
            environment.sig_int.store(false, Ordering::Relaxed);
            return Err(LispError::new("Lambda interupted by SIGINT."));
        }
        let last_eval = match eval_nr(environment, &body) {
            Ok(e) => e,
            Err(err) => {
                environment.scopes.pop();
                return Err(err);
            }
        };
        looping = environment.state.recur_num_args.is_some() && environment.exit_code.is_none();
        if looping {
            let recur_args = environment.state.recur_num_args.unwrap();
            environment.state.recur_num_args = None;
            if let ExpEnum::Vector(new_args) = &last_eval.get().data {
                if recur_args != new_args.len() {
                    environment.scopes.pop();
                    return Err(LispError::new("Called recur in a non-tail position."));
                }
                if !environment.scopes.is_empty() {
                    // Clear the old variables so no cruft is left on the recur.
                    environment.scopes.last().unwrap().borrow_mut().data.clear();
                }
                let mut ib = ListIter::new_list(&new_args);
                if let Err(err) = setup_args(environment, None, &params, &mut ib, false) {
                    environment.scopes.pop();
                    return Err(err);
                }
            }
        } else if environment.exit_code.is_none() {
            if let ExpEnum::LazyFn(lam, parts) = &last_eval.get().data {
                let lam_han: Expression = lam.into();
                let lam_d = lam_han.get();
                if let ExpEnum::Atom(Atom::Lambda(lam)) = &lam_d.data {
                    lambda_int = lam.clone();
                    lambda = &mut lambda_int;
                    body = lambda.body.clone_root().into();
                    params = lambda.params.clone_root().into();
                    looping = true;
                    environment.scopes.pop();
                    // scope is popped so can use ? now.
                    let new_scope = build_new_scope(Some(lambda.capture.clone()));
                    let mut ib = ListIter::new_list(&parts);
                    setup_args(
                        environment,
                        Some(&mut new_scope.borrow_mut()),
                        &params,
                        &mut ib,
                        false,
                    )?;
                    environment.scopes.push(new_scope);
                    set_expression_current(environment, "this-fn", None, lam_han.clone());
                }
            }
        }
        llast_eval = Some(last_eval);
    }
    environment.loose_symbols = old_loose;
    environment.scopes.pop();
    Ok(llast_eval
        .unwrap_or_else(Expression::make_nil)
        .resolve(environment)?)
}

fn exec_macro(
    environment: &mut Environment,
    sh_macro: &Lambda,
    args: &mut dyn Iterator<Item = Expression>,
) -> Result<Expression, LispError> {
    // DO NOT use ? in here, need to make sure the new_scope is popped off the
    // current_scope list before ending.
    let body: Expression = sh_macro.body.clone().into();
    let params: Expression = sh_macro.params.clone().into();
    let mut new_scope = Scope {
        data: HashMap::new(),
        outer: Some(sh_macro.capture.clone()),
        name: None,
    };
    match setup_args(environment, Some(&mut new_scope), &params, args, false) {
        Ok(_) => {}
        Err(err) => {
            return Err(err);
        }
    };

    environment.scopes.push(Rc::new(RefCell::new(new_scope)));
    let lazy = environment.allow_lazy_fn;
    environment.allow_lazy_fn = false;
    match eval(environment, &body) {
        Ok(expansion) => {
            let expansion = expansion.resolve(environment)?;
            environment.scopes.pop();
            let res = eval(environment, expansion);
            environment.allow_lazy_fn = lazy;
            res
        }
        Err(err) => {
            environment.allow_lazy_fn = lazy;
            environment.scopes.pop();
            Err(err)
        }
    }
}

pub fn fn_call(
    environment: &mut Environment,
    command: Expression,
    args: &mut dyn Iterator<Item = Expression>,
) -> Result<Expression, LispError> {
    match command.get().data.clone() {
        ExpEnum::Atom(Atom::Symbol(command)) => {
            if let Some(exp) = get_expression(environment, command) {
                match exp.exp.get().data.clone() {
                    ExpEnum::Function(c) if !c.is_special_form => (c.func)(environment, &mut *args),
                    ExpEnum::Atom(Atom::Lambda(_)) => {
                        if environment.allow_lazy_fn {
                            make_lazy(environment, exp.exp.clone(), args)
                        } else {
                            call_lambda(environment, exp.exp.clone(), args, true)
                        }
                    }
                    _ => {
                        let msg = format!(
                            "Symbol {} is not callable (or is macro or special form).",
                            command
                        );
                        Err(LispError::new(msg))
                    }
                }
            } else {
                let msg = format!(
                    "Symbol {} is not callable (or is macro or special form).",
                    command
                );
                Err(LispError::new(msg))
            }
        }
        ExpEnum::Atom(Atom::Lambda(_)) => {
            if environment.allow_lazy_fn {
                make_lazy(environment, command.clone(), args)
            } else {
                call_lambda(environment, command.clone(), args, true)
            }
        }
        ExpEnum::Atom(Atom::Macro(m)) => exec_macro(environment, &m, args),
        ExpEnum::Function(c) if !c.is_special_form => (c.func)(environment, &mut *args),
        _ => {
            let msg = format!(
                "Called an invalid command {}, type {}.",
                command.make_string(environment)?,
                command.display_type()
            );
            Err(LispError::new(msg))
        }
    }
}

fn make_lazy(
    environment: &mut Environment,
    lambda: Expression,
    args: &mut dyn Iterator<Item = Expression>,
) -> Result<Expression, LispError> {
    let mut parms: Vec<Handle> = Vec::new();
    for p in args {
        parms.push(eval(environment, p)?.into());
    }
    Ok(Expression::alloc(ExpObj {
        data: ExpEnum::LazyFn(lambda.into(), parms),
        meta: None,
        meta_tags: None,
    }))
}

pub fn box_slice_it<'a>(v: &'a [Handle]) -> Box<dyn Iterator<Item = Expression> + 'a> {
    Box::new(ListIter::new_slice(v))
}

fn fn_eval_lazy(
    environment: &mut Environment,
    expression: &Expression,
) -> Result<Expression, LispError> {
    let exp_d = expression.get();
    let e2: Expression;
    let e2_d;
    let (command, mut parts) = match &exp_d.data {
        ExpEnum::Vector(parts) => {
            let (command, parts) = match parts.split_first() {
                Some((c, p)) => (c, p),
                None => {
                    return Err(LispError::new("No valid command."));
                }
            };
            let ib = box_slice_it(parts);
            (command.clone(), ib)
        }
        ExpEnum::Pair(e1, ie2) => {
            e2 = ie2.into();
            e2_d = e2.get();
            let e2_iter = if let ExpEnum::Vector(list) = &e2_d.data {
                Box::new(ListIter::new_list(&list))
            } else {
                drop(e2_d);
                e2.iter()
            };
            (e1.clone(), e2_iter)
        }
        ExpEnum::Nil => return Ok(Expression::alloc_data(ExpEnum::Nil)),
        _ => return Err(LispError::new("Not a callable expression.")),
    };
    let command: Expression = command.into();
    let command = command.resolve(environment)?;
    let command_d = command.get();
    let allow_sys_com =
        environment.form_type == FormType::ExternalOnly || environment.form_type == FormType::Any;
    let allow_form =
        environment.form_type == FormType::FormOnly || environment.form_type == FormType::Any;
    match &command_d.data {
        ExpEnum::Atom(Atom::Symbol(command)) => {
            if command.is_empty() {
                return Ok(Expression::alloc_data(ExpEnum::Nil));
            }
            let form = get_expression(environment, &command);
            if let Some(exp) = form {
                match &exp.exp.get().data {
                    ExpEnum::Function(c) if allow_form => (c.func)(environment, &mut parts),
                    ExpEnum::Atom(Atom::Lambda(_)) if allow_form => {
                        if environment.allow_lazy_fn {
                            make_lazy(environment, exp.exp.clone(), &mut parts)
                        } else {
                            call_lambda(environment, exp.exp.clone(), &mut parts, true)
                        }
                    }
                    ExpEnum::Atom(Atom::Macro(m)) if allow_form => {
                        exec_macro(environment, &m, &mut parts)
                    }
                    ExpEnum::Atom(Atom::String(s, _)) if allow_sys_com => {
                        do_command(environment, s.trim(), &mut parts)
                    }
                    _ => {
                        if command.starts_with('$') {
                            if let ExpEnum::Atom(Atom::String(command, _)) =
                                &str_process(environment, command, true)?.get().data
                            {
                                do_command(environment, &command, &mut parts)
                            } else {
                                let msg = format!("Not a valid form {}, not found.", command);
                                Err(LispError::new(msg))
                            }
                        } else {
                            do_command(environment, command, &mut parts)
                        }
                    }
                }
            } else if allow_sys_com {
                if command.starts_with('$') {
                    if let ExpEnum::Atom(Atom::String(command, _)) =
                        &str_process(environment, command, true)?.get().data
                    {
                        do_command(environment, &command, &mut parts)
                    } else {
                        let msg = format!("Not a valid form {}, not found.", command);
                        Err(LispError::new(msg))
                    }
                } else {
                    do_command(environment, command, &mut parts)
                }
            } else {
                let msg = format!("Not a valid form {}, not found.", command);
                Err(LispError::new(msg))
            }
        }
        ExpEnum::Vector(_) => {
            drop(command_d); // Drop the lock on command.
            let allow_sys_com = environment.form_type == FormType::ExternalOnly
                || environment.form_type == FormType::Any;
            let com_exp = eval(environment, &command)?;
            let com_exp_d = com_exp.get();
            match &com_exp_d.data {
                ExpEnum::Atom(Atom::Lambda(_)) => {
                    if environment.allow_lazy_fn {
                        make_lazy(environment, com_exp.clone(), &mut parts)
                    } else {
                        call_lambda(environment, com_exp.clone(), &mut parts, true)
                    }
                }
                ExpEnum::Atom(Atom::Macro(m)) => exec_macro(environment, &m, &mut parts),
                ExpEnum::Function(c) => (c.func)(environment, &mut *parts),
                ExpEnum::Atom(Atom::String(s, _)) if allow_sys_com => {
                    do_command(environment, s.trim(), &mut parts)
                }
                _ => {
                    let msg = format!(
                        "Not a valid command {}, type {}.",
                        com_exp,
                        com_exp.display_type()
                    );
                    Err(LispError::new(msg))
                }
            }
        }
        ExpEnum::Pair(_, _) => {
            drop(command_d); // Drop the lock on command.
            let com_exp = eval(environment, &command)?;
            let com_exp_d = com_exp.get();
            match &com_exp_d.data {
                ExpEnum::Atom(Atom::Lambda(_)) => {
                    if environment.allow_lazy_fn {
                        make_lazy(environment, com_exp.clone(), &mut parts)
                    } else {
                        call_lambda(environment, com_exp.clone(), &mut parts, true)
                    }
                }
                ExpEnum::Atom(Atom::Macro(m)) => exec_macro(environment, &m, &mut parts),
                ExpEnum::Function(c) => (c.func)(environment, &mut *parts),
                ExpEnum::Atom(Atom::String(s, _)) if allow_sys_com => {
                    do_command(environment, s.trim(), &mut parts)
                }
                _ => {
                    let msg = format!(
                        "Not a valid command {}, type {}.",
                        com_exp,
                        com_exp.display_type()
                    );
                    Err(LispError::new(msg))
                }
            }
        }
        ExpEnum::Atom(Atom::Lambda(_)) => {
            if environment.allow_lazy_fn {
                make_lazy(environment, command.clone(), &mut parts)
            } else {
                call_lambda(environment, command.clone(), &mut parts, true)
            }
        }
        ExpEnum::Atom(Atom::Macro(m)) => exec_macro(environment, &m, &mut parts),
        ExpEnum::Function(c) => (c.func)(environment, &mut *parts),
        ExpEnum::Atom(Atom::String(s, _)) if allow_sys_com => {
            do_command(environment, s.trim(), &mut parts)
        }
        _ => {
            let msg = format!(
                "Not a valid command {}, type {}.",
                command.make_string(environment)?,
                command.display_type()
            );
            Err(LispError::new(msg))
        }
    }
}

fn str_process(
    environment: &mut Environment,
    string: &str,
    expand: bool,
) -> Result<Expression, LispError> {
    if expand && !environment.str_ignore_expand && string.contains('$') {
        let mut new_string = String::new();
        let mut last_ch = '\0';
        let mut in_var = false;
        let mut in_command = false;
        let mut command_depth: i32 = 0;
        let mut var_start = 0;
        for (i, ch) in string.chars().enumerate() {
            if in_var {
                if ch == '(' && var_start + 1 == i {
                    in_command = true;
                    in_var = false;
                    command_depth = 1;
                } else {
                    if ch == ' ' || ch == '"' || ch == ':' || (ch == '$' && last_ch != '\\') {
                        in_var = false;
                        match env::var(&string[var_start + 1..i]) {
                            Ok(val) => new_string.push_str(&val),
                            Err(_) => new_string.push_str(""),
                        }
                    }
                    if ch == ' ' || ch == '"' || ch == ':' {
                        new_string.push(ch);
                    }
                }
            } else if in_command {
                if ch == ')' && last_ch != '\\' {
                    command_depth -= 1;
                }
                if command_depth == 0 {
                    in_command = false;
                    let ast = read(environment, &string[var_start + 1..=i], None, false);
                    match ast {
                        Ok(ast) => {
                            environment.loose_symbols = true;
                            let old_out = environment.state.stdout_status.clone();
                            let old_err = environment.state.stderr_status.clone();
                            environment.state.stdout_status = Some(IOState::Pipe);
                            environment.state.stderr_status = Some(IOState::Pipe);

                            // Get out of a pipe for the str call if in one...
                            let data_in = environment.data_in.clone();
                            environment.data_in = None;
                            let in_pipe = environment.in_pipe;
                            environment.in_pipe = false;
                            let pipe_pgid = environment.state.pipe_pgid;
                            environment.state.pipe_pgid = None;
                            new_string.push_str(
                                eval(environment, ast)
                                    .map_err(|e| {
                                        environment.state.stdout_status = old_out.clone();
                                        environment.state.stderr_status = old_err.clone();
                                        e
                                    })?
                                    .as_string(environment)
                                    .map_err(|e| {
                                        environment.state.stdout_status = old_out.clone();
                                        environment.state.stderr_status = old_err.clone();
                                        e
                                    })?
                                    .trim(),
                            );
                            environment.state.stdout_status = old_out;
                            environment.state.stderr_status = old_err;
                            environment.data_in = data_in;
                            environment.in_pipe = in_pipe;
                            environment.state.pipe_pgid = pipe_pgid;
                            environment.loose_symbols = false;
                        }
                        Err(err) => return Err(LispError::new(err.reason)),
                    }
                } else if ch == '(' && last_ch != '\\' {
                    command_depth += 1;
                }
            } else if ch == '$' && last_ch != '\\' {
                in_var = true;
                var_start = i;
            } else if ch != '\\' {
                if last_ch == '\\' && ch != '$' {
                    new_string.push('\\');
                }
                new_string.push(ch);
            }
            last_ch = ch;
        }
        if in_var {
            match env::var(&string[var_start + 1..]) {
                Ok(val) => new_string.push_str(&val),
                Err(_) => new_string.push_str(""),
            }
        }
        if in_command {
            return Err(LispError::new(
                "Malformed command embedded in string (missing ')'?).",
            ));
        }
        if environment.interner.contains(&new_string) {
            Ok(Expression::alloc_data(ExpEnum::Atom(Atom::String(
                environment.interner.intern(&new_string).into(),
                None,
            ))))
        } else {
            Ok(Expression::alloc_data(ExpEnum::Atom(Atom::String(
                new_string.into(),
                None,
            ))))
        }
    } else if environment.interner.contains(string) {
        Ok(Expression::alloc_data(ExpEnum::Atom(Atom::String(
            environment.interner.intern(string).into(),
            None,
        ))))
    } else {
        Ok(Expression::alloc_data(ExpEnum::Atom(Atom::String(
            string.to_string().into(),
            None,
        ))))
    }
}

fn internal_eval(
    environment: &mut Environment,
    expression_in: &Expression,
) -> Result<Expression, LispError> {
    let mut expression = expression_in.clone_root();
    if environment.sig_int.load(Ordering::Relaxed) {
        environment.sig_int.store(false, Ordering::Relaxed);
        return Err(LispError::new("Script interupted by SIGINT."));
    }
    // exit was called so just return nil to unwind.
    if environment.exit_code.is_some() {
        return Ok(Expression::alloc_data(ExpEnum::Nil));
    }
    let in_recur = environment.state.recur_num_args.is_some();
    if in_recur {
        environment.state.recur_num_args = None;
        return Err(LispError::new("Called recur in a non-tail position."));
    }
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
        ExpEnum::Vector(_) => {
            drop(exp_a);
            environment.last_meta = expression.meta();
            fn_eval_lazy(environment, &expression)
        }
        ExpEnum::Values(v) => {
            if v.is_empty() {
                Ok(Expression::make_nil())
            } else {
                let v: Expression = (&v[0]).into();
                internal_eval(environment, &v)
            }
        }
        ExpEnum::Pair(_, _) => {
            drop(exp_a);
            environment.last_meta = expression.meta();
            fn_eval_lazy(environment, &expression)
        }
        ExpEnum::Nil => Ok(expression.clone()),
        ExpEnum::Atom(Atom::Symbol(s)) => {
            if s.starts_with('$') {
                match env::var(&s[1..]) {
                    Ok(val) => Ok(Expression::alloc_data(ExpEnum::Atom(Atom::String(
                        environment.interner.intern(&val).into(),
                        None,
                    )))),
                    Err(_) => Ok(Expression::alloc_data(ExpEnum::Nil)),
                }
            } else if s.starts_with(':') {
                // Got a keyword, so just be you...
                Ok(Expression::alloc_data(ExpEnum::Atom(Atom::Symbol(s))))
            } else if let Some(exp) = get_expression(environment, s) {
                let exp = &exp.exp;
                Ok(exp.clone())
            } else if environment.loose_symbols {
                str_process(environment, s, false)
            } else {
                let msg = format!("Symbol {} not found.", s);
                Err(LispError::new(msg))
            }
        }
        ExpEnum::HashMap(_) => Ok(expression.clone()),
        // If we have an iterator on the string then assume it is already processed and being used.
        // XXX TODO- verify this assumption is correct, maybe change when to process strings.
        ExpEnum::Atom(Atom::String(_, Some(_))) => Ok(expression.clone()),
        ExpEnum::Atom(Atom::String(string, _)) => str_process(environment, &string, true),
        ExpEnum::Atom(_) => Ok(expression.clone()),
        ExpEnum::Function(_) => Ok(Expression::alloc_data(ExpEnum::Nil)),
        ExpEnum::Process(_) => Ok(expression.clone()),
        ExpEnum::File(_) => Ok(Expression::alloc_data(ExpEnum::Nil)),
        ExpEnum::LazyFn(_, _) => {
            let int_exp = expression.clone().resolve(environment)?;
            eval(environment, int_exp)
        }
    };
    match ret {
        Ok(ret) => Ok(ret.clone_root()),
        Err(err) => Err(err),
    }
}

pub fn eval_nr(
    environment: &mut Environment,
    expression: impl AsRef<Expression>,
) -> Result<Expression, LispError> {
    let expression = expression.as_ref();
    if environment.return_val.is_some() {
        return Ok(Expression::alloc_data(ExpEnum::Nil));
    }
    if environment.state.eval_level > 500 {
        return Err(LispError::new("Eval calls to deep."));
    }
    environment.state.eval_level += 1;
    let tres = internal_eval(environment, expression);
    let mut result = if environment.state.eval_level == 1 && environment.return_val.is_some() {
        environment.return_val = None;
        Err(LispError::new("Return without matching block."))
    } else {
        tres
    };
    if let Err(err) = &mut result {
        if err.backtrace.is_none() {
            err.backtrace = Some(Vec::new());
        }
        if let Some(backtrace) = &mut err.backtrace {
            backtrace.push(expression.clone().into());
        }
    }
    environment.state.eval_level -= 1;
    environment.last_meta = None;
    result
}

pub fn eval(
    environment: &mut Environment,
    expression: impl AsRef<Expression>,
) -> Result<Expression, LispError> {
    let expression = expression.as_ref();
    eval_nr(environment, expression)?.resolve(environment)
}

pub fn eval_data(environment: &mut Environment, data: ExpEnum) -> Result<Expression, LispError> {
    let data = Expression::alloc_data(data);
    eval(environment, data)
}

pub fn eval_no_values(
    environment: &mut Environment,
    expression: impl AsRef<Expression>,
) -> Result<Expression, LispError> {
    let expression = expression.as_ref();
    let exp = eval(environment, expression)?;
    let exp_d = exp.get();
    if let ExpEnum::Values(v) = &exp_d.data {
        if v.is_empty() {
            Ok(Expression::make_nil())
        } else {
            Ok((&v[0]).into())
        }
    } else {
        drop(exp_d);
        Ok(exp)
    }
}
