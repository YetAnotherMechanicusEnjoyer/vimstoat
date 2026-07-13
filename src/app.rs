use keyring::KeyringEntry;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub enum AppState {
    InputToken,
    LoggedIn,
    Error(String),
}

pub struct App {
    pub state: AppState,
    pub input_text: String,
    pub token_entry: KeyringEntry,
    pub should_quit: bool,
}

impl App {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let crate_id = "vimstoat";
        let token_entry = KeyringEntry::try_new(crate_id)?;

        let state = if token_entry.get_secret().await.is_err() {
            AppState::InputToken
        } else {
            AppState::LoggedIn
        };

        Ok(Self {
            state,
            input_text: String::new(),
            token_entry,
            should_quit: false,
        })
    }

    pub async fn handle_key_event(
        &mut self,
        key: KeyEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.state {
            AppState::InputToken => match key.code {
                KeyCode::Enter => {
                    if !self.input_text.is_empty() {
                        match self.token_entry.set_secret(&self.input_text).await {
                            Ok(_) => {
                                self.state = AppState::LoggedIn;
                            }
                            Err(e) => {
                                let detailed_err = format!(
                                    "{}\n\nUnderlying Details:\n{:?}\n\n💡 Hint: If you are on a minimal Linux install, you likely need to install a Secret Service provider (e.g., `sudo pacman -S gnome-keyring`).",
                                    e, e
                                );
                                self.state = AppState::Error(detailed_err);
                            }
                        }
                    }
                }
                KeyCode::Char(c) => {
                    self.input_text.push(c);
                }
                KeyCode::Backspace => {
                    self.input_text.pop();
                }
                KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
            AppState::LoggedIn => {
                if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
            AppState::Error(_) => {
                if matches!(key.code, KeyCode::Char(_) | KeyCode::Esc | KeyCode::Enter) {
                    self.state = AppState::InputToken;
                }
            }
        }
        Ok(())
    }
}
