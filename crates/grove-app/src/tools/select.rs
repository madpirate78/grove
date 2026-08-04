use uuid::Uuid;

/// Selection state: which entities are selected.
#[derive(Debug, Default)]
pub struct Selection {
    pub ids: Vec<Uuid>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    pub fn set(&mut self, id: Uuid) {
        self.ids.clear();
        self.ids.push(id);
    }

    pub fn toggle(&mut self, id: Uuid) {
        if let Some(pos) = self.ids.iter().position(|&i| i == id) {
            self.ids.remove(pos);
        } else {
            self.ids.push(id);
        }
    }

    pub fn add(&mut self, id: Uuid) {
        if !self.ids.contains(&id) {
            self.ids.push(id);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn single(&self) -> Option<Uuid> {
        if self.ids.len() == 1 {
            Some(self.ids[0])
        } else {
            None
        }
    }
}
