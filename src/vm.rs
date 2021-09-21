use std::rc::Rc;

use crate::chunk::*;
use crate::error::*;
use crate::heap::*;
use crate::interner::*;
use crate::opcodes::*;
use crate::value::*;

macro_rules! decode_u16 {
    ($code:expr, $ip:expr) => {{
        let idx1 = $code[*$ip];
        let idx2 = $code[*$ip + 1];
        *$ip += 2;
        ((idx1 as u16) << 8) | (idx2 as u16)
    }};
}

macro_rules! decode1 {
    ($code:expr, $ip:expr, $wide:expr) => {{
        if $wide {
            decode_u16!($code, $ip)
        } else {
            let oip = *$ip;
            *$ip += 1;
            $code[oip] as u16
        }
    }};
}

macro_rules! decode2 {
    ($code:expr, $ip:expr, $wide:expr) => {{
        if $wide {
            (decode_u16!($code, $ip), decode_u16!($code, $ip))
        } else {
            let oip = *$ip;
            *$ip += 2;
            ($code[oip] as u16, $code[oip + 1] as u16)
        }
    }};
}

macro_rules! decode3 {
    ($code:expr, $ip:expr, $wide:expr) => {{
        if $wide {
            (
                decode_u16!($code, $ip),
                decode_u16!($code, $ip),
                decode_u16!($code, $ip),
            )
        } else {
            let oip = *$ip;
            *$ip += 3;
            (
                $code[oip] as u16,
                $code[oip + 1] as u16,
                $code[oip + 2] as u16,
            )
        }
    }};
}
/*
macro_rules! binary_math_inner {
    ($vm:expr, $registers:expr, $dest:expr, $bin_fn:expr, $op3:expr, $op2:expr, $int:expr) => {{
        let val = match $op3 {
            Value::Byte(i) if $int => Value::Int($bin_fn($op2 as i64, i as i64)),
            Value::Byte(i) => Value::Float($bin_fn($op2 as f64, i as f64)),
            Value::Int(i) if $int => Value::Int($bin_fn($op2 as i64, i as i64)),
            Value::Int(i) => Value::Float($bin_fn($op2 as f64, i as f64)),
            Value::UInt(i) if $int => Value::Int($bin_fn($op2 as i64, i as i64)),
            Value::UInt(i) => Value::Float($bin_fn($op2 as f64, i as f64)),
            Value::Float(i) if $int => Value::Int($bin_fn($op2 as i64, i as i64)),
            Value::Float(i) => Value::Float($bin_fn($op2 as f64, i as f64)),
            _ => panic!("Attempt to do math with a {:?}", $op3),
        };
        $vm.set_register($registers, $dest as usize, val);
    }};
}

macro_rules! binary_math {
    ($vm:expr, $chunk:expr, $ip:expr, $registers:expr, $bin_fn:expr, $wide:expr) => {{
        let (dest, op2, op3) = decode3!($chunk.code, $ip, $wide);
        let op2 = $registers[op2 as usize].unref($vm);
        let op3 = $registers[op3 as usize].unref($vm);
        match op2 {
            Value::Byte(i) => binary_math_inner!($vm, $registers, dest, $bin_fn, op3, i, true),
            Value::Int(i) => binary_math_inner!($vm, $registers, dest, $bin_fn, op3, i, true),
            Value::UInt(i) => binary_math_inner!($vm, $registers, dest, $bin_fn, op3, i, true),
            Value::Float(i) => binary_math_inner!($vm, $registers, dest, $bin_fn, op3, i, false),
            _ => panic!("Attempt to do math with a {:?}", op2),
        }
    }};
}*/
macro_rules! binary_math {
    ($vm:expr, $chunk:expr, $ip:expr, $registers:expr, $bin_fn:expr, $wide:expr) => {{
        let (dest, op2, op3) = decode3!($chunk.code, $ip, $wide);
        let op2 = $registers[op2 as usize].unref($vm);
        let op3 = $registers[op3 as usize].unref($vm);
        let val = if op2.is_int() && op3.is_int() {
            Value::Int($bin_fn(op2.get_int()?, op3.get_int()?))
        } else {
            Value::Float($bin_fn(op2.get_float()?, op3.get_float()?))
        };
        $vm.set_register($registers, dest as usize, val);
    }};
}

macro_rules! div_math {
    ($vm:expr, $chunk:expr, $ip:expr, $registers:expr, $wide:expr) => {{
        let (dest, op2, op3) = decode3!($chunk.code, $ip, $wide);
        let op2 = $registers[op2 as usize].unref($vm);
        let op3 = $registers[op3 as usize].unref($vm);
        let val = if op2.is_int() && op3.is_int() {
            let op3 = op3.get_int()?;
            if op3 == 0 {
                return Err(VMError::new_vm("Divide by zero error."));
            }
            Value::Int(op2.get_int()? / op3)
        } else {
            let op3 = op3.get_float()?;
            if op3 == 0.0 {
                return Err(VMError::new_vm("Divide by zero error."));
            }
            Value::Float(op2.get_float()? / op3)
        };
        $vm.set_register($registers, dest as usize, val);
    }};
}

/*macro_rules! set_register {
    ($registers:expr, $idx:expr, $val:expr) => {{
        $registers[$idx as usize] = $val;
        /*unsafe {
            let r = $registers.get_unchecked_mut($idx as usize);
            *r = $val;
        }*/
    }};
}*/

pub struct CallFrame {
    chunk: Rc<Chunk>,
    ip: usize,
    stack_top: usize,
}

