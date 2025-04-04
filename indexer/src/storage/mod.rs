use std::collections::HashMap;

pub mod memory;

type UFIndex = u32;
type UFSize = u32;

struct UnionFind {
    parents: Vec<UFIndex>,
    sizes: HashMap<UFIndex, UFSize>,
    next_index: UFIndex,
}

impl UnionFind {
    pub fn new() -> UnionFind {
        UnionFind {
            parents: Vec::new(),
            sizes: HashMap::new(),
            next_index: 0,
        }
    }

    pub fn add(&mut self) -> UFIndex {
        let idx = self.next_index;
        self.parents.push(idx);
        self.sizes.insert(idx, 1);
        self.next_index += 1;
        idx
    }

    // find the representative node for a given index
    pub fn find(&mut self, idx: UFIndex) -> UFIndex {
        let mut curr_idx = idx;
        while curr_idx != self.parents[curr_idx as usize] {
            curr_idx = self.parents[curr_idx as usize];
        }
        let rep = curr_idx;

        curr_idx = idx;
        while curr_idx != self.parents[curr_idx as usize] {
            let curr_pa = self.parents[curr_idx as usize];
            self.parents[curr_idx as usize] = rep;
            curr_idx = curr_pa;
        }
        rep
    }

    pub fn union(&mut self, idx1: UFIndex, idx2: UFIndex) {
        let pa1 = self.find(idx1);
        let pa2 = self.find(idx2);

        let pa1_size = self.sizes.get(&pa1).unwrap();
        let pa2_size = self.sizes.get(&pa2).unwrap();

        if pa1_size > pa2_size {
            self.parents[pa2 as usize] = pa1;
            self.sizes.insert(pa1, pa1_size + pa2_size);
            self.sizes.remove(&pa2);
        } else {
            self.parents[pa1 as usize] = pa2;
            self.sizes.insert(pa2, pa1_size + pa2_size);
            self.sizes.remove(&pa1);
        }
    }

    pub fn component_size(&mut self, idx: UFIndex) -> UFSize {
        let pa = self.find(idx);
        *self.sizes.get(&pa).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::UnionFind;

    #[test]
    fn test_1() {
        let mut uf = UnionFind::new();
        let idx1 = uf.add();
        let idx2 = uf.add();
        let idx3 = uf.add();
        uf.union(idx3, idx2);
        assert_eq!(uf.component_size(idx1), 1);
        assert_eq!(uf.component_size(idx2), 2);
        assert_eq!(uf.component_size(idx3), 2);
        uf.union(idx1, idx3);
        assert_eq!(uf.component_size(idx1), 3);
        assert_eq!(uf.component_size(idx2), 3);
        assert_eq!(uf.component_size(idx3), 3);
        let idx4 = uf.add();
        let idx5 = uf.add();
        let idx6 = uf.add();
        uf.union(idx4, idx5);
        uf.union(idx5, idx6);
        assert_eq!(uf.component_size(idx1), 3);
        assert_eq!(uf.component_size(idx4), 3);
        assert_eq!(uf.component_size(idx5), 3);
        assert_eq!(uf.component_size(idx6), 3);
        uf.union(idx1, idx6);
        assert_eq!(uf.component_size(idx3), 6);
        assert_eq!(uf.component_size(idx4), 6);
    }
}