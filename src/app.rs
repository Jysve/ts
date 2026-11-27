use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    model::{DependencyError, Priority, Store, Todo},
    storage,
};

#[derive(Clone)]
pub enum Mode {
    Normal,
    Calendar(CalendarState),
    Adding(AddForm),
    Details(DetailsForm),
    Dependencies(DependencyForm),
    Deleting(u64),
}

#[derive(Clone, Copy)]
pub struct CalendarState {
    pub highlighted: NaiveDate,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum AddField {
    Title,
    Priority,
}

#[derive(Clone)]
pub struct AddForm {
    pub title: String,
    pub priority: Priority,
    pub field: AddField,
    pub error: Option<String>,
    pub editing_id: Option<u64>,
}

impl Default for AddForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            priority: Priority::Medium,
            field: AddField::Title,
            error: None,
            editing_id: None,
        }
    }
}

#[derive(Clone)]
pub struct DetailsForm {
    pub todo_id: u64,
    pub details: String,
    pub scroll: u16,
}

#[derive(Clone)]
pub struct DependencyForm {
    pub todo_id: u64,
    pub selected_id: Option<u64>,
    pub error: Option<String>,
}

pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

pub struct App {
    pub store: Store,
    pub selected_date: NaiveDate,
    pub selected_id: Option<u64>,
    pub mode: Mode,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
    data_location: storage::DataLocation,
}

impl App {
    pub fn new(
        store: Store,
        data_location: storage::DataLocation,
        selected_date: NaiveDate,
    ) -> Self {
        let selected_id = store.todos_on(selected_date).first().map(|todo| todo.id);
        Self {
            store,
            selected_date,
            selected_id,
            mode: Mode::Normal,
            status: None,
            should_quit: false,
            data_location,
        }
    }

    pub fn todos(&self) -> Vec<&Todo> {
        self.store.todos_on(self.selected_date)
    }

