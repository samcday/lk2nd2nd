//! Wraps lk list_node pointers and provides standard iter::Iterator
//! over mutable references to the items.
//! I think this code is a little more stupid than it needs to be... but
//! for now, it works.

use core::marker::PhantomData;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct list_node {
    pub prev: *mut list_node,
    pub next: *mut list_node,
}

pub struct LkList<'a, T> {
    list: &'a mut list_node,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> LkList<'a, T> {
    pub fn new(list: *mut list_node) -> Self {
        Self {
            list: unsafe { &mut *list },
            _marker: PhantomData,
        }
    }
}

impl<'a, T> IntoIterator for LkList<'a, T> {
    type Item = &'a mut T;
    type IntoIter = LkListIterator<'a, &'a mut T>;

    fn into_iter(self) -> Self::IntoIter {
        LkListIterator {
            head: self.list,
            cur: self.list.next,
            _marker: PhantomData,
        }
    }
}

pub struct LkListIterator<'a, T> {
    head: *mut list_node,
    cur: *mut list_node,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for LkListIterator<'a, &'a mut T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        // Iteration is over when the current node is a pointer to the head of the list.
        if self.head == self.cur {
            return None;
        }

        let item = unsafe { &mut *(self.cur as *mut T) };
        self.cur = unsafe { (*self.cur).next };
        Some(item)
    }
}
