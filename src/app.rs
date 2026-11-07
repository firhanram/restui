use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    collection_import::import_collection,
    editor::TextInput,
    http::{self, HttpResponse},
    model::{QueryParam, RequestConfig, Workspace},
    storage::Storage,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Requests,
    Method,
    Url,
    Params,
    Body,
    Response,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Requests => Self::Method,
            Self::Method => Self::Url,
            Self::Url => Self::Params,
            Self::Params => Self::Body,
            Self::Body => Self::Response,
            Self::Response => Self::Requests,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Requests => Self::Response,
            Self::Method => Self::Requests,
            Self::Url => Self::Method,
            Self::Params => Self::Url,
            Self::Body => Self::Params,
            Self::Response => Self::Body,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    EditUrl,
    EditBody,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamField {
    Key,
    Value,
}

#[derive(Debug)]
pub enum Overlay {
    Workspaces,
    NewWorkspace(TextInput),
    Import(TextInput),
    Rename(TextInput),
    Params,
    ParamEditor {
        key: TextInput,
        value: TextInput,
        field: ParamField,
        index: Option<usize>,
    },
}

#[derive(Debug)]
enum AppEvent {
    Response {
        request_id: Uuid,
        result: Result<HttpResponse, String>,
    },
}

#[derive(Debug)]
pub struct App {
    pub workspaces: Vec<Workspace>,
    pub active_workspace: usize,
    pub selected_request: usize,
    pub workspace_selected: usize,
    pub param_selected: usize,
    pub focus: Focus,
    pub mode: Mode,
    pub overlay: Option<Overlay>,
    pub url_input: TextInput,
    pub body_input: TextInput,
    pub response_scroll: (u16, u16),
    pub status_message: String,
    pub should_quit: bool,
    responses: HashMap<Uuid, Result<HttpResponse, String>>,
    sending: Vec<Uuid>,
    storage: Storage,
    sender: mpsc::UnboundedSender<AppEvent>,
    receiver: mpsc::UnboundedReceiver<AppEvent>,
}

impl App {
    pub fn new(storage: Storage) -> Result<Self> {
        let mut loaded = storage.load()?;
        for workspace in &mut loaded.workspaces {
            if workspace.requests.is_empty() {
                workspace.requests.push(RequestConfig::new());
            }
        }
        let active_workspace = loaded.active_workspace.min(loaded.workspaces.len() - 1);
        let current = &loaded.workspaces[active_workspace].requests[0];
        let (url, body) = (current.url.clone(), current.body.clone());
        let (sender, receiver) = mpsc::unbounded_channel();

        Ok(Self {
            workspaces: loaded.workspaces,
            active_workspace,
            selected_request: 0,
            workspace_selected: active_workspace,
            param_selected: 0,
            focus: Focus::Url,
            mode: Mode::Normal,
            overlay: None,
            url_input: TextInput::new(url),
            body_input: TextInput::new(body),
            response_scroll: (0, 0),
            status_message: "Ready".to_string(),
            should_quit: false,
            responses: HashMap::new(),
            sending: Vec::new(),
            storage,
            sender,
            receiver,
        })
    }

    pub fn current_request(&self) -> &RequestConfig {
        &self.workspaces[self.active_workspace].requests[self.selected_request]
    }

    fn current_request_mut(&mut self) -> &mut RequestConfig {
        &mut self.workspaces[self.active_workspace].requests[self.selected_request]
    }

    pub fn current_response(&self) -> Option<&Result<HttpResponse, String>> {
        self.responses.get(&self.current_request().id)
    }

    pub fn is_sending(&self) -> bool {
        self.sending.contains(&self.current_request().id)
    }

