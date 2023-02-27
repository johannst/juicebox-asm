mod add;
mod dec;
mod jmp;
mod jz;
mod mov;
mod ret;
mod test;

pub trait Add<T, U> {
    fn add(&mut self, op1: T, op2: U);
}

pub trait Dec<T> {
    fn dec(&mut self, op1: T);
}

pub trait Jmp<T> {
    fn jmp(&mut self, op1: T);
}

pub trait Jz<T> {
    fn jz(&mut self, op1: T);
}

pub trait Mov<T, U> {
    fn mov(&mut self, op1: T, op2: U);
}

pub trait Test<T, U> {
    fn test(&mut self, op1: T, op2: U);
}
