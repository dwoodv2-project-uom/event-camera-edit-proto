mod windows;

use windows::editor::Editor;

fn main() -> iced::Result {
    iced::application(Editor::default, Editor::update, Editor::view)
        .title("Event Camera Editor")
        .run()
}