    pub fn poll_responses(&mut self) {
        while let Ok(AppEvent::Response { request_id, result }) = self.receiver.try_recv() {
            self.sending.retain(|id| *id != request_id);
            self.responses.insert(request_id, result);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
            self.persist();
            self.should_quit = true;
            return;
        }

        if self.overlay.is_some() {
            self.handle_overlay_key(key);
            return;
        }

        match self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::EditUrl => self.handle_url_key(key),
            Mode::EditBody => self.handle_body_key(key),
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        if let Some(overlay) = &mut self.overlay {
            match overlay {
                Overlay::NewWorkspace(input) | Overlay::Import(input) | Overlay::Rename(input) => {
                    input.insert_str(text);
                }
                Overlay::ParamEditor {
                    key, value, field, ..
                } => match field {
                    ParamField::Key => key.insert_str(text),
                    ParamField::Value => value.insert_str(text),
                },
                Overlay::Workspaces | Overlay::Params => {}
            }
            return;
        }

        match self.mode {
            Mode::EditUrl => self.url_input.insert_str(text),
            Mode::EditBody => self.body_input.insert_str(text),
            Mode::Normal => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('s') => self.send_current(),
                KeyCode::Char('n') => self.new_request(),
                KeyCode::Char('w') => {
                    self.workspace_selected = self.active_workspace;
                    self.overlay = Some(Overlay::Workspaces);
                }
                KeyCode::Char('o') => self.overlay = Some(Overlay::Import(TextInput::default())),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => {
                self.persist();
                self.should_quit = true;
            }
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Up => match self.focus {
                Focus::Requests => self.select_previous_request(),
                Focus::Response => {
                    self.response_scroll.0 = self.response_scroll.0.saturating_sub(1)
                }
                _ => {}
            },
            KeyCode::Down => match self.focus {
                Focus::Requests => self.select_next_request(),
                Focus::Response => {
                    self.response_scroll.0 = self.response_scroll.0.saturating_add(1)
                }
                _ => {}
            },
            KeyCode::Left => match self.focus {
                Focus::Method => self.change_method(false),
                Focus::Response => {
                    self.response_scroll.1 = self.response_scroll.1.saturating_sub(4)
                }
                _ => {}
            },
            KeyCode::Right => match self.focus {
                Focus::Method => self.change_method(true),
                Focus::Response => {
                    self.response_scroll.1 = self.response_scroll.1.saturating_add(4)
                }
                _ => {}
            },
            KeyCode::PageUp if self.focus == Focus::Response => {
                self.response_scroll.0 = self.response_scroll.0.saturating_sub(10);
            }
            KeyCode::PageDown if self.focus == Focus::Response => {
                self.response_scroll.0 = self.response_scroll.0.saturating_add(10);
            }
            KeyCode::Home if self.focus == Focus::Response => self.response_scroll = (0, 0),
            KeyCode::Enter => self.activate_focus(),
            KeyCode::Char('y') => self.copy_response(),
            KeyCode::Char('i') => self.overlay = Some(Overlay::Import(TextInput::default())),
            KeyCode::Char('r') => {
                self.overlay = Some(Overlay::Rename(TextInput::new(
                    self.current_request().name.clone(),
                )));
            }
            _ => {}
        }
    }

