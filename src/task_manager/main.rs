// #![deny(warnings)]

extern crate orbclient;
extern crate orbtk;

use std::fs;
use std::io::Read;
use orbclient::{Color, Renderer, WindowFlag};
use orbtk::{Action, Menu, Point, Rect, List, Entry, Label, Separator, TextBox, Window, Button};
use orbtk::traits::{Click, Place, Resize, Text};

use std::sync::Arc;

use std::{cmp};

#[cfg(target_os = "redox")]
static PROCESS_INFO: &'static str = "sys:/context";

#[cfg(not(target_os = "redox"))]
static PROCESS_INFO: &'static str = "tst/task_manager/ps_output.txt";

const ITEM_SIZE: i32 = 32;

struct ProcessInfo {
    pid : String,
    ppid : String,
    ruid : String,
    rgid : String,
    rns : String,
    euid : String,
    egid : String,
    ens : String,
    stat : String,
    cpu : String,
    mem : String,
    name : String
}

#[derive(Clone, Copy)]
struct Column {
    name: &'static str,
    x: i32,
    width: i32
}

struct TaskManager {
    processes : Vec<ProcessInfo>,
    columns : [Column; 1],
    column_labels: Vec<Arc<Label>>,
    window: Window,
    window_width: u32,
    window_height: u32,
    list_widget_index: Option<usize>,
}

impl TaskManager {
    pub fn new() -> Self {
        let title = "Task Manager";

        let (display_width, display_height) = orbclient::get_display_size().expect("viewer: failed to get display size");
        let (width, height) = (cmp::min(1024, display_width * 4/5), cmp::min(768, display_height * 4/5));

        let mut window = Window::new_flags(Rect::new(-1, -1, width, height), &title, &[WindowFlag::Resizable]);

        let refresh_button = Button::new();
        refresh_button.position(32, 0)
            .size(64, 16)
            .text("Refresh")
            .text_offset(4, 0);

        /* {
            let text_box = text_box.clone();
            refresh_button.on_click(move |_, _| {
                let mut f = fs::File::open(PROCESS_INFO).unwrap();
                let mut s = String::new();
                let _ = f.read_to_string(&mut s);
                text_box.text.set(s);
            });
        } */

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

        TaskManager {
            processes: Vec::new(),
            columns : [
                Column {
                    name: "PID",
                    x: 0,
                    width: 0,
                }
            ],
            column_labels: Vec::new(),
            window: window,
            window_width: width as u32,
            window_height: height as u32,
            list_widget_index : None
        }
    }

    fn resized_columns(&self) -> [Column; 1] {
        let mut columns = self.columns.clone();
        columns[0].width = cmp::max(
            columns[0].width,
            self.window_width as i32
                - columns[0].x
        );
        columns
    }

    fn redraw(&mut self) {
        self.update_headers();
        self.update_list();
        self.window.needs_redraw();
    }

    fn update_list(&mut self) {
        let columns = self.resized_columns();
        let w = (columns[columns.len() - 1].x + columns[columns.len() - 1].width) as u32;

        let list = List::new();
        list.position(0, 32).size(w, self.window_height - 32); // FIXME: 32 is magic.

        for process in self.processes.iter() {
            let entry = Entry::new(ITEM_SIZE as u32);
            let mut label = Label::new();
            label.position(columns[0].x, 0).size(w, ITEM_SIZE as u32).text(process.pid.clone());
            label.text_offset.set(Point::new(0, 8));
            label.bg.set(Color::rgba(255, 255, 255, 0));
            entry.add(&label);
            list.push(&entry);
        }

        if let Some(i) = self.list_widget_index {
            let mut widgets = self.window.widgets.borrow_mut();
            widgets.remove(i);
            widgets.insert(i, list);
        } else {
            self.list_widget_index = Some(self.window.add(&list));
        }
    }

    fn update_headers(&mut self) {
        let mut columns = self.resized_columns();
        for (i, column) in columns.iter().enumerate() {

            if let None = self.column_labels.get(i) {
                // header text
                let mut label = Label::new();
                self.window.add(&label);
                label.bg.set(Color::rgba(255, 255, 255, 0));
                label.text_offset.set(Point::new(0, 16));

                self.column_labels.push(label);
            }

            if let Some(label) = self.column_labels.get(i) {
                label.position(column.x, 0).size(column.width as u32, 32).text(column.name.clone());
            }
        }
    }

    fn update_processes(&mut self) {
        let mut f = fs::File::open(PROCESS_INFO).unwrap();
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        self.processes.clear();

        let mut lines : Vec<String> = s.split("\n").map(|s| String::from(s)).collect();
        for (i, line) in lines.into_iter().enumerate() {
            if i != 0 && line.len() > 0 {
                self.processes.push(get_process_info(line));
            }
        }
    }

    pub fn main(&mut self) {
        for column in self.columns.iter_mut() {
            column.width = (column.name.len() * 8) as i32 + 16;
        }

        while self.window.running.get() {
            self.window.step();
            self.update_processes();
            self.redraw();
            self.window.draw_if_needed();
        }
    }
}

fn get_process_info(line : String) -> ProcessInfo {
    let mut split_up : Vec<String> = line.split_whitespace().map(|t| String::from(t)).collect();
    ProcessInfo {
        pid : split_up[0].clone(),
        ppid : split_up[1].clone(),
        ruid : split_up[2].clone(),
        rgid : split_up[3].clone(),
        rns : split_up[4].clone(),
        euid : split_up[5].clone(),
        egid : split_up[6].clone(),
        ens : split_up[7].clone(),
        stat : split_up[8].clone(),
        cpu : split_up[9].clone(),
        mem : format!("{} {}", split_up[10].clone(), split_up[11].clone()),
        name : split_up[11].clone(),
    }
}

fn main(){
    let mut task_manager = TaskManager::new();
    task_manager.main();
}
