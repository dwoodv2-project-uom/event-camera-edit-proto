use iced::{
    Alignment, Length,
    widget::{Column, button, column, text},
};

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Increment,
    Decrement,
}

pub struct Editor {
    value: i32,
}

impl Default for Editor {
    fn default() -> Self {
        Self { value: 0 }
    }
}

impl Editor {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Increment => {
                self.value += 1;
            }
            Message::Decrement => {
                self.value -= 1;
            }
        }
    }

    pub fn view(&self) -> Column<'_, Message> {
        column![
            button("Increment").on_press(Message::Increment),
            text(self.value).size(50),
            button("Decrement").on_press(Message::Decrement)
        ]
        .padding(20)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill)
    }
}
