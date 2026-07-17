/// Action based on user input
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    #[allow(unused)]
    AppendCharacter(char),
    RemoveCharacter,
    Enter,
    CursorLeft,
    CursorRight,
    CursorUp,
    CursorDown,
    Escape,
    Quit,
}
