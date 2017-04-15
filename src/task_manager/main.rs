// #![deny(warnings)]

extern crate orbclient;
extern crate orbtk;

use std::fs;
use std::io::Read;
use orbclient::WindowFlag;
use orbtk::{Action, Menu, Point, Rect, Separator, TextBox, Window, Button};
use orbtk::traits::{Click, Place, Resize, Text};

use std::{cmp};

#[cfg(target_os = "redox")]
static PROCESS_INFO: &'static str = "sys:/context";

#[cfg(not(target_os = "redox"))]
static PROCESS_INFO: &'static str = "tst/task_manager/ps_output.txt";

fn main(){
    let title = format!("Task Manager");

    let (display_width, display_height) = orbclient::get_display_size().expect("viewer: failed to get display size");
    let (width, height) = (cmp::min(1024, display_width * 4/5), cmp::min(768, display_height * 4/5));

    let mut window = Window::new_flags(Rect::new(-1, -1, width, height), &title, &[WindowFlag::Resizable]);

    let text_box = TextBox::new();
    text_box.position(0, 16)
        .size(width, height - 16);
    window.add(&text_box);

    let refresh_button = Button::new();
    refresh_button.position(32, 0)
        .size(64, 16)
        .text("Refresh")
        .text_offset(4, 0);

    {
        let text_box_clone = text_box.clone();
        refresh_button.on_click(move |_, _| {
            let mut f = fs::File::open(PROCESS_INFO).unwrap();
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            text_box_clone.text.set(s);
        });
    }

    window.add(&refresh_button);


    let menu = Menu::new("File");
    menu.position(0, 0).size(32, 16);

    menu.add(&Separator::new());

    let close_action = Action::new("Close");
    let window_close = &mut window as *mut Window;
    close_action.on_click(move |_action: &Action, _point: Point| {
        println!("Close");
        unsafe { (&mut *window_close).close(); }
    });
    menu.add(&close_action);

    window.add(&menu);

    window.on_resize(move |_, width, height| {
        text_box.size(width, height - 16);
    });

    window.exec();
}
