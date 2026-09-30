# Iced

Iced uses the ELM architecture, which splits each UI compoment into 3 groups

![alt text](https://book.iced.rs/resources/the-gui-trinity.svg)

## Windows

Windows encapulate state, widgets, and interactions, therefore, they should be defined as follows:

```rust
// State
struct AWindow {
    some_data: u64,
    ...
}

// Message enum, will be large, but is manageble if you group them up enough.
//
enum Message {
    AWidgetMessage(some_widget::Action), // Abstract the action by defining another enum in the widget's file.
    ...
}

// Window code
impl AWindow {
    fn new(...) -> Self {
        // Bootstrap initial state
    }

    fn view(&self) -> iced::Element<'_, Message> {
        // Widgets -- controls the view
    }

    fn update(&self, message: Message) -> Task<Message> {
        // Interactions -- controls what messages are sent.
    }

    fn subscription(&self) -> Subscription<Message> {
        // Controls what messages when an asyncronous message is received.
    }
}
}
```

## Widgets

Custom widgets are created using [`iced::Widget`](https://docs.rs/iced/0.14.0/iced/advanced/trait.Widget.html) and the trait can be found here:

```rust
pub trait Widget<Message, Theme, Renderer>
where
    Renderer: Renderer,
{
    // Required methods
    fn size(&self) -> Size<Length>;

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &Limits,
    ) -> Node;

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    );
```

## Structure

The code structure is as follows

```
├── Cargo.toml
└── src
    ├── main.rs
    ├── widgets
    │   └── a_widget.rs
    └── windows
    │   └── editor.rs
    └── domain // app specific logic indepdent from iced.
        └── editor_logic.rs
```
