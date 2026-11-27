use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, FixedOffset, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    #[default]
    Medium,
    Normal,
    Low,
}

impl Priority {
    pub const ALL: [Self; 4] = [Self::High, Self::Medium, Self::Normal, Self::Low];

    pub fn label(self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Normal => "Normal",
            Self::Low => "Low",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Normal => 2,
            Self::Low => 3,
        }
    }

    pub fn offset(self, offset: i32) -> Self {
        let index = self.rank() as i32;
        let next = (index + offset).rem_euclid(Self::ALL.len() as i32);
        Self::ALL[next as usize]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub completed: bool,
    #[serde(default = "created_now")]
    pub created_at: DateTime<FixedOffset>,
    #[serde(default)]
    pub details: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyError {
    SameTodo,
    NotFound,
    Cycle,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Store {
    #[serde(default = "first_id")]
    pub next_id: u64,
    #[serde(default)]
    pub days: BTreeMap<NaiveDate, Vec<Todo>>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            next_id: first_id(),
            days: BTreeMap::new(),
        }
    }
}

impl Store {
    pub fn normalize_next_id(&mut self) {
        let next_from_todos = self
            .days
            .values()
            .flatten()
            .map(|todo| todo.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.next_id = self.next_id.max(next_from_todos).max(1);
    }

    pub fn todos_on(&self, date: NaiveDate) -> Vec<&Todo> {
        let mut todos: Vec<_> = self.days.get(&date).into_iter().flatten().collect();
        todos.sort_by_key(|todo| (todo.priority.rank(), todo.completed, todo.id));
        todos
    }

    pub fn todo_on(&self, date: NaiveDate, id: u64) -> Option<&Todo> {
        self.days.get(&date)?.iter().find(|todo| todo.id == id)
    }

    pub fn todo(&self, id: u64) -> Option<&Todo> {
        self.days.values().flatten().find(|todo| todo.id == id)
    }

    pub fn all_todos(&self) -> Vec<(NaiveDate, &Todo)> {
        self.days
            .keys()
            .flat_map(|date| {
                self.todos_on(*date)
                    .into_iter()
                    .map(move |todo| (*date, todo))
            })
            .collect()
    }

    pub fn add(&mut self, date: NaiveDate, title: String, priority: Priority) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.saturating_add(1);
        self.days.entry(date).or_default().push(Todo {
            id,
            title,
            priority,
            completed: false,
            created_at: created_now(),
            details: String::new(),
            prerequisites: Vec::new(),
        });
        id
    }

    pub fn update(&mut self, date: NaiveDate, id: u64, title: String, priority: Priority) -> bool {
        if let Some(todo) = self.todo_mut(date, id) {
            todo.title = title;
            todo.priority = priority;
            true
        } else {
            false
        }
    }

    pub fn set_details(&mut self, date: NaiveDate, id: u64, details: String) -> bool {
        if let Some(todo) = self.todo_mut(date, id) {
            todo.details = details;
            true
        } else {
            false
        }
    }

    pub fn toggle(&mut self, date: NaiveDate, id: u64) -> Result<Option<bool>, Vec<u64>> {
        let Some(todo) = self.todo_on(date, id) else {
            return Ok(None);
        };
        if !todo.completed {
            let blocked_by = self.blocked_by(id);
            if !blocked_by.is_empty() {
                return Err(blocked_by);
            }
        }

        let todo = self.todo_mut(date, id).expect("the todo was found above");
        todo.completed = !todo.completed;
        Ok(Some(todo.completed))
    }

    pub fn blocked_by(&self, id: u64) -> Vec<u64> {
        self.todo(id)
            .into_iter()
            .flat_map(|todo| &todo.prerequisites)
            .copied()
            .filter(|prerequisite_id| {
                self.todo(*prerequisite_id)
                    .is_none_or(|prerequisite| !prerequisite.completed)
            })
            .collect()
    }

    pub fn dependents(&self, id: u64) -> Vec<&Todo> {
        self.days
            .values()
            .flatten()
            .filter(|todo| todo.prerequisites.contains(&id))
            .collect()
    }

    pub fn toggle_prerequisite(
        &mut self,
        date: NaiveDate,
        id: u64,
        prerequisite_id: u64,
    ) -> Result<bool, DependencyError> {
        if id == prerequisite_id {
            return Err(DependencyError::SameTodo);
        }
        if self.todo_on(date, id).is_none() || self.todo(prerequisite_id).is_none() {
            return Err(DependencyError::NotFound);
        }

        if self
            .todo_on(date, id)
            .expect("the todo was found above")
            .prerequisites
            .contains(&prerequisite_id)
        {
            self.todo_mut(date, id)
                .expect("the todo was found above")
                .prerequisites
                .retain(|candidate| *candidate != prerequisite_id);
            return Ok(false);
        }

        if self.creates_cycle(id, prerequisite_id) {
            return Err(DependencyError::Cycle);
        }
        self.todo_mut(date, id)
            .expect("the todo was found above")
            .prerequisites
            .push(prerequisite_id);
        Ok(true)
    }

    pub fn set_priority(&mut self, date: NaiveDate, id: u64, priority: Priority) -> bool {
        if let Some(todo) = self.todo_mut(date, id) {
            todo.priority = priority;
            true
        } else {
            false
        }
    }

    pub fn delete(&mut self, date: NaiveDate, id: u64) -> Option<Todo> {
        let todos = self.days.get_mut(&date)?;
        let index = todos.iter().position(|todo| todo.id == id)?;
        let removed = todos.remove(index);
        if todos.is_empty() {
            self.days.remove(&date);
        }
        Some(removed)
    }

    fn creates_cycle(&self, id: u64, prerequisite_id: u64) -> bool {
        let mut pending = vec![prerequisite_id];
        let mut visited = HashSet::new();
        while let Some(current_id) = pending.pop() {
            if current_id == id {
                return true;
            }
            if !visited.insert(current_id) {
                continue;
            }
            if let Some(todo) = self.todo(current_id) {
                pending.extend(todo.prerequisites.iter().copied());
            }
        }
        false
    }

    fn todo_mut(&mut self, date: NaiveDate, id: u64) -> Option<&mut Todo> {
        self.days
            .get_mut(&date)?
            .iter_mut()
            .find(|todo| todo.id == id)
    }
}

fn first_id() -> u64 {
    1
}

fn created_now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}
