use std::cell::UnsafeCell;

pub struct VecCell<T> {
    vec: UnsafeCell<Vec<T>>,
}

impl<T: Copy> VecCell<T> {
    pub fn new() -> Self {
        VecCell {
            vec: UnsafeCell::new(Vec::new()),
        }
    }

    #[allow(dead_code)]
    pub fn push(&self, value: T) {
        let vec = unsafe { &mut *self.vec.get() };
        vec.push(value);
    }

    #[allow(dead_code)]
    pub fn get(&self, index: usize) -> T {
        let vec = unsafe { &*self.vec.get() };
        *vec.get(index).unwrap()
    }

    pub fn get_or_extend<I: Fn() -> T>(&self, index: usize, initializer: I) -> T {
        let vec = unsafe { &mut *self.vec.get() };
        if index >= vec.len() {
            let value = initializer();
            vec.resize(index + 1, value);
        }
        *vec.get(index).unwrap()
    }

    pub fn set(&self, index: usize, value: T) {
        let vec = unsafe { &mut *self.vec.get() };
        vec[index] = value;
    }

    pub fn set_or_extend<I: Fn() -> T>(&self, index: usize, value: T, initializer: I) {
        let vec = unsafe { &mut *self.vec.get() };
        if index >= vec.len() {
            let value = initializer();
            vec.resize(index + 1, value);
        }
        vec[index] = value;
    }
}

impl<T: Copy> Default for VecCell<T> {
    fn default() -> Self {
        VecCell::new()
    }
}
