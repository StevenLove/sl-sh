use crate::SloshVm;
use compile_state::state::SloshVmTrait;
use slvm::{Interned, VMError, VMResult, Value};
use std::io::{stdout, Write};

fn is_sym(vm: &SloshVm, name: &str, intern: Interned) -> bool {
    if let Some(i) = vm.get_if_interned(name) {
        if intern == i {
            return true;
        }
    }
    false
}

fn quotey(vm: &SloshVm, car: Value, buf: &mut String) -> bool {
    if let Value::Symbol(i) = car {
        if is_sym(vm, "quote", i) {
            buf.push('\'');
            true
        } else if is_sym(vm, "back-quote", i) {
            buf.push('`');
            true
        } else if is_sym(vm, "unquote", i) {
            buf.push(',');
            true
        } else if is_sym(vm, "unquote-splice", i) {
            buf.push_str(",@");
            true
        } else if is_sym(vm, "unquote-splice!", i) {
            buf.push_str(",.");
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn list_out_iter(vm: &SloshVm, res: &mut String, itr: &mut dyn Iterator<Item = Value>) {
    let mut first = true;
    for p in itr {
        if !first {
            res.push(' ');
        } else {
            first = false;
        }
        res.push_str(&display_value(vm, p));
    }
}

fn list_out(vm: &SloshVm, res: &mut String, lst: Value) {
    let mut first = true;
    let mut cdr = lst;
    loop {
        if let Value::Nil = cdr {
            break;
        }
        if !first {
            res.push(' ');
        } else {
            first = false;
        }
        match cdr {
            Value::Pair(_) | Value::List(_, _) => {
                let (car, ncdr) = cdr.get_pair(vm).expect("pair/list not a pair/list");
                res.push_str(&display_value(vm, car));
                cdr = ncdr;
            }
            _ => {
                res.push_str(". ");
                res.push_str(&display_value(vm, cdr));
                break;
            }
        }
    }
}

pub fn display_value(vm: &SloshVm, val: Value) -> String {
    match &val {
        Value::True => "true".to_string(),
        Value::False => "false".to_string(),
        Value::Int32(i) => format!("{i}"),
        Value::UInt32(i) => format!("{i}"),
        Value::Float64(handle) => format!("{}", vm.get_float(*handle)),
        Value::Int64(handle) => format!("{}", vm.get_int(*handle)),
        Value::UInt64(handle) => format!("{}", vm.get_uint(*handle)),
        Value::Byte(b) => format!("{b}"),
        Value::Symbol(i) => vm.get_interned(*i).to_string(),
        Value::Keyword(i) => format!(":{}", vm.get_interned(*i)),
        Value::StringConst(i) => format!("\"{}\"", vm.get_interned(*i)),
        Value::CodePoint(ch) => format!("\\{ch}"),
        Value::CharCluster(l, c) => {
            format!("\\{}", String::from_utf8_lossy(&c[0..*l as usize]))
        }
        Value::CharClusterLong(h) => format!("\\{}", vm.get_string(*h)),
        Value::Builtin(_) => "#<Function>".to_string(),
        Value::Nil => "nil".to_string(),
        Value::Undefined => "#<Undefined>".to_string(), //panic!("Tried to get type for undefined!"),
        Value::Lambda(_) => "#<Lambda>".to_string(),
        Value::Closure(_) => "#<Lambda>".to_string(),
        Value::Continuation(_) => "#<Continuation>".to_string(),
        Value::CallFrame(_) => "#<CallFrame>".to_string(),
        Value::Vector(h) => {
            let v = vm.get_vector(*h);
            let mut res = String::new();
            res.push('[');
            list_out_iter(vm, &mut res, &mut v.iter().copied());
            res.push(']');
            res
        }
        Value::PersistentVec(_) => {
            let mut res = String::new();
            res.push_str("#[");
            list_out_iter(vm, &mut res, &mut val.iter(vm));
            res.push(']');
            res
        }
        Value::PersistentMap(_) => {
            // TODO- implement
            "IMPLEMENT".to_string()
        }
        Value::MapNode(_) => {
            // TODO- implement
            "MapNode".to_string()
        }
        Value::VecNode(_) => {
            // TODO- implement
            "VecNode".to_string()
        }
        Value::Map(handle) => {
            let mut res = String::new();
            res.push('{');
            for (key, val) in vm.get_map(*handle).iter() {
                res.push_str(&format!(
                    "{} {}, ",
                    key.display_value(vm),
                    val.display_value(vm)
                ));
            }
            res.push('}');
            res
        }
        Value::Pair(_) | Value::List(_, _) => {
            let (car, cdr) = val.get_pair(vm).expect("pair/list not a pair/list");
            let mut res = String::new();
            if quotey(vm, car, &mut res) {
                if let Some((cadr, Value::Nil)) = cdr.get_pair(vm) {
                    res.push_str(&display_value(vm, cadr));
                } else {
                    res.push_str(&display_value(vm, cdr));
                }
            } else {
                res.push('(');
                list_out(vm, &mut res, val);
                res.push(')');
            }
            res
        }
        Value::String(h) => format!("\"{}\"", vm.get_string(*h)),
        Value::Bytes(_) => "Bytes".to_string(), // XXX TODO
        Value::Value(h) => display_value(vm, vm.get_value(*h)),
    }
}

pub fn pretty_value(vm: &SloshVm, val: Value) -> String {
    match &val {
        Value::StringConst(i) => vm.get_interned(*i).to_string(),
        Value::CodePoint(ch) => format!("{ch}"),
        Value::CharCluster(l, c) => {
            format!("{}", String::from_utf8_lossy(&c[0..*l as usize]))
        }
        Value::CharClusterLong(h) => vm.get_string(*h).to_string(),
        Value::String(h) => vm.get_string(*h).to_string(),
        _ => display_value(vm, val),
    }
}

pub fn pr(vm: &mut SloshVm, registers: &[Value]) -> VMResult<Value> {
    for v in registers {
        print!("{}", pretty_value(vm, *v));
    }
    stdout().flush()?;
    Ok(Value::Nil)
}

pub fn prn(vm: &mut SloshVm, registers: &[Value]) -> VMResult<Value> {
    for v in registers {
        print!("{}", pretty_value(vm, *v));
    }
    println!();
    Ok(Value::Nil)
}

pub fn dasm(vm: &mut SloshVm, registers: &[Value]) -> VMResult<Value> {
    if registers.len() != 1 {
        return Err(VMError::new_compile(
            "dasm: wrong number of args, expected one",
        ));
    }
    match registers[0].unref(vm) {
        Value::Lambda(handle) => {
            let l = vm.get_lambda(handle);
            l.disassemble_chunk(vm, 0)?;
            Ok(Value::Nil)
        }
        Value::Closure(handle) => {
            let (l, _) = vm.get_closure(handle);
            l.disassemble_chunk(vm, 0)?;
            Ok(Value::Nil)
        }
        _ => Err(VMError::new_vm("DASM: Not a callable.")),
    }
}

pub fn add_print_builtins(env: &mut SloshVm) {
    env.set_global_builtin("pr", pr);
    env.set_global_builtin("prn", prn);
    env.set_global_builtin("dasm", dasm);
}
