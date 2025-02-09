pub trait UsedIn: Sized{
	#[inline(always)]
	fn used_in<T>(self, f: impl FnOnce(Self) -> T) -> T ;
}
impl<T: Sized> UsedIn for T{

	#[inline(always)]
	fn used_in<U>(self, f: impl FnOnce(Self) -> U) -> U  {
		f(self)
	}
}