    pub fn selected_index(&self) -> Option<usize> {
        let todos = self.todos();
        if todos.is_empty() {
            return None;
        }
        Some(
            self.selected_id
                .and_then(|id| todos.iter().position(|todo| todo.id == id))
                .unwrap_or(0),
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return Ok(());
        }

        self.status = None;
        match self.mode.clone() {
            Mode::Normal => self.handle_normal(key),
            Mode::Calendar(calendar) => self.handle_calendar(key, calendar),
            Mode::Adding(form) => self.handle_adding(key, form),
            Mode::Details(form) => self.handle_details(key, form),
            Mode::Dependencies(form) => self.handle_dependencies(key, form),
            Mode::Deleting(id) => self.handle_deleting(key, id),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q' | 'Q') => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('u' | 'U') => self.select_relative(-1),
            KeyCode::Down | KeyCode::Char('e' | 'E') => self.select_relative(1),
            KeyCode::Left | KeyCode::Char('n' | 'N') => self.change_date(-1),
            KeyCode::Right | KeyCode::Char('i' | 'I') => self.change_date(1),
            KeyCode::Char('t' | 'T') => self.go_to_today(),
            KeyCode::Char('c' | 'C') => {
                self.mode = Mode::Calendar(CalendarState {
                    highlighted: self.selected_date,
                });
            }
            KeyCode::Char('a' | 'A') => {
                self.mode = Mode::Adding(AddForm::default());
            }
            KeyCode::Char('r' | 'R') => self.edit_selected(),
            KeyCode::Char('s' | 'S') => self.open_details(),
            KeyCode::Char('d' | 'D') => self.open_dependencies(),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_selected()?,
            KeyCode::Char('1') => self.set_selected_priority(Priority::High)?,
            KeyCode::Char('2') => self.set_selected_priority(Priority::Medium)?,
            KeyCode::Char('3') => self.set_selected_priority(Priority::Normal)?,
            KeyCode::Char('4') => self.set_selected_priority(Priority::Low)?,
            KeyCode::Char('x' | 'X') => {
                if let Some(id) = self.valid_selected_id() {
                    self.mode = Mode::Deleting(id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_calendar(&mut self, key: KeyEvent, mut calendar: CalendarState) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.selected_date = calendar.highlighted;
                self.select_first();
                self.mode = Mode::Normal;
                self.status = Some(StatusMessage {
                    text: format!("Date set to {}.", self.selected_date),
                    is_error: false,
                });
            }
            KeyCode::Left | KeyCode::Char('n' | 'N') => {
                calendar.highlighted = shift_days(calendar.highlighted, -1);
            }
            KeyCode::Right | KeyCode::Char('i' | 'I') => {
                calendar.highlighted = shift_days(calendar.highlighted, 1);
            }
            KeyCode::Up | KeyCode::Char('u' | 'U') => {
                calendar.highlighted = shift_days(calendar.highlighted, -7);
            }
            KeyCode::Down | KeyCode::Char('e' | 'E') => {
                calendar.highlighted = shift_days(calendar.highlighted, 7);
            }
            KeyCode::PageUp => calendar.highlighted = shift_months(calendar.highlighted, -1),
            KeyCode::PageDown => calendar.highlighted = shift_months(calendar.highlighted, 1),
            KeyCode::Char('t' | 'T') => calendar.highlighted = Local::now().date_naive(),
            _ => {}
        }

        if matches!(self.mode, Mode::Calendar(_)) {
            self.mode = Mode::Calendar(calendar);
        }
        Ok(())
    }

    fn handle_adding(&mut self, key: KeyEvent, mut form: AddForm) -> Result<()> {
        if key.code == KeyCode::Esc {
            self.mode = Mode::Normal;
            return Ok(());
        }

        match key.code {
            KeyCode::Enter => {
                let title = form.title.trim().to_owned();
                if title.is_empty() {
                    form.error = Some("A title is required.".to_owned());
                } else {
                    let (id, message) = if let Some(id) = form.editing_id {
                        if !self
                            .store
                            .update(self.selected_date, id, title, form.priority)
                        {
                            form.error = Some("This todo no longer exists.".to_owned());
                            self.mode = Mode::Adding(form);
                            return Ok(());
                        }
                        (id, "Todo updated.")
                    } else {
                        let id = self.store.add(self.selected_date, title, form.priority);
                        (id, "Todo added.")
                    };
                    storage::save_day(&self.data_location, &self.store, self.selected_date)?;
                    self.selected_id = Some(id);
                    self.mode = Mode::Normal;
                    self.status = Some(StatusMessage {
                        text: message.to_owned(),
                        is_error: false,
                    });
                    return Ok(());
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                form.field = match form.field {
                    AddField::Title => AddField::Priority,
                    AddField::Priority => AddField::Title,
                };
                form.error = None;
            }
            KeyCode::Char(character) if form.field == AddField::Title => {
                if !character.is_control() {
                    form.title.push(character);
                    form.error = None;
                }
            }
            KeyCode::Backspace if form.field == AddField::Title => {
                form.title.pop();
                form.error = None;
            }
            KeyCode::Left | KeyCode::Char('n' | 'N') if form.field == AddField::Priority => {
                form.priority = form.priority.offset(-1);
            }
            KeyCode::Right | KeyCode::Char('i' | 'I') if form.field == AddField::Priority => {
                form.priority = form.priority.offset(1);
            }
            KeyCode::Up | KeyCode::Down => {
                form.field = match form.field {
                    AddField::Title => AddField::Priority,
                    AddField::Priority => AddField::Title,
                };
            }
            KeyCode::Char('u' | 'U' | 'e' | 'E') if form.field == AddField::Priority => {
                form.field = AddField::Title;
            }
            _ => {}
        }

        self.mode = Mode::Adding(form);
        Ok(())
    }

    fn handle_details(&mut self, key: KeyEvent, mut form: DetailsForm) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s' | 'S'))
        {
            if self.store.set_details(
                self.selected_date,
                form.todo_id,
                form.details.trim().to_owned(),
            ) {
                self.persist()?;
                self.status = Some(StatusMessage {
                    text: "Details saved.".to_owned(),
                    is_error: false,
                });
            } else {
                self.status = Some(StatusMessage {
                    text: "This todo no longer exists.".to_owned(),
                    is_error: true,
                });
            }
            self.mode = Mode::Normal;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                return Ok(());
            }
            KeyCode::Char(character) if !character.is_control() => {
                form.details.push(character);
            }
            KeyCode::Backspace => {
                form.details.pop();
            }
            KeyCode::Enter => form.details.push('\n'),
            KeyCode::Tab => form.details.push_str("    "),
            KeyCode::Up => form.scroll = form.scroll.saturating_sub(1),
            KeyCode::Down => {
                let max_scroll = form.details.lines().count().saturating_sub(1) as u16;
                form.scroll = form.scroll.saturating_add(1).min(max_scroll);
            }
            KeyCode::PageUp => form.scroll = form.scroll.saturating_sub(10),
            KeyCode::PageDown => {
                let max_scroll = form.details.lines().count().saturating_sub(1) as u16;
                form.scroll = form.scroll.saturating_add(10).min(max_scroll);
            }
            _ => {}
        }

        self.mode = Mode::Details(form);
        Ok(())
    }

    fn handle_dependencies(&mut self, key: KeyEvent, mut form: DependencyForm) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                return Ok(());
            }
            KeyCode::Up | KeyCode::Char('u' | 'U') => {
                self.select_dependency(&mut form, -1);
            }
            KeyCode::Down | KeyCode::Char('e' | 'E') => {
                self.select_dependency(&mut form, 1);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let prerequisite_id = form.selected_id.filter(|selected_id| {
                    self.dependency_candidates(form.todo_id)
                        .into_iter()
                        .any(|(_, todo)| todo.id == *selected_id)
                });
                if let Some(prerequisite_id) = prerequisite_id {
                    match self.store.toggle_prerequisite(
                        self.selected_date,
                        form.todo_id,
                        prerequisite_id,
                    ) {
                        Ok(_) => {
                            self.persist()?;
                            form.error = None;
                        }
                        Err(DependencyError::SameTodo) => {
                            form.error = Some("A todo cannot depend on itself.".to_owned());
                        }
                        Err(DependencyError::NotFound) => {
                            form.error = Some("The selected todo no longer exists.".to_owned());
                        }
                        Err(DependencyError::Cycle) => {
                            form.error = Some(
                                "This prerequisite would create a dependency cycle.".to_owned(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        self.mode = Mode::Dependencies(form);
        Ok(())
    }

    fn handle_deleting(&mut self, key: KeyEvent, id: u64) -> Result<()> {
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                let dependents = self.store.dependents(id);
                if !dependents.is_empty() {
                    let references = self
                        .format_todo_references(dependents.iter().map(|dependent| dependent.id));
                    self.mode = Mode::Normal;
                    self.status = Some(StatusMessage {
                        text: format!("Cannot delete; required by {references}."),
                        is_error: true,
                    });
                    return Ok(());
                }

                let previous_index = self.selected_index().unwrap_or(0);
                if self.store.delete(self.selected_date, id).is_some() {
                    storage::save_day(&self.data_location, &self.store, self.selected_date)?;
                    let todos = self.todos();
                    self.selected_id = if todos.is_empty() {
                        None
                    } else {
                        Some(todos[previous_index.min(todos.len() - 1)].id)
                    };
                    self.status = Some(StatusMessage {
                        text: "Todo deleted.".to_owned(),
                        is_error: false,
                    });
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Esc | KeyCode::Char('n' | 'N') => self.mode = Mode::Normal,
            _ => self.mode = Mode::Deleting(id),
        }
        Ok(())
    }

    fn edit_selected(&mut self) {
        let Some(id) = self.valid_selected_id() else {
            return;
        };
        if let Some(todo) = self.store.todo_on(self.selected_date, id).cloned() {
            self.mode = Mode::Adding(AddForm {
                title: todo.title,
                priority: todo.priority,
                field: AddField::Title,
                error: None,
                editing_id: Some(id),
            });
        }
    }

    fn open_details(&mut self) {
        let Some(id) = self.valid_selected_id() else {
            return;
        };
        if let Some(todo) = self.store.todo_on(self.selected_date, id) {
            self.mode = Mode::Details(DetailsForm {
                todo_id: id,
                details: todo.details.clone(),
                scroll: 0,
            });
        }
    }

    fn open_dependencies(&mut self) {
        let Some(id) = self.valid_selected_id() else {
            return;
        };
        let selected_id = self
            .dependency_candidates(id)
            .first()
            .map(|(_, todo)| todo.id);
        self.mode = Mode::Dependencies(DependencyForm {
            todo_id: id,
            selected_id,
            error: None,
        });
    }

    pub fn dependency_candidates(&self, todo_id: u64) -> Vec<(NaiveDate, &Todo)> {
        self.store
            .all_todos()
            .into_iter()
            .filter(|(_, todo)| todo.id != todo_id)
            .collect()
    }

    fn select_dependency(&mut self, form: &mut DependencyForm, offset: isize) {
        let candidates = self.dependency_candidates(form.todo_id);
        if candidates.is_empty() {
            form.selected_id = None;
            return;
        }
        let current = form
            .selected_id
            .and_then(|id| candidates.iter().position(|(_, todo)| todo.id == id))
            .unwrap_or(0);
        let next = if offset.is_negative() {
            current.saturating_sub(offset.unsigned_abs())
        } else {
            (current + offset.unsigned_abs()).min(candidates.len() - 1)
        };
        form.selected_id = Some(candidates[next].1.id);
        form.error = None;
    }

    fn toggle_selected(&mut self) -> Result<()> {
        let Some(id) = self.valid_selected_id() else {
            return Ok(());
        };
        match self.store.toggle(self.selected_date, id) {
            Ok(Some(completed)) => {
                self.persist()?;
                self.selected_id = Some(id);
                self.status = Some(StatusMessage {
                    text: if completed {
                        "Todo completed.".to_owned()
                    } else {
                        "Todo marked as active.".to_owned()
                    },
                    is_error: false,
                });
            }
            Ok(None) => {}
            Err(blocked_by) => {
                self.status = Some(StatusMessage {
                    text: format!(
                        "Complete or remove prerequisites first: {}.",
                        self.format_todo_references(blocked_by)
                    ),
                    is_error: true,
                });
            }
        }
        Ok(())
    }

    fn format_todo_references(&self, ids: impl IntoIterator<Item = u64>) -> String {
        ids.into_iter()
            .map(|id| match self.store.todo(id) {
                Some(todo) => format!("#{} {}", id, todo.title),
                None => format!("#{id} (missing)"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn set_selected_priority(&mut self, priority: Priority) -> Result<()> {
        let Some(id) = self.valid_selected_id() else {
            return Ok(());
        };
        if self.store.set_priority(self.selected_date, id, priority) {
            self.persist()?;
            self.selected_id = Some(id);
            self.status = Some(StatusMessage {
                text: format!("Priority set to {}.", priority.label()),
                is_error: false,
            });
        }
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        storage::save_day(&self.data_location, &self.store, self.selected_date)
    }

    fn valid_selected_id(&self) -> Option<u64> {
        let todos = self.todos();
        if todos.is_empty() {
            None
        } else {
            Some(
                self.selected_id
                    .filter(|id| todos.iter().any(|todo| todo.id == *id))
                    .unwrap_or(todos[0].id),
            )
        }
    }

    fn select_relative(&mut self, offset: isize) {
        let todos = self.todos();
        if todos.is_empty() {
            self.selected_id = None;
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let next = if offset.is_negative() {
            current.saturating_sub(offset.unsigned_abs())
        } else {
            (current + offset.unsigned_abs()).min(todos.len() - 1)
        };
        self.selected_id = Some(todos[next].id);
    }

    fn select_first(&mut self) {
        self.selected_id = self.todos().first().map(|todo| todo.id);
    }

    fn change_date(&mut self, days: i64) {
        self.selected_date = shift_days(self.selected_date, days);
        self.select_first();
    }

    fn go_to_today(&mut self) {
        self.selected_date = Local::now().date_naive();
        self.select_first();
    }

    pub fn todo_by_id(&self, id: u64) -> Option<&Todo> {
        self.store.todo_on(self.selected_date, id)
    }
}

fn shift_days(date: NaiveDate, days: i64) -> NaiveDate {
    if days < 0 {
        date.checked_sub_signed(Duration::days(days.unsigned_abs() as i64))
            .unwrap_or(date)
    } else {
        date.checked_add_signed(Duration::days(days))
            .unwrap_or(date)
    }
}

fn shift_months(date: NaiveDate, months: i32) -> NaiveDate {
    let month_index = date.year() * 12 + date.month0() as i32 + months;
    let year = month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) as u32 + 1;
    let Some(first_of_target) = NaiveDate::from_ymd_opt(year, month, 1) else {
        return date;
    };
    let next_month = shift_month_index(year, month);
    let Some(first_of_next) = NaiveDate::from_ymd_opt(next_month.0, next_month.1, 1) else {
        return date;
    };
    let days_in_month = (first_of_next - first_of_target).num_days() as u32;
    NaiveDate::from_ymd_opt(year, month, date.day().min(days_in_month)).unwrap_or(date)
}

fn shift_month_index(year: i32, month: u32) -> (i32, u32) {
    let index = year * 12 + month as i32;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}
