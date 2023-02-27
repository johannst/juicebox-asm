mod mov;

pub trait Mov<T, U> {
    fn mov(&mut self, op1: T, op2: U);
}
