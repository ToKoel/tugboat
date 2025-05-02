use std::collections::VecDeque;

use smart_default::SmartDefault;

#[derive(SmartDefault)]
pub struct MaxSlidingWindow<T> {
    pub data: VecDeque<(T, T)>,
    max_queue: VecDeque<T>,
    #[default = 60]
    capacity: usize,
}

impl<T> MaxSlidingWindow<T>
where
    T: Copy + PartialOrd + PartialEq,
{

    pub fn add(&mut self, x: (T, T)) {
        self.data.push_back(x);
        while let Some(&back) = self.max_queue.back() {
            if back < x.1 {
                self.max_queue.pop_back();
            } else {
                break;
            }
        }
        self.max_queue.push_back(x.1);
        while self.data.len() > self.capacity {
            self.remove();
        }
    }

    fn remove(&mut self) {
        if let Some(front) = self.data.pop_front() {
            if Some(&front.1) == self.max_queue.front() {
                self.max_queue.pop_front();
            }
        }
    }

    pub fn get_max(&self) -> Option<T> {
        self.max_queue.front().copied()
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.max_queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    impl<T> MaxSlidingWindow<T>
    where
        T: Copy + PartialOrd + PartialEq,
    {
        pub fn new(capacity: usize) -> Self {
            Self {
                data: VecDeque::new(),
                max_queue: VecDeque::new(),
                capacity,
            }
        }
    }

    #[test]
    fn add_pushes_to_queue_and_updates_largest() {
        let mut max_sliding_window = MaxSlidingWindow::new(2);
        assert_eq!(None, max_sliding_window.get_max());
        max_sliding_window.add((1.0, 10.0));
        assert_eq!(10.0, max_sliding_window.get_max().unwrap());
        max_sliding_window.add((2.0, 30.0));
        assert_eq!(30.0, max_sliding_window.get_max().unwrap());
        max_sliding_window.add((3.0, 12.0));
        assert_eq!(30.0, max_sliding_window.get_max().unwrap());
        assert_eq!(2, max_sliding_window.data.len());
        max_sliding_window.add((4.0, 11.0));
        assert_eq!(12.0, max_sliding_window.get_max().unwrap());
    }

    #[test]
    fn clear_removes_data() {
        let mut sliding_window = MaxSlidingWindow::new(1);
        sliding_window.add((1.0, 1.0));
        sliding_window.clear();
        assert!(sliding_window.data.is_empty());
        assert!(sliding_window.max_queue.is_empty());
    }
}
