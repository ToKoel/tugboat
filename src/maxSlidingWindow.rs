use std::collections::VecDeque;

pub struct MaxSlidingWindow<T> {
    data: VecDeque<T>,
    max_queue: VecDeque<T>,
}

impl<T> MaxSlidingWindow<T>
where
    T: Copy + PartialOrd + PartialEq,
{
    pub fn new() -> Self {
        Self {
            data: VecDeque::new(),
            max_queue: VecDeque::new(),
        }
    }

    pub fn add(&mut self, x: T) {
        self.data.push_back(x);
        while let Some(&back) = self.max_queue.back() {
            if back < x {
                self.max_queue.pop_back();
            } else {
                break;
            }
        }
        self.max_queue.push_back(x);
    }

    pub fn remove(&mut self) {
        if let Some(front) = self.data.pop_front() {
            if Some(&front) == self.max_queue.front() {
                self.max_queue.pop_front();
            }
        }
    }

    pub fn get_max(&self) -> Option<T> {
        self.max_queue.front().copied()
    }
}
