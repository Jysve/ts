use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    #[default]
    Medium,
    Low,
}

impl Priority {
    pub const ALL: [Self; 3] = [Self::High, Self::Medium, Self::Low];

    pub fn label(self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Low => 2,
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

    pub fn toggle(&mut self, date: NaiveDate, id: u64) -> Option<bool> {
        let todo = self.todo_mut(date, id)?;
        todo.completed = !todo.completed;
        Some(todo.completed)
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
