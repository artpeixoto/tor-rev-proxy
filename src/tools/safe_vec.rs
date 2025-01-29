use core::{alloc,};
use std::{alloc::GlobalAlloc, array, collections::TryReserveError, marker::PhantomPinned, mem::{self, MaybeUninit}, ops::{Deref, Index, IndexMut, Range, RangeBounds, RangeFrom, RangeFull}, slice::{from_raw_parts_mut, SliceIndex}};

use tor_rtcompat::test_with_all_runtimes;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SafeVec<T> {  
	inner_value	: Vec<T>,
}

impl<T: Sized> Drop for SafeVec<T>{
	fn drop(&mut self) {
		let inner_value = mem::replace(&mut self.inner_value, Vec::new());
		clean_vec(inner_value);
	}
}

impl<T: Sized> SafeVec<T>{
	pub fn with_capacity(cap: usize) -> Self{
		Self { inner_value: Vec::with_capacity(cap) }
	}

	// Do notice that reallocation of this is quite expensive. 
	pub fn new() -> Self{
		Self {
			inner_value: Vec::new()
		}
	}
	pub fn new_from_vec(vec: Vec<T>) -> Self{
		Self { inner_value: vec }
	}

	pub fn reserve(&mut self, count: usize){
		let desired_capacity = get_next_capacity(self.inner_value.len() + count);
		reserve_capacity(&mut self.inner_value, desired_capacity);
	}

	pub fn push(&mut self, val: T) {
		if self.inner_value.spare_capacity_mut().is_empty(){
			let desired_cap = get_next_capacity(self.inner_value.len() + 1);
			reserve_capacity(&mut self.inner_value, desired_cap);
		}
		self.inner_value.push(val);
	}

}

#[inline]
fn get_next_capacity(min_cap: usize) -> usize {
	(min_cap).checked_next_power_of_two().unwrap_or(usize::MAX)
} 

fn clean_vec<T: Sized>(mut v: Vec<T>) {
	// drop elements
	v.clear();	
	unsafe {
		let used_size = v.capacity() * mem::size_of::<T>();
		let v_start = v.as_mut_ptr() as *mut u8;
		let v_data_slice = core::slice::from_raw_parts_mut(v_start, used_size);
		v_data_slice.fill(0);
	}
}

fn reserve_capacity<T>(vec: &mut Vec<T>, new_cap: usize ){
	if new_cap <= vec.capacity() { return }

	let mut old_vec = mem::replace(vec, Vec::with_capacity(new_cap));
	vec.append(&mut old_vec);
	clean_vec(old_vec);
}



impl<T: Sized + Clone> SafeVec<T>{
	pub fn extend_from_slice(&mut self, vals: &[T]){
		self.reserve(vals.len());
		for val in vals{
			self.push(val.clone());
		}
	}
	pub fn extend_with_elements(&mut self, value: T, count: usize){
		self.reserve(count);
		for _ in 0..count {
			self.push(value.clone());
		}
	}
}
impl<T> AsRef<[T]> for SafeVec<T>{
	fn as_ref(&self) -> &[T] {
		&self.inner_value
	}
}

impl<T> AsMut<[T]> for SafeVec<T>{
	
	fn as_mut(&mut self) -> &mut [T] {
		&mut self.inner_value
	} 
}

impl<T> Index<usize> for SafeVec<T>{
	type Output = T;

	fn index(&self, index: usize) -> &Self::Output {
		&self.inner_value[index]
	}
}

impl<T> Index<Range<usize>> for SafeVec<T>{
	type Output = [T];

	fn index(&self, index: Range<usize>) -> &Self::Output {
		self.inner_value.index(index)
	}
}
impl<T> IndexMut<Range<usize>> for SafeVec<T>{
	fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
		self.inner_value.index_mut(index)
	}
}
impl<T> Index<RangeFrom<usize>> for SafeVec<T>{
	type Output = [T];

	fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
		self.inner_value.index(index)
	}
}
impl<T> IndexMut<RangeFrom<usize>> for SafeVec<T>{
	fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut Self::Output {
		self.inner_value.index_mut(index)
	}
}



#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct SafeString{ inner_value: String }

impl AsRef<str> for SafeString{
	fn as_ref(&self) -> &str {
		&self.inner_value
	}
}


impl AsMut<str> for SafeString{
	fn as_mut(&mut self) -> &mut str {
		todo!()
	}
}

impl Drop for SafeString{
	fn drop(&mut self) {
		todo!()
	}
}

impl SafeString{
	pub fn new()  -> Self{
		Self{inner_value: String::new()}
	}

	pub fn with_capacity(cap: usize) -> Self{
		Self{inner_value: String::with_capacity(cap)}
	}
	pub fn from_string(string: String) -> Self{
		Self{inner_value: string}
	}

	pub fn reserve(&mut self, byte_count: usize){
		let min_cap = get_next_capacity(self.inner_value.len() + byte_count);
		replace_with::replace_with_or_abort(&mut self.inner_value, |string| {
			let mut vec = string.into_bytes();
			reserve_capacity(&mut vec, min_cap);
			String::from_utf8(vec).unwrap()
		});
	}
	pub fn push(&mut self, c: char) {
		self.reserve(c.len_utf8());
		self.inner_value.push(c);
	}
	
	pub fn push_str(&mut self, str: &str){
		self.reserve(str.len());
		self.inner_value.push_str(str);
	}
}