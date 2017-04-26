// #![deny(warnings)]

use std::str::FromStr;

extern crate orbclient;
extern crate orbtk;

use std::fs;
use std::io::Read;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::time::Duration;
use std::thread;
use orbclient::{Color, Renderer, WindowFlag};
use orbtk::{Action, Menu, Point, Rect, List, Entry, Label, Separator, TextBox, Window, Button};
use orbtk::traits::{Click, Place, Resize, Text};

use std::sync::Arc;

use std::{cmp};

#[cfg(target_os = "redox")]
extern crate syscall;

#[cfg(target_os = "redox")]
static PROCESS_INFO: &'static str = "sys:/context";

#[cfg(not(target_os = "redox"))]
static PROCESS_INFO: &'static str = "tst/task_manager/ps_output.txt";

const ITEM_SIZE: i32 = 16;

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

enum TaskManagerCommand {
    Resize(u32, u32),
    Update(Vec<ProcessInfo>)
}

#[derive(Clone, Copy, Debug)]
struct Column {
    name: &'static str,
    x: i32,
    width: i32
}

struct TaskManager {
    processes : Vec<ProcessInfo>,
    columns : [Column; 2],
    column_labels: Vec<Arc<Label>>,
    window: Window,
    window_width: u32,
    window_height: u32,
    list_widget_index: Option<usize>,
    tx: Sender<TaskManagerCommand>,
    rx: Receiver<TaskManagerCommand>,
}

impl TaskManager {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let title = "Task Manager";

        let (display_width, display_height) = orbclient::get_display_size().expect("viewer: failed to get display size");
        let (width, height) = (cmp::min(1024, display_width * 4/5), cmp::min(768, display_height * 4/5));

        let mut window = Window::new_flags(Rect::new(-1, -1, width, height), &title, &[WindowFlag::Resizable]);

        let tx_resize = tx.clone();
        window.on_resize(move |_window, width, height| {
            tx_resize.send(TaskManagerCommand::Resize(width, height)).unwrap();
        });

        TaskManager {
            processes: Vec::new(),
            columns : [
                Column {
                    name: "Name",
                    x : 0,
                    width: 0
                },
                Column {
                    name: "PID",
                    x: 0,
                    width: 48,
                }
            ],
            column_labels: Vec::new(),
            window: window,
            window_width: width as u32,
            window_height: height as u32,
            list_widget_index : None,
            tx : tx,
            rx : rx
        }
    }

    fn resized_columns(&self) -> [Column; 2] {
        let mut columns = self.columns.clone();
        columns[0].width = cmp::max(
            columns[0].width,
            self.window_width as i32
                - columns[1].width
        );
        columns[1].x = columns[0].x + columns[0].width;
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
            label.position(columns[0].x, 0).size(w, ITEM_SIZE as u32).text(process.name.clone());
            label.bg.set(Color::rgba(255, 255, 255, 0));
            entry.add(&label);

            label = Label::new();
            label.position(columns[1].x, 0).size(w, ITEM_SIZE as u32).text(process.pid.clone());
            label.bg.set(Color::rgba(255, 255, 255, 0));
            entry.add(&label);

            let pid = process.pid.clone();
            entry.on_click(move |_, _| {
                kill_pid(&pid);
            });

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

    pub fn main(&mut self) {
        let tx_refresh = self.tx.clone();
        thread::spawn(move || {
            loop {
                tx_refresh.send(TaskManagerCommand::Update(get_processes())).unwrap();
                thread::sleep(Duration::new(2, 0));
            }
        });

        self.processes = get_processes();
        self.redraw();

        while self.window.running.get() {
            self.window.step();

            while let Ok(event) = self.rx.try_recv() {
                match event {
                    TaskManagerCommand::Resize(width, height) => {
                        self.window_width = width;
                        self.window_height = height;
                    },
                    TaskManagerCommand::Update(processes) => {
                        println!("update");
                        self.processes = processes;
                    }
                }
                self.redraw();
            }

            self.window.draw_if_needed();
        }
    }
}

fn get_processes() -> Vec<ProcessInfo> {
    let mut f = fs::File::open(PROCESS_INFO).unwrap();
    let mut s = String::new();
    let _ = f.read_to_string(&mut s);
    s.split("\n")
        .filter(|s| s.len() > 0)
        .map(|s| String::from(s))
        .skip(1)
        .map(get_process_info)
        .collect()
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
        name : if split_up.len() > 12 { split_up[12].clone() } else {String::from("N/A")},
    }
}

#[cfg(target_os="redox")]
fn kill_pid(pid: &String) {
    println!("Killed pid: {}", pid);
    syscall::kill(usize::from_str(pid.as_str()).unwrap(), 0x9).unwrap();
}

#[cfg(not(target_os="redox"))]
fn kill_pid(pid: &String) {
    println!("Not implemented on redox. Killed pid: {}", pid);
}

fn main(){
    let mut task_manager = TaskManager::new();
    task_manager.main();
}
