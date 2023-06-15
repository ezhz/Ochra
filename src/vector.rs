
use std::ops::*;

// ----------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Vector<T, const N: usize>(pub [T; N]);

// ----------------------------------------------------------------------------------------------------

impl<T, const N: usize> Index<usize> for Vector<T, N>
{
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output
    {
        &self.0[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for Vector<T, N>
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output
    {
        &mut self.0[index]
    }
}

// ----------------------------------------------------------------------------------------------------

macro_rules! impl_operators
{
    () =>
    {        
        impl_operators!{@ += Add::add, AddAssign::add_assign}
        impl_operators!{@ -= Sub::sub, SubAssign::sub_assign}
        impl_operators!{@ *= Mul::mul, MulAssign::mul_assign}
        impl_operators!{@ /= Div::div, DivAssign::div_assign}
    };
    (@ 
        $operator:tt
        $base_trait:tt :: $base_trait_method:ident ,
        $assign_trait:tt :: $assign_trait_method:ident
    ) =>
    {
        impl<T, const N: usize> $base_trait for Vector<T, N>
        where
            T: $base_trait<Output = T> + $assign_trait + Copy
        {
            type Output = Self;
            fn $base_trait_method(mut self, other: Self) -> Self::Output
            {
                for i in 0..N
                {
                    self.0[i] $operator other.0[i]
                }
                self
            }
        }
        impl<T, const N: usize> $base_trait<T> for Vector<T, N>
        where
            T: $base_trait<Output = T> + $assign_trait + Copy
        {
            type Output = Self;
            fn $base_trait_method(mut self, other: T) -> Self::Output
            {
                for i in 0..N
                {
                    self.0[i] $operator other
                }
                self
            }
        }
        impl<T, const N: usize> $assign_trait for Vector<T, N>
        where
            T: $base_trait<Output = T> + $assign_trait + Copy
        {
            fn $assign_trait_method(&mut self, other: Self) -> ()
            {
                for i in 0..N
                {
                    self.0[i] $operator other.0[i]
                }
            }
        }
        impl<T, const N: usize> $assign_trait<T> for Vector<T, N>
        where
            T: $base_trait<Output = T> + $assign_trait + Copy
        {
            fn $assign_trait_method(&mut self, other: T) -> ()
            {
                for i in 0..N
                {
                    self.0[i] $operator other
                }
            }
        }
    }
}

impl_operators!{}
