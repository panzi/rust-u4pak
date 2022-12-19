// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::iter::Map;

use crate::Error;
use crate::Result;

#[derive(Debug)]
pub struct Filter<'a> {
    nodes: std::collections::HashMap<&'a str, Filter<'a>>,
    included: bool,
    visited: bool,
}

impl<'a> Default for Filter<'a> {
    fn default() -> Self {
        Self {
            nodes: std::collections::HashMap::<&'a str, Filter<'a>>::new(),
            included: false,
            visited: false,
        }
    }
}

impl<'a> Filter<'a> {
    pub fn new() -> Self {
        Self {
            nodes: std::collections::HashMap::<&'a str, Filter<'a>>::new(),
            included: false,
            visited: false,
        }
    }

    pub fn from_paths<I>(paths: I) -> Self
    where
        I: std::iter::Iterator<Item = &'a str>,
    {
        let mut filter = Self {
            nodes: std::collections::HashMap::<&'a str, Filter<'a>>::new(),
            included: false,
            visited: false,
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
    where
        I: std::iter::Iterator<Item = &'a str>,
    {
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
        self.contains_iter(
            path.as_ref()
                .trim_matches('/')
                .split('/')
                .filter(|comp| !comp.is_empty()),
        )
    }

    pub fn contains_iter<'b, I>(&self, mut path: I) -> bool
    where
        I: std::iter::Iterator<Item = &'b str>,
    {
        if self.included {
            true
        } else if let Some(name) = path.next() {
            if let Some(child) = self.nodes.get(name) {
                child.contains_iter(path)
            } else {
                false
            }
        } else {
            false
        }
    }

    #[inline]
    pub fn visit(&mut self, path: impl AsRef<str>) -> bool {
        self.visit_iter(
            path.as_ref()
                .trim_matches('/')
                .split('/')
                .filter(|comp| !comp.is_empty()),
        )
    }

    pub fn visit_iter<'b, I>(&mut self, mut path: I) -> bool
    where
        I: std::iter::Iterator<Item = &'b str>,
    {
        if self.included {
            self.visited = true;
            if let Some(name) = path.next() {
                if let Some(child) = self.nodes.get_mut(name) {
                    child.visit_iter(path);
                }
            }

            true
        } else if let Some(name) = path.next() {
            if let Some(child) = self.nodes.get_mut(name) {
                child.visit_iter(path)
            } else {
                false
            }
        } else {
            false
        }
    }

    #[inline]
    pub fn iter(&'a self) -> FilterIter<'a> {
        FilterIter {
            stack: vec![(self, self.nodes.iter(), 0)],
            buffer: String::new(),
        }
    }

    #[inline]
    pub fn paths(&'a self) -> Map<FilterIter<'_>, impl FnMut((&'a Filter<'a>, String)) -> String> {
        self.iter().map(|(_, path)| path)
    }

    #[inline]
    pub fn visited_paths(
        &'a self,
    ) -> Map<
        std::iter::Filter<FilterIter<'_>, impl FnMut(&(&'a Filter<'a>, String)) -> bool>,
        impl FnMut((&'a Filter<'a>, String)) -> String,
    > {
        self.iter()
            .filter(|&(filter, _)| filter.visited)
            .map(|(_, path)| path)
    }

    #[inline]
    pub fn non_visited_paths(
        &'a self,
    ) -> Map<
        std::iter::Filter<FilterIter<'_>, impl FnMut(&(&'a Filter<'a>, String)) -> bool>,
        impl FnMut((&'a Filter<'a>, String)) -> String,
    > {
        self.iter()
            .filter(|&(filter, _)| !filter.visited)
            .map(|(_, path)| path)
    }

    pub fn assert_all_visited(&self) -> Result<()> {
        let mut iter = self.non_visited_paths();
        if let Some(filename) = iter.next() {
            let mut message = format!("Paths not found in pak:\n* {}", filename);
            for filename in iter {
                message.push_str("\n* ");
                message.push_str(&filename);
            }
            return Err(Error::new(message));
        }
        Ok(())
    }

    //#[inline]
    //pub fn filter<'b, I>(&'a mut self, records: I) -> std::iter::Filter<I, impl FnMut(&&'b Record) -> bool>
    //where I: Iterator<Item=&'b Record> {
    //    records.filter(move |&record| self.visit(record.filename()))
    //}
}

#[derive(Debug)]
pub struct FilterIter<'a> {
    stack: Vec<(
        &'a Filter<'a>,
        std::collections::hash_map::Iter<'a, &'a str, Filter<'a>>,
        usize,
    )>,
    buffer: String,
}

impl<'a> std::iter::Iterator for FilterIter<'a> {
    type Item = (&'a Filter<'a>, String);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&mut (_, ref mut iter, buffer_index)) = self.stack.last_mut() {
            if let Some((&name, child)) = iter.next() {
                let prev_index = self.buffer.len();
                self.buffer.push('/');
                self.buffer.push_str(name);
                self.stack.push((child, child.nodes.iter(), prev_index));
            } else {
                let (child, _, _) = self.stack.pop().unwrap();

                if child.included {
                    let filename = self.buffer.clone();
                    self.buffer.truncate(buffer_index);
                    return Some((child, filename));
                } else {
                    self.buffer.truncate(buffer_index);
                }
            }
        }
        None
    }
}

impl<'a> From<&[&'a str]> for Filter<'a> {
    #[inline]
    fn from(paths: &[&'a str]) -> Self {
        Filter::from_paths(paths.iter().cloned())
    }
}
