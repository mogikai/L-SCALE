use generational_arena::{Arena, Index};

struct Data<T> {
    gen: u64,
    value: Option<T>,
}
pub struct Multiarena<T> {
    index: Rc<RefCell<Arena<Index>>>,
    data: Vec<data<T>>,
}

impl Multiarena<T> {
    pub fn new(index: Rc<RefCell<Arena<Index>>>) -> Self {
        return Multiarena {
            index,
            data: Vec::new(),
        };
    }

    pub fn insert(&mut self, value: T) -> Index {
        let i = self.index.borrow_mut().insert_with(|idx| idx);
        let (location, gen) = i.into_raw_parts();
        if location <= self.data.len() {
            self.data.resize_with(location + 1, || Data{gen: 0, value: None});
        }
        self.data.insert(location, Data{gen, value});
        return i;
    }
    pub fn get(&self, i: Index) -> Option<&T> {
        if ! self.index.contains(i) {
            return None;
        }
        let (i, gen) = i.into_raw_parts();
        if i < self.data.len() {
            return None;
        }
        match self.data.get(i) {
            Some(g, d) => {
                if g == gen {
                    return Some(d);
                }
                return None;
            },
            None => todo!("bug");
        }
    }
}