    fn handle_url_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.finish_editing();
            self.send_current();
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.finish_editing(),
            KeyCode::Tab => {
                self.finish_editing();
                self.focus = Focus::Params;
            }
            _ => self.url_input.handle_key(key, false),
        }
    }

    fn handle_body_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.finish_editing();
            self.send_current();
            return;
        }
        if key.code == KeyCode::Esc {
            self.finish_editing();
        } else {
            self.body_input.handle_key(key, true);
        }
    }

    fn activate_focus(&mut self) {
        match self.focus {
            Focus::Method => self.change_method(true),
            Focus::Url => self.mode = Mode::EditUrl,
            Focus::Params => {
                self.param_selected = self.param_selected.min(self.current_request().params.len());
                self.overlay = Some(Overlay::Params);
            }
            Focus::Body => self.mode = Mode::EditBody,
            Focus::Requests | Focus::Response => {}
        }
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) {
        let event = key;
        let Some(overlay) = self.overlay.take() else {
            return;
        };
        match overlay {
            Overlay::Workspaces => self.handle_workspace_list_key(key),
            Overlay::NewWorkspace(mut input) => match key.code {
                KeyCode::Esc => self.overlay = Some(Overlay::Workspaces),
                KeyCode::Enter => {
                    let name = input.value().trim().to_string();
                    if name.is_empty() {
                        self.status_message = "Workspace name cannot be empty".to_string();
                        self.overlay = Some(Overlay::NewWorkspace(input));
                    } else {
                        self.sync_input_to_request();
                        self.workspaces.push(Workspace::new(name.clone()));
                        self.active_workspace = self.workspaces.len() - 1;
                        self.selected_request = 0;
                        self.sync_editors_from_request();
                        self.persist();
                        self.status_message = format!("Created workspace {name}");
                    }
                }
                _ => {
                    input.handle_key(key, false);
                    self.overlay = Some(Overlay::NewWorkspace(input));
                }
            },
            Overlay::Import(mut input) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let path = expand_path(input.value().trim());
                    if input.value().trim().is_empty() {
                        self.status_message = "Enter a collection file path".to_string();
                        self.overlay = Some(Overlay::Import(input));
                    } else {
                        match import_collection(&path) {
                            Ok(imported) => {
                                self.sync_input_to_request();
                                let count = imported.requests.len();
                                let workspace = &mut self.workspaces[self.active_workspace];
                                let first_import = workspace.requests.len();
                                workspace.requests.extend(imported.requests);
                                self.selected_request = first_import;
                                self.sync_editors_from_request();
                                self.persist();
                                self.status_message = format!(
                                    "Imported {count} request{} from {}",
                                    if count == 1 { "" } else { "s" },
                                    imported.name
                                );
                            }
                            Err(error) => {
                                self.status_message = format!("{error:#}");
                                self.overlay = Some(Overlay::Import(input));
                            }
                        }
                    }
                }
                _ => {
                    input.handle_key(key, false);
                    self.overlay = Some(Overlay::Import(input));
                }
            },
            Overlay::Rename(mut input) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let name = input.value().trim().to_string();
                    if name.is_empty() {
                        self.status_message = "Request name cannot be empty".to_string();
                        self.overlay = Some(Overlay::Rename(input));
                    } else {
                        self.current_request_mut().name = name;
                        self.persist();
                        self.status_message = "Request renamed".to_string();
                    }
                }
                _ => {
                    input.handle_key(key, false);
                    self.overlay = Some(Overlay::Rename(input));
                }
            },
            Overlay::Params => self.handle_params_key(key),
            Overlay::ParamEditor {
                mut key,
                mut value,
                mut field,
                index,
            } => match event.code {
                KeyCode::Esc => self.overlay = Some(Overlay::Params),
                KeyCode::Tab | KeyCode::BackTab => {
                    field = match field {
                        ParamField::Key => ParamField::Value,
                        ParamField::Value => ParamField::Key,
                    };
                    self.overlay = Some(Overlay::ParamEditor {
                        key,
                        value,
                        field,
                        index,
                    });
                }
                KeyCode::Enter => {
                    let param =
                        QueryParam::new(key.value().trim().to_string(), value.value().to_string());
                    if param.key.is_empty() {
                        self.status_message = "Parameter key cannot be empty".to_string();
                        self.overlay = Some(Overlay::ParamEditor {
                            key,
                            value,
                            field,
                            index,
                        });
                    } else {
                        let params = &mut self.current_request_mut().params;
                        if let Some(index) = index {
                            let enabled = params[index].enabled;
                            params[index] = QueryParam { enabled, ..param };
                        } else {
                            params.push(param);
                            self.param_selected = params.len() - 1;
                        }
                        self.persist();
                        self.overlay = Some(Overlay::Params);
                    }
                }
                _ => {
                    match field {
                        ParamField::Key => key.handle_key(event, false),
                        ParamField::Value => value.handle_key(event, false),
                    }
                    self.overlay = Some(Overlay::ParamEditor {
                        key,
                        value,
                        field,
                        index,
                    });
                }
            },
        }
    }

    fn handle_workspace_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Up => {
                self.workspace_selected = self.workspace_selected.saturating_sub(1);
                self.overlay = Some(Overlay::Workspaces);
            }
            KeyCode::Down => {
                self.workspace_selected =
                    (self.workspace_selected + 1).min(self.workspaces.len() - 1);
                self.overlay = Some(Overlay::Workspaces);
            }
            KeyCode::Char('n') => self.overlay = Some(Overlay::NewWorkspace(TextInput::default())),
            KeyCode::Enter => {
                self.sync_input_to_request();
                self.active_workspace = self.workspace_selected;
                self.selected_request = 0;
                self.sync_editors_from_request();
                self.persist();
                self.status_message = format!(
                    "Switched to workspace {}",
                    self.workspaces[self.active_workspace].name
                );
            }
            _ => self.overlay = Some(Overlay::Workspaces),
        }
    }

    fn handle_params_key(&mut self, key: KeyEvent) {
        let count = self.current_request().params.len();
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Up => {
                self.param_selected = self.param_selected.saturating_sub(1);
                self.overlay = Some(Overlay::Params);
            }
            KeyCode::Down => {
                if count > 0 {
                    self.param_selected = (self.param_selected + 1).min(count - 1);
                }
                self.overlay = Some(Overlay::Params);
            }
            KeyCode::Char('a') => {
                self.overlay = Some(Overlay::ParamEditor {
                    key: TextInput::default(),
                    value: TextInput::default(),
                    field: ParamField::Key,
                    index: None,
                });
            }
            KeyCode::Enter | KeyCode::Char('e') if count > 0 => {
                let param = &self.current_request().params[self.param_selected.min(count - 1)];
                self.overlay = Some(Overlay::ParamEditor {
                    key: TextInput::new(param.key.clone()),
                    value: TextInput::new(param.value.clone()),
                    field: ParamField::Key,
                    index: Some(self.param_selected.min(count - 1)),
                });
            }
            KeyCode::Char('d') if count > 0 => {
                let index = self.param_selected.min(count - 1);
                self.current_request_mut().params.remove(index);
                self.param_selected = index.saturating_sub(1);
                self.persist();
                self.overlay = Some(Overlay::Params);
            }
            KeyCode::Char(' ') if count > 0 => {
                let index = self.param_selected.min(count - 1);
                let enabled = self.current_request().params[index].enabled;
                self.current_request_mut().params[index].enabled = !enabled;
                self.persist();
                self.overlay = Some(Overlay::Params);
            }
            _ => self.overlay = Some(Overlay::Params),
        }
    }

    fn select_previous_request(&mut self) {
        if self.selected_request > 0 {
            self.sync_input_to_request();
            self.selected_request -= 1;
            self.sync_editors_from_request();
        }
    }

    fn select_next_request(&mut self) {
        if self.selected_request + 1 < self.workspaces[self.active_workspace].requests.len() {
            self.sync_input_to_request();
            self.selected_request += 1;
            self.sync_editors_from_request();
        }
    }

    fn change_method(&mut self, forward: bool) {
        let method = self.current_request().method;
        self.current_request_mut().method = if forward {
            method.next()
        } else {
            method.previous()
        };
        self.persist();
    }

    fn new_request(&mut self) {
        self.sync_input_to_request();
        let workspace = &mut self.workspaces[self.active_workspace];
        workspace.requests.push(RequestConfig::new());
        self.selected_request = workspace.requests.len() - 1;
        self.focus = Focus::Url;
        self.sync_editors_from_request();
        self.mode = Mode::EditUrl;
        self.persist();
        self.status_message = "Created a request; enter its URL".to_string();
    }

    fn finish_editing(&mut self) {
        self.sync_input_to_request();
        self.mode = Mode::Normal;
        self.persist();
    }

    fn sync_input_to_request(&mut self) {
        let url = self.url_input.value().to_string();
        let body = self.body_input.value().to_string();
        let request = self.current_request_mut();
        request.url = url;
        request.body = body;
    }

    fn sync_editors_from_request(&mut self) {
        let request = self.current_request();
        let (url, body) = (request.url.clone(), request.body.clone());
        self.url_input.set_value(url);
        self.body_input.set_value(body);
        self.response_scroll = (0, 0);
        self.mode = Mode::Normal;
    }

    fn send_current(&mut self) {
        self.sync_input_to_request();
        if self.current_request().url.trim().is_empty() {
            self.status_message = "URL is required before sending".to_string();
            return;
        }
        let config = self.current_request().clone();
        if self.sending.contains(&config.id) {
            self.status_message = "Request is already in flight".to_string();
            return;
        }
        self.sending.push(config.id);
        self.status_message = format!("Sending {} {}", config.method.as_str(), config.url);
        self.persist();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let result = http::execute(config.clone())
                .await
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(AppEvent::Response {
                request_id: config.id,
                result,
            });
        });
    }

    fn copy_response(&mut self) {
        let Some(Ok(response)) = self.current_response() else {
            self.status_message = "No successful response to copy".to_string();
            return;
        };
        match Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(response.display_body.clone()))
        {
            Ok(()) => {
                self.status_message = if response.is_json {
                    "Copied formatted JSON to the clipboard".to_string()
                } else {
                    "Copied response body to the clipboard".to_string()
                };
            }
            Err(error) => self.status_message = format!("Clipboard error: {error}"),
        }
    }

    fn persist(&mut self) {
        self.sync_input_to_request();
        if let Err(error) = self.storage.save(&self.workspaces, self.active_workspace) {
            self.status_message = format!("Could not save workspaces: {error:#}");
        }
    }
}

fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    let value = if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    };
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use tempfile::{TempDir, tempdir};

    use super::*;

    fn test_app() -> (TempDir, App) {
        let directory = tempdir().unwrap();
        let storage = Storage::new(directory.path().join("workspaces.json"));
        let app = App::new(storage).unwrap();
        (directory, app)
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn press_control(app: &mut App, character: char) {
        app.handle_key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ));
    }

    #[test]
    fn creates_and_persists_a_workspace_through_key_events() {
        let (_directory, mut app) = test_app();
        press_control(&mut app, 'w');
        press(&mut app, KeyCode::Char('n'));
        app.handle_paste("Quality Assurance");
        press(&mut app, KeyCode::Enter);

        assert_eq!(app.workspaces.len(), 2);
        assert_eq!(app.active_workspace, 1);
        assert_eq!(app.workspaces[1].name, "Quality Assurance");
        let loaded = app.storage.load().unwrap();
        assert_eq!(loaded.active_workspace, 1);
        assert_eq!(loaded.workspaces[1].name, "Quality Assurance");
    }

    #[test]
    fn adds_a_query_parameter_through_the_editor() {
        let (_directory, mut app) = test_app();
        app.focus = Focus::Params;
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('a'));
        app.handle_paste("q");
        press(&mut app, KeyCode::Tab);
        app.handle_paste("rust lang");
        press(&mut app, KeyCode::Enter);

        assert_eq!(
            app.current_request().params,
            vec![QueryParam::new("q".into(), "rust lang".into())]
        );
        assert!(matches!(app.overlay, Some(Overlay::Params)));
    }

    #[test]
    fn imports_a_collection_through_the_dialog() {
        let (directory, mut app) = test_app();
        let path = directory.path().join("collection with spaces.json");
        std::fs::write(
            &path,
            r#"{"info":{"name":"Imported"},"item":[{"name":"Health","request":"https://example.test/health"}]}"#,
        )
        .unwrap();

        press_control(&mut app, 'o');
        app.handle_paste(&format!("'{}'", path.to_str().unwrap()));
        press(&mut app, KeyCode::Enter);

        assert_eq!(app.workspaces[0].requests.len(), 2);
        assert_eq!(app.current_request().name, "Health");
        assert_eq!(app.current_request().url, "https://example.test/health");
        assert_eq!(app.storage.load().unwrap().workspaces[0].requests.len(), 2);
    }

    #[test]
    fn i_opens_the_import_dialog() {
        let (_directory, mut app) = test_app();
        press(&mut app, KeyCode::Char('i'));
        assert!(matches!(app.overlay, Some(Overlay::Import(_))));
    }

    #[test]
    fn strips_matching_quotes_from_import_paths() {
        let path = "/Users/firhanramadhan/Downloads/Midtrans Payment API.postman_collection.json";
        assert_eq!(expand_path(&format!("'{path}'")), PathBuf::from(path));
        assert_eq!(expand_path(&format!("\"{path}\"")), PathBuf::from(path));
    }

    #[test]
    fn new_request_opens_url_editing() {
        let (_directory, mut app) = test_app();
        press_control(&mut app, 'n');
        assert_eq!(app.mode, Mode::EditUrl);
        assert_eq!(app.focus, Focus::Url);
        assert_eq!(app.workspaces[0].requests.len(), 2);
    }
}
