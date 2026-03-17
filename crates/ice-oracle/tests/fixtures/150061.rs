// https://github.com/rust-lang/rust/issues/150061
// Exponential compile time / memory from nested const-evaluation via macro.
// Used by ice-oracle tests to verify --debug detection of timeout and memory-out.

pub trait ConstTypeId {
    const TYPE_ID: u128;
}

impl ConstTypeId for u8 {
    const TYPE_ID: u128 = 0;
}

impl ConstTypeId for u16 {
    const TYPE_ID: u128 = 1;
}

pub trait Contained<T> {
    const CONTAINED: bool;
}

impl<T, U> Contained<T> for (U,)
where
    T: ConstTypeId,
    U: ConstTypeId,
{
    const CONTAINED: bool = <U as ConstTypeId>::TYPE_ID == <T as ConstTypeId>::TYPE_ID;
}

impl<T, U, V> Contained<T> for (U, V)
where
    T: ConstTypeId,
    U: ConstTypeId,
    V: Contained<T>,
{
    const CONTAINED: bool = {
        let a = <U as ConstTypeId>::TYPE_ID == <T as ConstTypeId>::TYPE_ID;
        let b = <V as Contained<T>>::CONTAINED;
        a | b
    };
}

pub trait Chained<T, const CONTAINED: bool> {
    type Result;
}

impl<T, U> Chained<T, true> for U {
    type Result = U;
}

impl<T, U> Chained<T, false> for U {
    type Result = (T, U);
}

#[macro_export]
macro_rules! unique_type_tuple {
    ($first_type:ty, $($rest:ty),* $(,)?) => {
        <$crate::unique_type_tuple!($($rest),*) as $crate::Chained<$first_type, { <$crate::unique_type_tuple!($($rest),*) as $crate::Contained<$first_type>>::CONTAINED }>>::Result
    };
    ($current_type:ty $(,)?) => {
        ($current_type,)
    };
}

fn main() {
    type Bar = unique_type_tuple!(
        u8, u16, u8, u8, u16, u8, u8, u16, u8, u8, u16, u8, u8, u16, u8, u8, u16, u8, u8,
        u16,
    );
    println!("Bar: {:#?}", std::any::type_name::<Bar>());
}
