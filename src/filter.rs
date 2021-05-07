// This file is part of rust-u4pak.
//
// rust-u4pak is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-u4pak is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-u4pak.  If not, see <https://www.gnu.org/licenses/>.

#[derive(Debug)]
pub struct Filter<'a> {
    nodes: std::collections::HashMap<&'a str, Filter<'a>>,
    included: bool,
}

impl<'a> Filter<'a> {
    pub fn new() -> Self {
        Self {
            nodes: std::collections::HashMap::<&'a str, Filter<'a>>::new(),
            included: false,
        }
    }

    pub fn from_paths<I>(paths: I) -> Self
    where I: std::iter::Iterator<Item=&'a str> {
        let mut filter = Self {
            nodes: std::collections::HashMap::<&'a str, Filter<'a>>::new(),
            included: false,
        };

        for path in paths {
            filter.insert(path);
        }

        filter
    }

    #[inline]
    pub fn insert(&mut self, path: &'a str) {
        self.insert_iter(path.trim_matches('/').split('/'))
    }

    pub fn insert_iter<I>(&mut self, mut path: I)
    where I: std::iter::Iterator<Item=&'a str> {
        if let Some(name) = path.next() {
            if name.is_empty() {
                self.insert_iter(path);
            } else if let Some(child) = self.nodes.get_mut(name) {
                child.insert_iter(path);
            } else {
                let mut child = Self::new();
                child.insert_iter(path);
                self.nodes.insert(name, child);
            }
        } else {
            self.included = true;
        }
    }

    #[inline]
    pub fn contains(&self, path: impl AsRef<str>) -> bool {
        self.contains_iter(path.as_ref().trim_matches('/').split('/'))
    }

    pub fn contains_iter<'b, I>(&self, mut path: I) -> bool
    where I: std::iter::Iterator<Item=&'b str> {
        if self.included {
            true
        } else if let Some(name) = path.next() {
            if name.is_empty() {
                self.contains_iter(path)
            } else if let Some(child) = self.nodes.get(name) {
                child.contains_iter(path)
            } else {
                false
            }
        } else {
            false
        }
    }
}
