use std::collections::HashMap;
use std::hash::BuildHasher;
use std::io;
use std::rc::Rc;

use crate::builtins_util::*;
use crate::environment::*;
use crate::eval::*;
use crate::types::*;

fn builtin_type(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return Ok(Expression::Atom(Atom::String(arg.display_type())));
        }
    }
    Err(io::Error::new(io::ErrorKind::Other, "type takes one form"))
}

fn builtin_is_nil(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::Nil) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-nil needs one form",
    ))
}

fn builtin_is_true(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::True) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-true needs one form",
    ))
}

fn builtin_is_float(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::Float(_)) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-float needs one form",
    ))
}

fn builtin_is_int(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::Int(_)) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-int needs one form",
    ))
}

fn builtin_is_symbol(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::Symbol(_)) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-symbol needs one form",
    ))
}

fn builtin_is_string(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::String(_)) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-string needs one form",
    ))
}

fn builtin_is_lambda(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::Lambda(_)) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-lambda needs one form",
    ))
}

fn builtin_is_macro(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Atom(Atom::Macro(_)) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-macro needs one form",
    ))
}

fn builtin_is_vec(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::List(_) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-vec needs one form",
    ))
}

fn builtin_is_pair(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Pair(_, _) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-pair needs one form",
    ))
}

fn builtin_is_builtin(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Func(_) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-builtin needs one form",
    ))
}

fn builtin_is_process(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::Process(_) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-process needs one form",
    ))
}

fn builtin_is_file(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if let Expression::File(_) = arg {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-file needs one form",
    ))
}

fn builtin_is_proper_list(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?;
            return if is_proper_list(&arg) {
                Ok(Expression::Atom(Atom::True))
            } else {
                Ok(Expression::Atom(Atom::Nil))
            };
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "is-proper-list needs one form",
    ))
}

pub fn add_type_builtins<S: BuildHasher>(data: &mut HashMap<String, Rc<Expression>, S>) {
    data.insert(
        "type".to_string(),
        Rc::new(Expression::make_function(builtin_type, "")),
    );
    data.insert(
        "is-nil".to_string(),
        Rc::new(Expression::make_function(builtin_is_nil, "")),
    );
    data.insert(
        "is-true".to_string(),
        Rc::new(Expression::make_function(builtin_is_true, "")),
    );
    data.insert(
        "is-float".to_string(),
        Rc::new(Expression::make_function(builtin_is_float, "")),
    );
    data.insert(
        "is-int".to_string(),
        Rc::new(Expression::make_function(builtin_is_int, "")),
    );
    data.insert(
        "is-symbol".to_string(),
        Rc::new(Expression::make_function(builtin_is_symbol, "")),
    );
    data.insert(
        "is-string".to_string(),
        Rc::new(Expression::make_function(builtin_is_string, "")),
    );
    data.insert(
        "is-lambda".to_string(),
        Rc::new(Expression::make_function(builtin_is_lambda, "")),
    );
    data.insert(
        "is-macro".to_string(),
        Rc::new(Expression::make_function(builtin_is_macro, "")),
    );
    data.insert(
        "is-vec".to_string(),
        Rc::new(Expression::make_function(builtin_is_vec, "")),
    );
    data.insert(
        "is-pair".to_string(),
        Rc::new(Expression::make_function(builtin_is_pair, "")),
    );
    data.insert(
        "is-builtin".to_string(),
        Rc::new(Expression::make_function(builtin_is_builtin, "")),
    );
    data.insert(
        "is-process".to_string(),
        Rc::new(Expression::make_function(builtin_is_process, "")),
    );
    data.insert(
        "is-file".to_string(),
        Rc::new(Expression::make_function(builtin_is_file, "")),
    );
    data.insert(
        "is-proper-list".to_string(),
        Rc::new(Expression::make_function(builtin_is_proper_list, "")),
    );
}