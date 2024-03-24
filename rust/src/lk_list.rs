//! Wraps lk list_node pointers and provides standard iter::Iterator
//! over mutable references to the items.
//! I think this code is a little more stupid than it needs to be... but
//! for now, it works.

use core::marker::PhantomData;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct list_node {
    pub prev: *mut list_node,
    pub next: *mut list_node,
}

pub struct LkListIterator<'a, T> {
    head: *mut list_node,
    cur: *mut list_node,
    _marker: PhantomData<&'a T>,
}

impl <'a, T> LkListIterator<'a, T> {
    pub fn new(list: *mut list_node) -> Self {
        Self {
            head: list,
            cur: (&unsafe { *list }.next).cast(),
            _marker: PhantomData,
        }
    }
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
