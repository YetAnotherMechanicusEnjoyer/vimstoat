/// Action based on user input
#[derive(Clone, Copy)]
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
