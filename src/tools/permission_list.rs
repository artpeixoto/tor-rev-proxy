use std::{collections::HashSet, hash::{self, Hash}, ops::Not};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionList<T: Hash + Eq>{
    Block(HashSet<T>),
    Allow(HashSet<T>),
}
impl<T: Hash + Eq> Default for PermissionList<T> {
	fn default() -> Self {
		PermissionList::Block(HashSet::new())
	}
}

impl<T> PermissionList<T> where T: std::hash::Hash + Eq{
	pub fn allows(&self, value: &T) -> bool {
		match self{
			PermissionList::Allow(allowed) => {
				allowed.contains(value)
			},
			PermissionList::Block(blocked) => {
				blocked.contains(value).not()
			},
		}
	}
}


