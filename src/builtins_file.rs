use std::collections::HashMap;
use std::env;
use std::hash::BuildHasher;
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

use glob::glob;

use crate::builtins_util::*;
use crate::environment::*;
use crate::eval::*;
use crate::process::*;
use crate::types::*;

fn cd_expand_all_dots(cd: String) -> String {
    let mut all_dots = false;
    if cd.len() > 2 {
        all_dots = true;
        for ch in cd.chars() {
            if ch != '.' {
                all_dots = false;
                break;
            }
        }
    }
    if all_dots {
        let mut new_cd = String::new();
        let paths_up = cd.len() - 2;
        new_cd.push_str("../");
        for _i in 0..paths_up {
            new_cd.push_str("../");
        }
        new_cd
    } else {
        cd
    }
}

fn builtin_cd(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    let home = match env::var("HOME") {
        Ok(val) => val,
        Err(_) => "/".to_string(),
    };
    let old_dir = match env::var("OLDPWD") {
        Ok(val) => val,
        Err(_) => home.to_string(),
    };
    let new_dir = if let Some(arg) = args.next() {
        if args.next().is_none() {
            let arg = eval(environment, arg)?.as_string(environment)?;
            if let Some(h) = expand_tilde(&arg) {
                h
            } else {
                arg
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "cd can not have more then one form",
            ));
        }
    } else {
        home
    };
    let new_dir = if new_dir == "-" { &old_dir } else { &new_dir };
    let new_dir = cd_expand_all_dots(new_dir.to_string());
    let root = Path::new(&new_dir);
    env::set_var("OLDPWD", env::current_dir()?);
    if let Err(e) = env::set_current_dir(&root) {
        eprintln!("Error changing to {}, {}", root.display(), e);
        Ok(Expression::Atom(Atom::Nil))
    } else {
        env::set_var("PWD", env::current_dir()?);
        Ok(Expression::Atom(Atom::True))
    }
}

fn builtin_path_exists(
    environment: &mut Environment,
    args: &[Expression],
) -> io::Result<Expression> {
    let args = list_to_args(environment, args, true)?;
    if args.len() != 1 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "fs-exists? takes one form (a path)",
        ))
    } else if let Expression::Atom(Atom::String(p)) = &args[0] {
        let path = Path::new(&p);
        if path.exists() {
            Ok(Expression::Atom(Atom::True))
        } else {
            Ok(Expression::Atom(Atom::Nil))
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "fs-exists? path must be a string",
        ))
    }
}

fn builtin_is_file(environment: &mut Environment, args: &[Expression]) -> io::Result<Expression> {
    let args = list_to_args(environment, args, true)?;
    if args.len() != 1 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "fs-file? takes one form (a path)",
        ))
    } else if let Expression::Atom(Atom::String(p)) = &args[0] {
        let path = Path::new(&p);
        if path.is_file() {
            Ok(Expression::Atom(Atom::True))
        } else {
            Ok(Expression::Atom(Atom::Nil))
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "fs-file? path must be a string",
        ))
    }
}

fn builtin_is_dir(environment: &mut Environment, args: &[Expression]) -> io::Result<Expression> {
    let args = list_to_args(environment, args, true)?;
    if args.len() != 1 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "fs-dir? takes one form (a path)",
        ))
    } else if let Expression::Atom(Atom::String(p)) = &args[0] {
        let path = Path::new(&p);
        if path.is_dir() {
            Ok(Expression::Atom(Atom::True))
        } else {
            Ok(Expression::Atom(Atom::Nil))
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "fs-dir? path must be a string",
        ))
    }
}

fn pipe_write_file(environment: &Environment, writer: &mut dyn Write) -> io::Result<()> {
    let mut do_write = false;
    match &environment.data_in {
        Some(Expression::Atom(Atom::Nil)) => {}
        Some(Expression::Atom(_atom)) => {
            do_write = true;
        }
        Some(Expression::Process(ProcessState::Running(_pid))) => {
            do_write = true;
        }
        Some(Expression::File(FileState::Stdin)) => {
            do_write = true;
        }
        Some(Expression::File(FileState::Read(_file))) => {
            do_write = true;
        }
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Invalid expression state before file.",
            ));
        }
        None => {}
    }
    if do_write {
        environment
            .data_in
            .as_ref()
            .unwrap()
            .writef(environment, writer)?;
    }
    Ok(())
}

