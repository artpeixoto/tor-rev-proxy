use core::alloc;
use std::{alloc::GlobalAlloc, array, collections::TryReserveError, marker::PhantomPinned, mem::{self, MaybeUninit}, ops::{Deref, Index, RangeBounds}, slice::{from_raw_parts_mut, SliceIndex}};

pub struct SafeVec<T>{  // careful
	inner_value	: Vec<T>,
}

impl<T: Sized> Drop for SafeVec<T>{
	fn drop(&mut self) {
		let inner_value = mem::replace(&mut self.inner_value, Vec::new());
		Self::clean_vec(inner_value);
	}
}

impl<T: Sized> SafeVec<T>{
	pub fn with_capacity(cap: usize) -> Self{
		Self { inner_value: Vec::with_capacity(cap) }
	}

	pub fn new() -> Self{
		Self {
			inner_value: Vec::new()
		}
	}
	pub fn new_from_vec(vec: Vec<T>) -> Self{
		Self { inner_value: vec }
	}
	// this does not reserve exactly.
	pub fn reserve(&mut self, count: usize){
		let desired_capacity = self.inner_value.len() + count;
		self.grow_capacity(desired_capacity);
	}

	pub fn grow_capacity(&mut self, new_cap: usize ){
		if new_cap <= self.inner_value.capacity() { return }

		let new_cap = {
			let mut cap = self.inner_value.capacity();
			while cap < new_cap{
				cap *= 2;
			}
			cap
		};

		let mut new_inner_value = Vec::with_capacity(new_cap);
		new_inner_value.append(&mut self.inner_value);
		new_inner_value = mem::replace(&mut self.inner_value, new_inner_value);
		Self::clean_vec(new_inner_value);
	}

	pub fn push(&mut self, val: T){
		if self.inner_value.spare_capacity_mut().is_empty(){
			self.grow_capacity(self.inner_value.capacity() * 2);
		}
		self.inner_value.push(val);
	}

	pub fn clean_vec(mut v: Vec<T>) {
		v.clear();	
		unsafe {
			let used_size = v.capacity() * mem::size_of::<T>();
			let v_start = v.as_mut_ptr() as *mut u8;
			let v_data_slice = core::slice::from_raw_parts_mut(v_start, used_size);
			v_data_slice.fill(0);
		}
	}
}

impl<T: Sized + Clone> SafeVec<T>{
	pub fn extend_from_slice(&mut self, vals: &[T]){
		self.reserve(vals.len());
		for val in vals{
			self.push(val.clone());
		}
	}
	pub fn extend_with_element(&mut self, value: T, count: usize){
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