pub struct Vm {
    interner: Interner,
    heap: Heap,
    stack: Vec<Value>,
    call_stack: Vec<CallFrame>,
    globals: Globals,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        let globals = Globals::new();
        let mut stack = Vec::with_capacity(1024);
        stack.resize(1024, Value::Undefined);
        Vm {
            interner: Interner::with_capacity(8192),
            heap: Heap::new(),
            stack,
            call_stack: Vec::new(),
            globals,
        }
    }

    pub fn alloc(&mut self, obj: Object) -> Handle {
        self.heap.alloc(obj, |_heap| Ok(()))
    }

    pub fn get(&self, handle: Handle) -> HandleRef<'_> {
        self.heap.get(handle)
    }

    pub fn get_mut(&mut self, handle: Handle) -> HandleRefMut<'_> {
        self.heap.get_mut(handle)
    }

    pub fn get_global(&self, idx: u32) -> Value {
        self.globals.get(idx)
    }

    pub fn get_stack(&self, idx: usize) -> Value {
        self.stack[idx]
    }

    pub fn intern(&mut self, string: &str) -> Interned {
        self.interner.intern(string)
    }

    pub fn reserve_symbol(&mut self, string: &str) -> Value {
        let sym = self.interner.intern(string);
        Value::Symbol(sym, Some(self.globals.reserve(sym)))
    }

    pub fn def_symbol(&mut self, string: &str, value: Value) -> Value {
        let sym = self.interner.intern(string);
        Value::Symbol(sym, Some(self.globals.def(sym, value)))
    }

    #[inline]
    fn set_register(&mut self, registers: &mut [Value], idx: usize, val: Value) {
        match &registers[idx] {
            Value::Binding(handle) => {
                self.heap.replace(*handle, Object::Value(val));
            }
            Value::Global(idx) => self.globals.set(*idx, val),
            _ => registers[idx] = val,
        }
    }

    #[inline]
    fn mov_register(&mut self, registers: &mut [Value], idx: usize, val: Value) {
        registers[idx] = val;
    }

    fn list(
        &mut self,
        code: &[u8],
        ip: &mut usize,
        registers: &mut [Value],
        wide: bool,
    ) -> VMResult<()> {
        let (dest, start, end) = decode3!(code, ip, wide);
        if end == start {
            self.set_register(registers, dest as usize, Value::Nil);
        } else {
            let mut last_cdr = Value::Nil;
            for i in (start..end).rev() {
                let car = if let Some(op) = registers.get(i as usize) {
                    op.unref(self)
                } else {
                    return Err(VMError::new_vm("List: Not enough elements."));
                };
                let cdr = last_cdr;
                last_cdr = Value::Reference(self.alloc(Object::Pair(car, cdr)));
            }
            self.set_register(registers, dest as usize, last_cdr);
        }
        Ok(())
    }

    fn xar(
        &mut self,
        code: &[u8],
        ip: &mut usize,
        registers: &mut [Value],
        wide: bool,
    ) -> VMResult<()> {
        let (pair_reg, val) = decode2!(code, ip, wide);
        let pair = registers[pair_reg as usize].unref(self);
        let val = registers[val as usize].unref(self);
        match &pair {
            Value::Reference(cons_handle) => {
                let cons_d = self.heap.get(*cons_handle);
                if let Object::Pair(_car, cdr) = &*cons_d {
                    let cdr = *cdr;
                    self.heap.replace(*cons_handle, Object::Pair(val, cdr));
                } else if cons_d.is_nil() {
                    let pair = Object::Pair(val, Value::Nil);
                    self.heap.replace(*cons_handle, pair);
                } else {
                    return Err(VMError::new_vm("XAR: Not a pair/conscell."));
                }
            }
            Value::Nil => {
                let pair = Value::Reference(self.alloc(Object::Pair(val, Value::Nil)));
                self.set_register(registers, pair_reg as usize, pair);
            }
            _ => {
                return Err(VMError::new_vm("XAR: Not a pair/conscell."));
            }
        }
        Ok(())
    }

    fn xdr(
        &mut self,
        code: &[u8],
        ip: &mut usize,
        registers: &mut [Value],
        wide: bool,
    ) -> VMResult<()> {
        let (pair_reg, val) = decode2!(code, ip, wide);
        let pair = registers[pair_reg as usize].unref(self);
        let val = registers[val as usize].unref(self);
        match &pair {
            Value::Reference(cons_handle) => {
                let cons_d = self.heap.get(*cons_handle);
                if let Object::Pair(car, _cdr) = &*cons_d {
                    let car = *car;
                    self.heap.replace(*cons_handle, Object::Pair(car, val));
                } else if cons_d.is_nil() {
                    let pair = Object::Pair(Value::Nil, val);
                    self.heap.replace(*cons_handle, pair);
                } else {
                    return Err(VMError::new_vm("XAR: Not a pair/conscell."));
                }
            }
            Value::Nil => {
                let pair = Value::Reference(self.alloc(Object::Pair(Value::Nil, val)));
                self.set_register(registers, pair_reg as usize, pair);
            }
            _ => {
                return Err(VMError::new_vm("XAR: Not a pair/conscell."));
            }
        }
        Ok(())
    }

    // Need to break the registers lifetime away from self or we can not do much...
    // The underlying stack should never be deleted or reallocated for the life
    // of Vm so this should be safe.
    fn make_registers(&mut self, start: usize) -> &'static mut [Value] {
        unsafe { &mut *(&mut self.stack[start..] as *mut [Value]) }
    }

    pub fn execute(&mut self, chunk: Rc<Chunk>) -> VMResult<()> {
        let mut stack_top = 0;
        let mut registers = self.make_registers(stack_top);
        let mut chunk = chunk;
        let mut ip = 0;
        let mut wide = false;
        loop {
            let opcode = chunk.code[ip];
            ip += 1;
            match opcode {
                NOP => {}
                HALT => {
                    return Err(VMError::new_vm("HALT: VM halted and on fire!"));
                }
                RET => {
                    if let Some(frame) = self.call_stack.pop() {
                        stack_top = frame.stack_top;
                        registers = self.make_registers(stack_top);
                        chunk = frame.chunk.clone();
                        ip = frame.ip;
                    } else {
                        return Ok(());
                    }
                }
                WIDE => wide = true,
                MOV => {
                    let (dest, src) = decode2!(chunk.code, &mut ip, wide);
                    let val = registers[src as usize];
                    self.mov_register(registers, dest as usize, val);
                }
                SET => {
                    let (dest, src) = decode2!(chunk.code, &mut ip, wide);
                    let val = registers[src as usize].unref(self);
                    self.set_register(registers, dest as usize, val);
                }
                CONST => {
                    let (dest, src) = decode2!(chunk.code, &mut ip, wide);
                    let val = chunk.constants[src as usize];
                    self.mov_register(registers, dest as usize, val);
                }
                REF => {
                    let (dest, src) = decode2!(chunk.code, &mut ip, wide);
                    let idx = if let Value::Symbol(s, i) = registers[src as usize].unref(self) {
                        if let Some(i) = i {
                            i
                        } else if let Some(i) = self.globals.interned_slot(s) {
                            i as u32
                        } else {
                            return Err(VMError::new_vm("REF: Symbol not interned."));
                        }
                    } else {
                        return Err(VMError::new_vm("REF: Not a symbol."));
                    };
                    if let Value::Undefined = self.globals.get(idx as u32) {
                        return Err(VMError::new_vm("REF: Symbol is not defined."));
                    }
                    self.mov_register(registers, dest as usize, Value::Global(idx));
                }
                DEF => {
                    let (dest, src) = decode2!(chunk.code, &mut ip, wide);
                    let val = registers[src as usize].unref(self);
                    if let Value::Symbol(s, i) = registers[dest as usize].unref(self) {
                        if let Some(i) = i {
                            self.globals.set(i, val);
                        } else {
                            self.globals.def(s, val);
                        }
                    } else {
                        return Err(VMError::new_vm("DEF: Not a symbol."));
                    }
                }
                DEFV => {
                    let (dest, src) = decode2!(chunk.code, &mut ip, wide);
                    let val = registers[src as usize].unref(self);
                    if let Value::Symbol(s, i) = registers[dest as usize].unref(self) {
                        if let Some(i) = i {
                            if let Value::Undefined = self.globals.get(i) {
                                self.globals.set(i, val);
                            }
                        } else {
                            self.globals.defvar(s, val);
                        }
                    } else {
                        return Err(VMError::new_vm("DEFV: Not a symbol."));
                    }
                }
                CALL => {
                    let (lambda, num_args, first_reg) = decode3!(chunk.code, &mut ip, wide);
                    let lambda = registers[lambda as usize];
                    match lambda.unref(self) {
                        Value::Builtin(f) => {
                            let last_reg = (first_reg + num_args + 1) as usize;
                            let res = f(self, &registers[(first_reg + 1) as usize..last_reg])?;
                            self.mov_register(registers, first_reg as usize, res);
                        }
                        Value::Reference(h) => match self.heap.get(h) {
                            Object::Lambda(l) => {
                                let frame = CallFrame {
                                    chunk: chunk.clone(),
                                    ip,
                                    stack_top,
                                };
                                self.call_stack.push(frame);
                                stack_top = first_reg as usize;
                                chunk = l.clone();
                                ip = 0;
                                registers = self.make_registers(stack_top);
                                self.mov_register(registers, 0, Value::UInt(num_args as u64));
                            }
                            _ => return Err(VMError::new_vm("CALL: Not a callable.")),
                        },
                        _ => return Err(VMError::new_vm("CALL: Not a callable.")),
                    }
                }
                TCALL => {
                    let (lambda, num_args) = decode2!(chunk.code, &mut ip, wide);
                    let lambda = registers[lambda as usize];
                    match lambda.unref(self) {
                        Value::Builtin(f) => {
                            let last_reg = num_args as usize + 1;
                            let res = f(self, &registers[1..last_reg])?;
                            self.mov_register(registers, 0, res);
                        }
                        Value::Reference(h) => match self.heap.get(h) {
                            Object::Lambda(l) => {
                                chunk = l.clone();
                                ip = 0;
                                self.mov_register(registers, 0, Value::UInt(num_args as u64));
                            }
                            _ => return Err(VMError::new_vm("TCALL: Not a callable.")),
                        },
                        _ => return Err(VMError::new_vm("TCALL: Not a callable.")),
                    }
                }
                JMP => {
                    let nip = decode1!(chunk.code, &mut ip, wide);
                    ip = nip as usize;
                }
                JMPF => {
                    let ipoff = decode1!(chunk.code, &mut ip, wide);
                    ip += ipoff as usize;
                }
                JMPB => {
                    let ipoff = decode1!(chunk.code, &mut ip, wide);
                    ip -= ipoff as usize;
                }
                JMPFT => {
                    let (test, ipoff) = decode2!(chunk.code, &mut ip, wide);
                    if registers[test as usize].unref(self).is_truethy() {
                        ip += ipoff as usize;
                    }
                }
                JMPBT => {
                    let (test, ipoff) = decode2!(chunk.code, &mut ip, wide);
                    if registers[test as usize].unref(self).is_truethy() {
                        ip -= ipoff as usize;
                    }
                }
                JMPFF => {
                    let (test, ipoff) = decode2!(chunk.code, &mut ip, wide);
                    if registers[test as usize].unref(self).is_falsey() {
                        ip += ipoff as usize;
                    }
                }
                JMPBF => {
                    let (test, ipoff) = decode2!(chunk.code, &mut ip, wide);
                    if registers[test as usize].unref(self).is_falsey() {
                        ip -= ipoff as usize;
                    }
                }
                JMP_T => {
                    let (test, nip) = decode2!(chunk.code, &mut ip, wide);
                    if registers[test as usize].unref(self).is_truethy() {
                        ip = nip as usize;
                    }
                }
                JMP_F => {
                    let (test, nip) = decode2!(chunk.code, &mut ip, wide);
                    if registers[test as usize].unref(self).is_falsey() {
                        ip = nip as usize;
                    }
                }
                JMPEQ => {
                    let (op1, op2, nip) = decode3!(chunk.code, &mut ip, wide);
                    let op1 = registers[op1 as usize].unref(self).get_int()?;
                    let op2 = registers[op2 as usize].unref(self).get_int()?;
                    if op1 == op2 {
                        ip = nip as usize;
                    }
                }
                JMPLT => {
                    let (op1, op2, nip) = decode3!(chunk.code, &mut ip, wide);
                    let op1 = registers[op1 as usize].unref(self).get_int()?;
                    let op2 = registers[op2 as usize].unref(self).get_int()?;
                    if op1 < op2 {
                        ip = nip as usize;
                    }
                }
                JMPGT => {
                    let (op1, op2, nip) = decode3!(chunk.code, &mut ip, wide);
                    let op1 = registers[op1 as usize].unref(self).get_int()?;
                    let op2 = registers[op2 as usize].unref(self).get_int()?;
                    if op1 > op2 {
                        ip = nip as usize;
                    }
                }
                ADD => binary_math!(self, chunk, &mut ip, registers, |a, b| a + b, wide),
                SUB => binary_math!(self, chunk, &mut ip, registers, |a, b| a - b, wide),
                MUL => binary_math!(self, chunk, &mut ip, registers, |a, b| a * b, wide),
                DIV => div_math!(self, chunk, &mut ip, registers, wide),
                INC => {
                    let (dest, i) = decode2!(chunk.code, &mut ip, wide);
                    let val = match registers[dest as usize].unref(self) {
                        Value::Byte(v) => Value::Byte(v + i as u8),
                        Value::Int(v) => Value::Int(v + i as i64),
                        Value::UInt(v) => Value::UInt(v + i as u64),
                        _ => {
                            return Err(VMError::new_vm(format!(
                                "INC: Can only INC an integer type, got {:?}.",
                                registers[dest as usize].unref(self)
                            )))
                        }
                    };
                    self.set_register(registers, dest as usize, val);
                }
                DEC => {
                    let (dest, i) = decode2!(chunk.code, &mut ip, wide);
                    let val = match registers[dest as usize].unref(self) {
                        Value::Byte(v) => Value::Byte(v - i as u8),
                        Value::Int(v) => Value::Int(v - i as i64),
                        Value::UInt(v) => Value::UInt(v - i as u64),
                        _ => {
                            return Err(VMError::new_vm(format!(
                                "DEC: Can only DEC an integer type, got {:?}.",
                                registers[dest as usize].unref(self)
                            )))
                        }
                    };
                    self.set_register(registers, dest as usize, val);
                }
                CONS => {
                    let (dest, op2, op3) = decode3!(chunk.code, &mut ip, wide);
                    let car = registers[op2 as usize].unref(self);
                    let cdr = registers[op3 as usize].unref(self);
                    let pair = Value::Reference(self.alloc(Object::Pair(car, cdr)));
                    self.set_register(registers, dest as usize, pair);
                }
                CAR => {
                    let (dest, op) = decode2!(chunk.code, &mut ip, wide);
                    let op = registers[op as usize];
                    match op.unref(self) {
                        Value::Reference(handle) => {
                            let handle_d = self.heap.get(handle);
                            if let Object::Pair(car, _) = &*handle_d {
                                let car = *car;
                                self.set_register(registers, dest as usize, car);
                            } else {
                                return Err(VMError::new_vm("CAR: Not a pair/conscell."));
                            }
                        }
                        Value::Nil => self.set_register(registers, dest as usize, Value::Nil),
                        _ => return Err(VMError::new_vm("CAR: Not a pair/conscell.")),
                    }
                }
                CDR => {
                    let (dest, op) = decode2!(chunk.code, &mut ip, wide);
                    let op = registers[op as usize];
                    match op.unref(self) {
                        Value::Reference(handle) => {
                            let handle_d = self.heap.get(handle);
                            if let Object::Pair(_, cdr) = &*handle_d {
                                let cdr = *cdr;
                                self.set_register(registers, dest as usize, cdr);
                            } else {
                                return Err(VMError::new_vm("CDR: Not a pair/conscell."));
                            }
                        }
                        Value::Nil => self.set_register(registers, dest as usize, Value::Nil),
                        _ => return Err(VMError::new_vm("CDR: Not a pair/conscell.")),
                    }
                }
                LIST => self.list(&chunk.code[..], &mut ip, registers, wide)?,
                XAR => self.xar(&chunk.code[..], &mut ip, registers, wide)?,
                XDR => self.xdr(&chunk.code[..], &mut ip, registers, wide)?,
                VECMK => {
                    let (dest, op) = decode2!(chunk.code, &mut ip, wide);
                    let len = registers[op as usize].unref(self).get_int()?;
                    let val = Value::Reference(
                        self.alloc(Object::Vector(Vec::with_capacity(len as usize))),
                    );
                    self.set_register(registers, dest as usize, val);
                }
                VECELS => {
                    let (dest, op) = decode2!(chunk.code, &mut ip, wide);
                    let len = registers[op as usize].unref(self).get_int()?;
                    if let Object::Vector(v) =
                        registers[dest as usize].unref(self).get_object(self)?
                    {
                        v.resize(len as usize, Value::Undefined);
                    }
                }
                VECPSH => {
                    let (dest, op) = decode2!(chunk.code, &mut ip, wide);
                    let val = registers[op as usize].unref(self);
                    if let Object::Vector(v) =
                        registers[dest as usize].unref(self).get_object(self)?
                    {
                        v.push(val);
                    }
                }
                VECPOP => {
                    let (vc, dest) = decode2!(chunk.code, &mut ip, wide);
                    let val = if let Object::Vector(v) =
                        registers[vc as usize].unref(self).get_object(self)?
                    {
                        if let Some(val) = v.pop() {
                            val
                        } else {
                            return Err(VMError::new_vm("VECPOP: Vector is empty."));
                        }
                    } else {
                        return Err(VMError::new_vm("VECPOP: Not a vector."));
                    };
                    self.set_register(registers, dest as usize, val);
                }
                VECNTH => {
                    let (vc, dest, i) = decode3!(chunk.code, &mut ip, wide);
                    let i = registers[i as usize].unref(self).get_int()? as usize;
                    let val = if let Object::Vector(v) =
                        registers[vc as usize].unref(self).get_object(self)?
                    {
                        if let Some(val) = v.get(i) {
                            *val
                        } else {
                            return Err(VMError::new_vm("VECNTH: index out of bounds."));
                        }
                    } else {
                        return Err(VMError::new_vm("VECNTH: Not a vector."));
                    };
                    self.set_register(registers, dest as usize, val);
                }
                VECSTH => {
                    let (vc, src, i) = decode3!(chunk.code, &mut ip, wide);
                    let i = registers[i as usize].unref(self).get_int()? as usize;
                    let val = registers[src as usize].unref(self);
                    if let Object::Vector(v) =
                        registers[vc as usize].unref(self).get_object(self)?
                    {
                        if i >= v.len() {
                            return Err(VMError::new_vm("VECSTH: Index out of range."));
                        }
                        v[i] = val;
                    } else {
                        return Err(VMError::new_vm("VECSTH: Not a vector."));
                    };
                }
                VECMKD => {
                    let (dest, len, dfn) = decode3!(chunk.code, &mut ip, wide);
                    let len = registers[len as usize].unref(self).get_int()?;
                    let dfn = registers[dfn as usize].unref(self);
                    let mut v = Vec::with_capacity(len as usize);
                    for _ in 0..len {
                        v.push(dfn);
                    }
                    let val = Value::Reference(self.alloc(Object::Vector(v)));
                    self.set_register(registers, dest as usize, val);
                }
                _ => {
                    return Err(VMError::new_vm(format!("Invalid opcode {}", opcode)));
                }
            }
            if wide && opcode != WIDE {
                wide = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_int(_vm: &Vm, val: &Value) -> VMResult<i64> {
        if let Value::Int(i) = val {
            Ok(*i)
        } else {
            Err(VMError::new_vm("Not an int"))
        }
    }

    fn is_nil(_vm: &Vm, val: &Value) -> VMResult<bool> {
        if let Value::Nil = val {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    #[test]
    fn test_list() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        chunk.add_constant(Value::Int(1));
        chunk.add_constant(Value::Int(2));
        chunk.add_constant(Value::Int(3));
        chunk.add_constant(Value::Int(4));
        chunk.add_constant(Value::Nil);
        chunk.encode2(CONST, 0, 0, line).unwrap();
        chunk.encode2(CONST, 1, 1, line).unwrap();
        chunk.encode3(CONS, 1, 0, 1, line).unwrap();
        chunk.encode2(CDR, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let const_handle = vm.alloc(Object::Value(Value::Nil));
        chunk.add_constant(Value::Reference(const_handle));
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 2);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CAR, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 1);

        // car with nil
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 4, line).unwrap();
        chunk.encode2(CAR, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].is_nil());

        // car with nil on heap
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 5, line).unwrap();
        chunk.encode2(CAR, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].is_nil());

        // cdr with nil
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CDR, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].is_nil());

        // cdr with nil on heap
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 5, line).unwrap();
        chunk.encode2(CDR, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].is_nil());

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 2, line).unwrap();
        chunk.encode2(XAR, 1, 2, line).unwrap();
        chunk.encode2(CAR, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 3);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 3, line).unwrap();
        chunk.encode2(XDR, 1, 2, line).unwrap();
        chunk.encode2(CDR, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 4);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 4, line).unwrap();
        chunk.encode2(CONST, 3, 2, line).unwrap();
        chunk.encode2(XAR, 2, 3, line).unwrap();
        chunk.encode2(CAR, 0, 2, line).unwrap();
        chunk.encode2(CDR, 3, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 3);
        assert!(vm.stack[3].is_nil());

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 2, 4, line).unwrap();
        chunk.encode2(CONST, 3, 3, line).unwrap();
        chunk.encode2(XDR, 2, 3, line).unwrap();
        chunk.encode2(CDR, 0, 2, line).unwrap();
        chunk.encode2(CAR, 3, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 4);
        assert!(vm.stack[3].is_nil());

        // Test a list with elements.
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 0, 0, line).unwrap();
        chunk.encode2(CONST, 1, 1, line).unwrap();
        chunk.encode2(CONST, 2, 2, line).unwrap();
        chunk.encode3(LIST, 0, 0, 3, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack.get(0).unwrap();
        if let Value::Reference(h) = result {
            if let Object::Pair(car, cdr) = &*vm.heap.get(*h) {
                assert!(get_int(&vm, car)? == 1);
                if let Value::Reference(cdr) = cdr {
                    if let Object::Pair(car, cdr) = &*vm.heap.get(*cdr) {
                        assert!(get_int(&vm, car)? == 2);
                        if let Value::Reference(cdr) = cdr {
                            if let Object::Pair(car, cdr) = &*vm.heap.get(*cdr) {
                                assert!(get_int(&vm, car)? == 3);
                                assert!(is_nil(&vm, cdr)?);
                            } else {
                                assert!(false);
                            }
                        } else {
                            assert!(false);
                        }
                    } else {
                        assert!(false);
                    }
                } else {
                    assert!(false);
                }
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode3(LIST, 0, 0, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack.get(0).unwrap();
        assert!(result.is_nil());

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode3(LIST, 0, 1, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack.get(0).unwrap();
        assert!(result.is_nil());
        Ok(())
    }

    #[test]
    fn test_store() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        for i in 0..u16::MAX {
            chunk.add_constant(Value::Int(i as i64));
        }
        chunk.encode2(CONST, 0, 0, line).unwrap();
        chunk.encode2(CONST, 1, 255, line).unwrap();
        chunk.encode3(ADD, 0, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;

        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 255);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 1, 256, line).unwrap();
        chunk.encode3(ADD, 0, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;

        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 255 + 256);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(MOV, 1, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let result = vm.stack[1].get_int()?;
        assert!(result == 256);
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[1].get_int()?;
        assert!(result == 255 + 256);

        let mut vm = Vm::new();
        let handle = vm.alloc(Object::Value(Value::Int(1)));
        vm.stack[0] = Value::Binding(handle);
        vm.stack[1] = Value::Int(10);
        vm.stack[2] = Value::Int(1);
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(MOV, 1, 0, line).unwrap();
        chunk.encode3(ADD, 1, 1, 2, line).unwrap();
        chunk.encode3(ADD, 1, 1, 2, line).unwrap();
        chunk.encode3(ADD, 1, 1, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].unref(&vm).get_int()?;
        assert!(result == 4);
        let result = vm.stack[1].unref(&vm).get_int()?;
        assert!(result == 4);

        Ok(())
    }

    #[test]
    fn test_global() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let mut vm = Vm::new();
        let sym = vm.intern("test_sym");
        let sym2 = vm.intern("test_symTWO");
        let slot = vm.globals.reserve(sym);
        let const0 = chunk.add_constant(Value::Symbol(sym, Some(slot))) as u16;
        let const1 = chunk.add_constant(Value::Symbol(sym2, None)) as u16;
        let const2 = chunk.add_constant(Value::Int(42)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(REF, 1, 0, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        assert!(vm.execute(chunk.clone()).is_err());

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        vm.globals.set(slot, Value::Int(11));
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(REF, 1, 0, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[1].unref(&vm).get_int()? == 11);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        chunk.encode2(CONST, 0, const1, line)?;
        chunk.encode2(CONST, 1, const2, line)?;
        chunk.encode2(DEF, 0, 1, line)?;
        chunk.encode2(REF, 2, 0, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[2].unref(&vm).get_int()? == 42);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        vm.globals.set(slot, Value::Int(11));
        let slot = vm.globals.interned_slot(sym2).unwrap() as u32;
        let const1 = chunk.add_constant(Value::Symbol(sym2, Some(slot))) as u16;
        let const2 = chunk.add_constant(Value::Int(43)) as u16;
        let const3 = chunk.add_constant(Value::Int(53)) as u16;
        chunk.encode2(CONST, 0, const1, line)?;
        chunk.encode2(CONST, 1, const2, line)?;
        chunk.encode2(CONST, 3, const3, line)?;
        chunk.encode2(DEF, 0, 1, line)?;
        chunk.encode2(DEFV, 0, 3, line)?;
        chunk.encode2(REF, 2, 0, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[2].unref(&vm).get_int()? == 43);

        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        let slot = vm.globals.interned_slot(sym2).unwrap() as u32;
        vm.globals.set(slot, Value::Int(11));
        assert!(vm.globals.get(slot).get_int()? == 11);
        let const1 = chunk.add_constant(Value::Symbol(sym2, Some(slot))) as u16;
        let const2 = chunk.add_constant(Value::Int(43)) as u16;
        let const3 = chunk.add_constant(Value::Int(53)) as u16;
        chunk.encode2(CONST, 0, const1, line)?;
        chunk.encode2(CONST, 1, const2, line)?;
        chunk.encode2(CONST, 3, const3, line)?;
        chunk.encode2(DEF, 0, 1, line)?;
        chunk.encode2(DEFV, 0, 3, line)?;
        chunk.encode2(REF, 2, 0, line)?;
        chunk.encode2(REF, 5, 0, line)?;
        chunk.encode2(SET, 5, 3, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[2].unref(&vm).get_int()? == 53);
        assert!(vm.stack[5].unref(&vm).get_int()? == 53);
        assert!(vm.globals.get(slot).get_int()? == 53);

        let mut vm = Vm::new();
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        let slot = vm.globals.reserve(sym2);
        let const1 = chunk.add_constant(Value::Symbol(sym2, Some(slot))) as u16;
        let const2 = chunk.add_constant(Value::Int(44)) as u16;
        let const3 = chunk.add_constant(Value::Int(53)) as u16;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode2(CONST, 3, const3, line)?;
        chunk.encode2(DEFV, 1, 2, line)?;
        chunk.encode2(DEFV, 1, 3, line)?;
        chunk.encode2(REF, 0, 1, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].unref(&vm).get_int()? == 44);

        let mut vm = Vm::new();
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        let slot = vm.globals.reserve(sym2);
        let const1 = chunk.add_constant(Value::Symbol(sym2, Some(slot))) as u16;
        let const2 = chunk.add_constant(Value::Int(45)) as u16;
        let const3 = chunk.add_constant(Value::Int(55)) as u16;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode2(CONST, 3, const3, line)?;
        chunk.encode2(DEFV, 1, 2, line)?;
        chunk.encode2(DEF, 1, 3, line)?;
        chunk.encode2(REF, 0, 1, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].unref(&vm).get_int()? == 55);

        let mut vm = Vm::new();
        let mut chunk = Rc::try_unwrap(chunk).unwrap();
        chunk.code.clear();
        let slot = vm.globals.reserve(sym2);
        let const1 = chunk.add_constant(Value::Symbol(sym2, Some(slot))) as u16;
        let const2 = chunk.add_constant(Value::Int(45)) as u16;
        let const3 = chunk.add_constant(Value::Int(1)) as u16;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode2(CONST, 3, const3, line)?;
        chunk.encode2(DEFV, 1, 2, line)?;
        chunk.encode2(DEF, 1, 3, line)?;
        chunk.encode2(REF, 0, 1, line)?;
        chunk.encode2(MOV, 5, 0, line)?;
        chunk.encode2(SET, 5, 3, line)?;
        chunk.encode3(ADD, 5, 5, 3, line)?;
        chunk.encode3(ADD, 5, 5, 3, line)?;
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].unref(&vm).get_int()? == 3);
        assert!(vm.globals.get(slot).get_int()? == 3);

        Ok(())
    }

    #[test]
    fn test_pol() -> VMResult<()> {
        // algorithm from http://dan.corlan.net/bench.html
        // Do a lot of loops and simple math.
        /*
        (defn eval-pol (n x)
          (let ((su 0.0) (mu 10.0) (pu 0.0)
                (pol (make-vec 100 0.0)))
            (dotimes-i i n
              (do
                (set! su 0.0)
                (dotimes-i j 100
                   (do
                     (set! mu (/ (+ mu 2.0) 2.0))
                     (vec-set! pol j mu)))
                (dotimes-i j 100
                  (set! su (+ (vec-nth pol j) (* su x))))
                (set! pu (+ pu su))))
            (println pu)))
                 */
        let mut vm = Vm::new();
        let mut chunk = Chunk::new("no_file", 1);
        let n = chunk.add_constant(Value::Int(5000)) as u16;
        let x = chunk.add_constant(Value::Float(0.2)) as u16;
        let su = chunk.add_constant(Value::Float(0.0)) as u16;
        let mu = chunk.add_constant(Value::Float(10.0)) as u16;
        let pu = chunk.add_constant(Value::Float(0.0)) as u16;
        let zero = chunk.add_constant(Value::Int(0)) as u16;
        let zerof = chunk.add_constant(Value::Float(0.0)) as u16;
        let twof = chunk.add_constant(Value::Float(2.0)) as u16;
        let hundred = chunk.add_constant(Value::Int(100)) as u16;
        let one = chunk.add_constant(Value::Int(1)) as u16;
        let line = 1;
        chunk.encode2(CONST, 1, n, line)?;
        chunk.encode2(CONST, 2, x, line)?;
        chunk.encode2(CONST, 3, su, line)?;
        chunk.encode2(CONST, 4, mu, line)?;
        chunk.encode2(CONST, 5, pu, line)?;
        chunk.encode2(CONST, 6, zero, line)?; // i
        chunk.encode2(CONST, 7, zero, line)?; // j
        chunk.encode2(CONST, 8, twof, line)?; // 2.0
        chunk.encode2(CONST, 100, hundred, line)?;
        chunk.encode2(CONST, 101, one, line)?;
        chunk.encode2(CONST, 103, zerof, line)?;
        chunk.encode3(VECMKD, 10, 100, 103, line)?; // pols
                                                    //chunk.encode2(VECELS, 10, 100, line)?;
                                                    // loop i .. n
        chunk.encode2(CONST, 3, zerof, line)?;
        chunk.encode2(CONST, 7, zero, line)?; // j
                                              // loop j .. 100
                                              // (set! mu (/ (+ mu 2.0) 2.0))
        chunk.encode3(ADD, 4, 4, 8, line)?;
        chunk.encode3(DIV, 4, 4, 8, line)?;
        // (vec-set! pol j mu)))
        chunk.encode3(VECSTH, 10, 4, 7, line)?;

        chunk.encode2(INC, 7, 1, line)?;
        chunk.encode3(JMPLT, 7, 100, 0x2b, line)?;

        chunk.encode2(CONST, 7, zero, line)?; // j
                                              // (dotimes-i j 100 (j2)
                                              //   (set! su (+ (vec-nth pol j) (* su x))))
        chunk.encode3(MUL, 50, 3, 2, line)?;
        chunk.encode3(VECNTH, 10, 51, 7, line)?;
        chunk.encode3(ADD, 3, 50, 51, line)?;

        chunk.encode2(INC, 7, 1, line)?;
        chunk.encode3(JMPLT, 7, 100, 0x41, line)?;
        // (set! pu (+ pu su))))
        chunk.encode3(ADD, 5, 5, 3, line)?;

        chunk.encode2(INC, 6, 1, line)?;
        chunk.encode3(JMPLT, 6, 1, 0x25, line)?;

        chunk.encode0(RET, line)?;
        chunk.disassemble_chunk()?;
        //assert!(false);

        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[5].get_float()?;
        assert!(result == 12500.0);

        Ok(())
    }

    #[test]
    fn test_lambda() -> VMResult<()> {
        let mut vm = Vm::new();
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        chunk.encode3(ADD, 0, 1, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let add = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const1 = chunk.add_constant(Value::Int(10)) as u16;
        chunk.encode2(CONST, 2, const1, line).unwrap();
        chunk.encode3(ADD, 0, 1, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let add_ten = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        vm.stack[0] = add;
        vm.stack[1] = add_ten;
        vm.stack[3] = Value::Int(5);
        vm.stack[4] = Value::Int(2);
        vm.stack[6] = Value::Int(2);
        chunk.encode3(CALL, 0, 2, 2, line).unwrap();
        chunk.encode3(CALL, 1, 1, 5, line).unwrap();
        chunk.encode0(RET, line)?;

        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[2].get_int()?;
        assert!(result == 7);
        let result = vm.stack[5].get_int()?;
        assert!(result == 12);
        let result = vm.stack[7].get_int()?;
        assert!(result == 10);

        Ok(())
    }

    #[test]
    fn test_tcall() -> VMResult<()> {
        let mut vm = Vm::new();
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        chunk.encode3(ADD, 0, 1, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let add = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const1 = chunk.add_constant(Value::Int(10)) as u16;
        let const2 = chunk.add_constant(add) as u16;
        chunk.encode2(CONST, 2, const1, line).unwrap();
        chunk.encode2(CONST, 3, const2, line).unwrap();
        chunk.encode2(TCALL, 3, 2, line).unwrap();
        // The TCALL will keep HALT from executing.
        chunk.encode0(HALT, line)?;
        let add_ten = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        vm.stack[1] = Value::Int(5);
        vm.stack[2] = Value::Int(2);
        vm.stack[4] = Value::Int(2);
        vm.stack[50] = add;
        vm.stack[60] = add_ten;
        chunk.encode3(CALL, 60, 1, 3, line).unwrap();
        chunk.encode2(TCALL, 50, 2, line).unwrap();
        // The TCALL will keep HALT from executing.
        chunk.encode0(HALT, line)?;

        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[0].get_int()?;
        assert!(result == 7);
        let result = vm.stack[3].get_int()?;
        assert!(result == 12);

        Ok(())
    }

    #[test]
    fn test_builtin() -> VMResult<()> {
        fn add_b(_vm: &mut Vm, registers: &[Value]) -> VMResult<Value> {
            if registers.len() != 2 {
                return Err(VMError::new_vm("test add: wrong number of args."));
            }
            Ok(Value::Int(
                registers[0].get_int()? + registers[1].get_int()?,
            ))
        }
        fn add_10(_vm: &mut Vm, registers: &[Value]) -> VMResult<Value> {
            if registers.len() != 1 {
                return Err(VMError::new_vm("test add_10: wrong number of args."));
            }
            Ok(Value::Int(registers[0].get_int()? + 10))
        }
        fn make_str(vm: &mut Vm, registers: &[Value]) -> VMResult<Value> {
            if registers.len() != 0 {
                return Err(VMError::new_vm("test make_str: wrong number of args."));
            }
            let s = Value::Reference(vm.alloc(Object::String("builtin hello".into())));
            Ok(s)
        }
        let mut vm = Vm::new();
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const1 = chunk.add_constant(Value::Builtin(add_b)) as u16;
        chunk.encode2(CONST, 10, const1, line).unwrap();
        chunk.encode3(CALL, 10, 2, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let add = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const1 = chunk.add_constant(Value::Builtin(add_b)) as u16;
        chunk.encode2(CONST, 10, const1, line).unwrap();
        chunk.encode2(TCALL, 10, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let tadd = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const1 = chunk.add_constant(Value::Builtin(add_10)) as u16;
        chunk.encode2(CONST, 3, const1, line).unwrap();
        chunk.encode3(CALL, 3, 1, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let add_ten = Value::Reference(vm.alloc(Object::Lambda(Rc::new(chunk))));

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        vm.stack[0] = add;
        vm.stack[1] = add_ten;
        vm.stack[3] = Value::Int(6);
        vm.stack[4] = Value::Int(3);
        vm.stack[6] = Value::Int(12);
        let const1 = chunk.add_constant(Value::Builtin(make_str)) as u16;
        chunk.encode3(CALL, 0, 2, 2, line).unwrap();
        chunk.encode3(CALL, 1, 1, 5, line).unwrap();
        chunk.encode2(CONST, 8, const1, line).unwrap();
        chunk.encode3(CALL, 8, 0, 9, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[2].get_int()?;
        assert!(result == 9);
        let result = vm.stack[5].get_int()?;
        assert!(result == 22);
        match vm.stack[9] {
            Value::Reference(h) => match vm.heap.get(h) {
                Object::String(s) => assert!(s == "builtin hello"),
                _ => panic!("bad make_str call."),
            },
            _ => panic!("bad make_str call"),
        }
        assert!(vm.call_stack.is_empty());

        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        for i in 0..100 {
            vm.stack[i] = Value::Undefined;
        }
        vm.stack[0] = tadd;
        vm.stack[1] = add_ten;
        vm.stack[3] = Value::Int(6);
        vm.stack[4] = Value::Int(3);
        vm.stack[6] = Value::Int(12);
        let const1 = chunk.add_constant(Value::Builtin(make_str)) as u16;
        chunk.encode3(CALL, 0, 2, 2, line).unwrap();
        chunk.encode3(CALL, 1, 1, 5, line).unwrap();
        chunk.encode2(CONST, 8, const1, line).unwrap();
        chunk.encode3(CALL, 8, 0, 9, line).unwrap();
        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let result = vm.stack[2].get_int()?;
        assert!(result == 9);
        let result = vm.stack[5].get_int()?;
        assert!(result == 22);
        match vm.stack[9] {
            Value::Reference(h) => match vm.heap.get(h) {
                Object::String(s) => assert!(s == "builtin hello"),
                _ => panic!("bad make_str call."),
            },
            _ => panic!("bad make_str call"),
        }

        Ok(())
    }

    #[test]
    fn test_jumps() -> VMResult<()> {
        let mut vm = Vm::new();
        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Int(2 as i64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        vm.stack[0] = Value::True;
        vm.stack[1] = Value::False;
        vm.stack[2] = Value::Nil;
        vm.stack[3] = Value::Int(0);
        let line = 1;
        chunk.encode2(CONST, 4, const0, line)?;
        chunk.encode1(JMP, 8, line)?;
        chunk.encode2(CONST, 4, const1, line)?;
        chunk.encode2(CONST, 5, const1, line)?;

        chunk.encode2(CONST, 6, const0, line)?;
        chunk.encode1(JMPF, 3, line)?;
        chunk.encode2(CONST, 6, const1, line)?;
        chunk.encode2(CONST, 7, const1, line)?;

        chunk.encode1(JMPF, 5, line)?;
        chunk.encode2(CONST, 8, const0, line)?;
        chunk.encode1(JMPF, 2, line)?;
        chunk.encode1(JMPB, 7, line)?;
        chunk.encode2(CONST, 9, const1, line)?;

        chunk.encode2(CONST, 10, const0, line)?;
        chunk.encode2(JMPFT, 0, 3, line)?;
        chunk.encode2(CONST, 10, const1, line)?;
        chunk.encode2(CONST, 11, const1, line)?;

        chunk.encode2(CONST, 12, const0, line)?;
        chunk.encode2(JMPFT, 3, 3, line)?;
        chunk.encode2(CONST, 12, const1, line)?;
        chunk.encode2(CONST, 13, const1, line)?;

        chunk.encode2(CONST, 14, const0, line)?;
        chunk.encode2(JMPFF, 1, 3, line)?;
        chunk.encode2(CONST, 14, const1, line)?;
        chunk.encode2(CONST, 15, const1, line)?;

        chunk.encode2(CONST, 16, const0, line)?;
        chunk.encode2(JMPFF, 2, 3, line)?;
        chunk.encode2(CONST, 16, const1, line)?;
        chunk.encode2(CONST, 17, const1, line)?;

        chunk.encode2(CONST, 18, const0, line)?;
        chunk.encode2(JMPFT, 1, 3, line)?;
        chunk.encode2(CONST, 18, const1, line)?;
        chunk.encode2(CONST, 19, const1, line)?;

        chunk.encode2(CONST, 20, const0, line)?;
        chunk.encode2(JMPFT, 2, 3, line)?;
        chunk.encode2(CONST, 20, const1, line)?;
        chunk.encode2(CONST, 21, const1, line)?;

        chunk.encode2(CONST, 22, const0, line)?;
        chunk.encode2(JMPFF, 0, 3, line)?;
        chunk.encode2(CONST, 22, const1, line)?;
        chunk.encode2(CONST, 23, const1, line)?;

        chunk.encode2(CONST, 24, const0, line)?;
        chunk.encode2(JMPFF, 3, 3, line)?;
        chunk.encode2(CONST, 24, const1, line)?;
        chunk.encode2(CONST, 25, const1, line)?;

        chunk.encode0(RET, line)?;
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[4].get_int()? == 2);
        assert!(vm.stack[5].get_int()? == 3);
        assert!(vm.stack[6].get_int()? == 2);
        assert!(vm.stack[7].get_int()? == 3);
        assert!(vm.stack[8].get_int()? == 2);
        assert!(vm.stack[9].get_int()? == 3);
        assert!(vm.stack[10].get_int()? == 2);
        assert!(vm.stack[11].get_int()? == 3);
        assert!(vm.stack[12].get_int()? == 2);
        assert!(vm.stack[13].get_int()? == 3);
        assert!(vm.stack[14].get_int()? == 2);
        assert!(vm.stack[15].get_int()? == 3);
        assert!(vm.stack[16].get_int()? == 2);
        assert!(vm.stack[17].get_int()? == 3);
        assert!(vm.stack[18].get_int()? == 3);
        assert!(vm.stack[19].get_int()? == 3);
        assert!(vm.stack[20].get_int()? == 3);
        assert!(vm.stack[21].get_int()? == 3);
        assert!(vm.stack[22].get_int()? == 3);
        assert!(vm.stack[23].get_int()? == 3);
        assert!(vm.stack[24].get_int()? == 3);
        assert!(vm.stack[25].get_int()? == 3);
        Ok(())
    }

    #[test]
    fn test_add() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const0 = chunk.add_constant(Value::Int(2 as i64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(1)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(ADD, 0, 0, 1, line).unwrap();
        chunk.encode3(ADD, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].get_int()? == 6);

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Float(2 as f64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(1)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(ADD, 0, 0, 1, line).unwrap();
        chunk.encode3(ADD, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        assert!(!item.is_int());
        assert!(item.is_number());
        assert!(item.get_float()? == 6.0);

        let mut chunk = Chunk::new("no_file", 1);
        for i in 0..u16::MAX {
            chunk.add_constant(Value::Int(i as i64));
        }
        chunk.encode2(CONST, 1, 1, line)?;
        chunk.encode2(CONST, 2, 2, line)?;
        chunk.encode2(CONST, 5, 5, line)?;
        chunk.encode2(CONST, 500, 500, line)?;
        chunk.encode3(ADD, 0, 1, 2, line).unwrap();
        chunk.encode3(ADD, 0, 5, 0, line).unwrap();
        chunk.encode3(ADD, 1, 500, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        let item2 = vm.stack[1];
        assert!(item.is_int());
        assert!(item.get_int()? == 8);
        assert!(item2.get_int()? == 508);
        Ok(())
    }

    #[test]
    fn test_sub() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const0 = chunk.add_constant(Value::Int(2 as i64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(1)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(SUB, 0, 0, 1, line).unwrap();
        chunk.encode3(SUB, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].get_int()? == -2);

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Float(5 as f64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(1)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(SUB, 0, 0, 1, line).unwrap();
        chunk.encode3(SUB, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        assert!(!item.is_int());
        assert!(item.is_number());
        assert!(item.get_float()? == 1.0);

        let mut chunk = Chunk::new("no_file", 1);
        for i in 0..u16::MAX {
            chunk.add_constant(Value::Int(i as i64));
        }
        chunk.encode2(CONST, 1, 1, line)?;
        chunk.encode2(CONST, 2, 2, line)?;
        chunk.encode2(CONST, 5, 5, line)?;
        chunk.encode2(CONST, 500, 500, line)?;
        chunk.encode3(SUB, 0, 1, 2, line).unwrap();
        chunk.encode3(SUB, 0, 5, 0, line).unwrap();
        chunk.encode3(SUB, 1, 500, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        let item2 = vm.stack[1];
        assert!(item.is_int());
        assert!(item.get_int()? == 6);
        assert!(item2.get_int()? == 494);
        Ok(())
    }

    #[test]
    fn test_mul() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const0 = chunk.add_constant(Value::Int(2 as i64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(1)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(MUL, 0, 0, 1, line).unwrap();
        chunk.encode3(MUL, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].get_int()? == 6);

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Float(5 as f64)) as u16;
        let const1 = chunk.add_constant(Value::Int(3 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(2)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(MUL, 0, 0, 1, line).unwrap();
        chunk.encode3(MUL, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        assert!(!item.is_int());
        assert!(item.is_number());
        assert!(item.get_float()? == 30.0);

        let mut chunk = Chunk::new("no_file", 1);
        for i in 0..u16::MAX {
            chunk.add_constant(Value::Int(i as i64));
        }
        chunk.encode2(CONST, 1, 1, line)?;
        chunk.encode2(CONST, 2, 2, line)?;
        chunk.encode2(CONST, 5, 5, line)?;
        chunk.encode2(CONST, 500, 500, line)?;
        chunk.encode3(MUL, 0, 1, 2, line).unwrap();
        chunk.encode3(MUL, 0, 5, 0, line).unwrap();
        chunk.encode3(MUL, 1, 500, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        let item2 = vm.stack[1];
        assert!(item.is_int());
        assert!(item.get_int()? == 10);
        assert!(item2.get_int()? == 5000);
        Ok(())
    }

    #[test]
    fn test_div() -> VMResult<()> {
        let mut chunk = Chunk::new("no_file", 1);
        let line = 1;
        let const0 = chunk.add_constant(Value::Int(18 as i64)) as u16;
        let const1 = chunk.add_constant(Value::Int(2 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(3)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(DIV, 0, 0, 1, line).unwrap();
        chunk.encode3(DIV, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        assert!(vm.stack[0].get_int()? == 3);

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Float(10 as f64)) as u16;
        let const1 = chunk.add_constant(Value::Int(2 as i64)) as u16;
        let const2 = chunk.add_constant(Value::Byte(2)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode2(CONST, 2, const2, line)?;
        chunk.encode3(DIV, 0, 0, 1, line).unwrap();
        chunk.encode3(DIV, 0, 0, 2, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        assert!(!item.is_int());
        assert!(item.is_number());
        assert!(item.get_float()? == 2.5);

        let mut chunk = Chunk::new("no_file", 1);
        for i in 0..u16::MAX {
            chunk.add_constant(Value::Int(i as i64));
        }
        chunk.encode2(CONST, 1, 1, line)?;
        chunk.encode2(CONST, 2, 2, line)?;
        chunk.encode2(CONST, 10, 10, line)?;
        chunk.encode2(CONST, 500, 500, line)?;
        chunk.encode3(DIV, 0, 2, 1, line).unwrap();
        chunk.encode3(DIV, 0, 10, 0, line).unwrap();
        chunk.encode3(DIV, 1, 500, 0, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        vm.execute(chunk.clone())?;
        let item = vm.stack[0];
        let item2 = vm.stack[1];
        assert!(item.is_int());
        assert!(item.get_int()? == 5);
        assert!(item2.get_int()? == 100);

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Int(10 as i64)) as u16;
        let const1 = chunk.add_constant(Value::Int(0 as i64)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode3(DIV, 0, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        let res = vm.execute(chunk.clone());
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string() == "[VM]: Divide by zero error.");

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Float(10 as f64)) as u16;
        let const1 = chunk.add_constant(Value::Float(0 as f64)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode3(DIV, 0, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        let res = vm.execute(chunk.clone());
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string() == "[VM]: Divide by zero error.");

        let mut chunk = Chunk::new("no_file", 1);
        let const0 = chunk.add_constant(Value::Float(10 as f64)) as u16;
        let const1 = chunk.add_constant(Value::Byte(0)) as u16;
        chunk.encode2(CONST, 0, const0, line)?;
        chunk.encode2(CONST, 1, const1, line)?;
        chunk.encode3(DIV, 0, 0, 1, line).unwrap();
        chunk.encode0(RET, line)?;
        let mut vm = Vm::new();
        let chunk = Rc::new(chunk);
        let res = vm.execute(chunk.clone());
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string() == "[VM]: Divide by zero error.");
        Ok(())
    }
}