fn builtin_pipe(environment: &mut Environment, args: &[Expression]) -> io::Result<Expression> {
    let args = list_to_args(environment, args, false)?;
    if environment.in_pipe {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "pipe within pipe, not valid",
        ));
    }
    let old_out_status = environment.state.stdout_status.clone();
    environment.in_pipe = true;
    let mut out = Expression::Atom(Atom::Nil);
    environment.state.stdout_status = Some(IOState::Pipe);
    let mut error: Option<io::Result<Expression>> = None;
    let mut i = 1; // Meant 1 here.
    for p in &args {
        if i == args.len() {
            environment.state.stdout_status = old_out_status.clone();
            environment.in_pipe = false; // End of the pipe and want to wait.
        }
        environment.data_in = Some(out.clone());
        let res = eval(environment, p);
        if let Err(err) = res {
            error = Some(Err(err));
            break;
        }
        if let Ok(Expression::Process(ProcessState::Running(pid))) = res {
            if environment.state.pipe_pgid.is_none() {
                environment.state.pipe_pgid = Some(pid);
            }
        }
        if let Ok(Expression::Process(ProcessState::Over(pid, _exit_status))) = res {
            if environment.state.pipe_pgid.is_none() {
                environment.state.pipe_pgid = Some(pid);
            }
        }
        if let Ok(Expression::File(FileState::Stdout)) = &res {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            if let Err(err) = pipe_write_file(environment, &mut handle) {
                error = Some(Err(err));
                break;
            }
        }
        if let Ok(Expression::File(FileState::Stderr)) = &res {
            let stderr = io::stderr();
            let mut handle = stderr.lock();
            if let Err(err) = pipe_write_file(environment, &mut handle) {
                error = Some(Err(err));
                break;
            }
        }
        if let Ok(Expression::File(FileState::Write(f))) = &res {
            if let Err(err) = pipe_write_file(environment, &mut *f.borrow_mut()) {
                error = Some(Err(err));
                break;
            }
        }
        if let Ok(Expression::File(FileState::Read(_))) = &res {
            if i > 1 {
                error = Some(Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Not a valid place for a read file (must be at start of pipe).",
                )));
                break;
            }
        }
        out = if let Ok(out) = res { out } else { out };
        i += 1;
    }
    environment.data_in = None;
    environment.in_pipe = false;
    environment.state.pipe_pgid = None;
    environment.state.stdout_status = old_out_status;
    if let Some(error) = error {
        error
    } else {
        Ok(out)
    }
}

fn builtin_wait(environment: &mut Environment, args: &[Expression]) -> io::Result<Expression> {
    let args = list_to_args(environment, args, true)?;
    if args.len() != 1 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "wait takes one form (a pid to wait on)",
        ))
    } else {
        match args[0] {
            Expression::Process(ProcessState::Running(pid)) => {
                match wait_pid(environment, pid, None) {
                    Some(exit_status) => Ok(Expression::Atom(Atom::Int(i64::from(exit_status)))),
                    None => Ok(Expression::Atom(Atom::Nil)),
                }
            }
            Expression::Process(ProcessState::Over(_pid, exit_status)) => {
                Ok(Expression::Atom(Atom::Int(i64::from(exit_status))))
            }
            Expression::Atom(Atom::Int(pid)) => match wait_pid(environment, pid as u32, None) {
                Some(exit_status) => Ok(Expression::Atom(Atom::Int(i64::from(exit_status)))),
                None => Ok(Expression::Atom(Atom::Nil)),
            },
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "wait error: not a pid",
            )),
        }
    }
}

fn builtin_pid(environment: &mut Environment, args: &[Expression]) -> io::Result<Expression> {
    let args = list_to_args(environment, args, true)?;
    if args.len() != 1 {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "pid takes one form (a process)",
        ))
    } else {
        match args[0] {
            Expression::Process(ProcessState::Running(pid)) => {
                Ok(Expression::Atom(Atom::Int(i64::from(pid))))
            }
            Expression::Process(ProcessState::Over(pid, _exit_status)) => {
                Ok(Expression::Atom(Atom::Int(i64::from(pid))))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "pid error: not a process",
            )),
        }
    }
}

fn builtin_glob(
    environment: &mut Environment,
    args: &mut dyn Iterator<Item = &Expression>,
) -> io::Result<Expression> {
    let mut files = Vec::new();
    for pat in args {
        let pat = match eval(environment, pat)? {
            Expression::Atom(Atom::String(s)) => s,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "globs need to be strings",
                ))
            }
        };
        let pat = match expand_tilde(&pat) {
            Some(p) => p,
            None => pat,
        };
        match glob(&pat) {
            Ok(paths) => {
                for p in paths {
                    match p {
                        Ok(p) => {
                            if let Some(p) = p.to_str() {
                                files.push(Expression::Atom(Atom::String(p.to_string())));
                            }
                        }
                        Err(err) => {
                            let msg = format!("glob error on while iterating {}, {}", pat, err);
                            return Err(io::Error::new(io::ErrorKind::Other, msg));
                        }
                    }
                }
            }
            Err(err) => {
                let msg = format!("glob error on {}, {}", pat, err);
                return Err(io::Error::new(io::ErrorKind::Other, msg));
            }
        }
    }
    Ok(Expression::with_list(files))
}

pub fn add_file_builtins<S: BuildHasher>(data: &mut HashMap<String, Rc<Expression>, S>) {
    data.insert(
        "cd".to_string(),
        Rc::new(Expression::make_function(builtin_cd, "Change directory.")),
    );
    data.insert(
        "fs-exists?".to_string(),
        Rc::new(Expression::Func(builtin_path_exists)),
    );
    data.insert(
        "fs-file?".to_string(),
        Rc::new(Expression::Func(builtin_is_file)),
    );
    data.insert(
        "fs-dir?".to_string(),
        Rc::new(Expression::Func(builtin_is_dir)),
    );
    data.insert("pipe".to_string(), Rc::new(Expression::Func(builtin_pipe)));
    data.insert("wait".to_string(), Rc::new(Expression::Func(builtin_wait)));
    data.insert("pid".to_string(), Rc::new(Expression::Func(builtin_pid)));
    data.insert(
        "glob".to_string(),
        Rc::new(Expression::make_function(
            builtin_glob,
            "Takes a list of globs and return the list of them expanded.",
        )),
    );
}
