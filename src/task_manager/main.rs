// #![deny(warnings)]
#![feature(const_fn)]

use std::str::FromStr;

extern crate orbclient;
extern crate orbtk;
extern crate orbimage;

use std::fs;
use std::io::Read;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::time::Duration;
use std::thread;
use orbclient::{Color, Renderer, WindowFlag};
use orbtk::{Action, Event, Menu, Point, Rect, List, Entry, Label, Separator, TextBox, Window, Button};
use std::path::Path;
use orbtk::widgets::Widget;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use orbtk::traits::{Click, Place, Resize, Text};
use std::sync::Arc;
use std::{cmp};
use orbtk::theme::{LABEL_BACKGROUND, LABEL_BORDER, LABEL_FOREGROUND};

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
    columns : [Column; 3],
    column_labels: Vec<Arc<Label>>,
    window: Window,
    window_width: u32,
    window_height: u32,
    tx: Sender<TaskManagerCommand>,
    rx: Receiver<TaskManagerCommand>,
    list_widget_index : Option<usize>
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
                },
                Column {
                    name: "Program Usage",
                    x: 0,
                    width: 150
                }
            ],
            column_labels: Vec::new(),
            window: window,
            window_width: width as u32,
            window_height: height as u32,
            tx : tx,
            rx : rx,
            list_widget_index : None
        }
    }

    fn resized_columns(&self) -> [Column; 3] {
        let mut columns = self.columns.clone();
        columns[0].width = cmp::max(
            columns[0].width,
            self.window_width as i32
                - columns[1].width
                - columns[2].width
        );
        columns[1].x = columns[0].x + columns[0].width;
        columns[2].x = columns[1].x + columns[1].width;
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

            label = Label::new();
            label.position(columns[2].x, 0).size(w, ITEM_SIZE as u32).text(process.mem.clone());
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
    println!("Only implemented on redox. Killed pid: {}", pid);
}

#[cfg(target_os="redox")]
fn main(){
    thread::spawn(move || {
        let graph_viewer = GraphViewer::new();
        graph_viewer.main();
    });

    let mut task_manager = TaskManager::new();
    task_manager.main();
}

#[cfg(not(target_os="redox"))]
fn main() {
    let mut task_manager = TaskManager::new();
    task_manager.main();
}

enum GraphCommand {
    Update(bool),
}
struct GraphViewer {
    window: Window,
    window_width: u32,
    window_height: u32,
    graph : Arc<LineGraph>,
    points : Vec<i32>,
    pointsKept : usize,
    tx: Sender<GraphCommand>,
    rx: Receiver<GraphCommand>,
    max: i32,
}

impl GraphViewer {

    pub fn new() -> Self {
        let title = "Task Manager - Graph Viewer";
        let (width, height) = (300, 300);
        let (tx, rx) = channel();

        let (graph1_x, graph1_y) = (0, 0);
        let (graph1_w, graph1_h) = (width, height);

        let window = Window::new(Rect::new(10, 30, width, height), &title);
        let graph = LineGraph::from_color(graph1_x, graph1_y, width, height, LABEL_BACKGROUND);
        // graph.plot(vec![0, 10, 20, 30, 40, 50], 500);
        let xlabel = Label::new();
        xlabel.text("Time")
              .position(graph1_x + graph1_w as i32 - 50, graph1_y + graph1_h as i32 - 20)
              .size(40, 16);
        let title = Label::new();
        title.text("Memory Usage")
            .position(graph1_x + (graph1_w / 3) as i32, graph1_y + 10)
            .size (100, 16);
        window.add(&graph);
        window.add(&xlabel);
        window.add(&title);

        let points: Vec<i32> = Vec::new();
        GraphViewer {
            window : window,
            graph : graph,
            window_width : width,
            window_height : height,
            points : points,
            // Change pointsKept to change the amount of points in the graph
            pointsKept : 10,
            tx : tx,
            rx : rx,
            max: get_memory_usage().2,
        }
    }

    pub fn main(mut self) {
        let tx_refresh = self.tx.clone();
        thread::spawn(move || {
            loop {
                tx_refresh.send(GraphCommand::Update(true)).unwrap();
                thread::sleep(Duration::new(2, 0));
            }
        });

        while self.window.running.get() {
            self.window.step();

            while let Ok(event) = self.rx.try_recv() {
                match event {
                    // GraphCommand::Resize(width, height) => {
                    //     self.window_width = width;
                    //     self.window_height = height;
                    // },
                    GraphCommand::Update(true) => {
                        self.update();
                    },
                    _ => (),
                }
                self.window.needs_redraw();
            }
            self.window.draw_if_needed();
        }
    }
    fn update(&mut self){
        self.updatePoints();
        self.graph.plot(self.points.clone(), self.max.clone());
    }

    fn updatePoints(&mut self){
        let (free,used,size) = get_memory_usage();
        if self.points.len() > self.pointsKept {
            let _ = self.points.remove(0);
        }
        self.points.push(used);
    }


}

/// Orbital Widget for representing a graph.
struct LineGraph {
    rect: Cell<Rect>,
    image: RefCell<orbimage::Image>,
    background_color : Color
}

const NOTCH_HEIGHT : i32 = 20;
const LINE_COLOR : Color = Color::rgb(0, 0, 0);
const BORDER : i32 = 30;

impl LineGraph {

    /// Creates a new graph with specified width and height in pixels.
    /// Taken from here: https://github.com/redox-os/orbtk/blob/master/src/widgets/image.rs
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Arc<Self> {
        Self::from_image(x, y, orbimage::Image::new(width, height), Color::rgb(255, 255, 255))
    }

    /// Creates a new graph with the specified background color.
    /// Taken from here: https://github.com/redox-os/orbtk/blob/master/src/widgets/image.rs
    pub fn from_color(x: i32, y: i32, width: u32, height: u32, color: Color) -> Arc<Self> {
        Self::from_image(x, y, orbimage::Image::from_color(width, height, color), color)
    }

    /// Creates a new graph with provided background image.
    /// Taken from here: https://github.com/redox-os/orbtk/blob/master/src/widgets/image.rs
    pub fn from_image(x: i32, y: i32, image: orbimage::Image, color : Color) -> Arc<Self> {
        Arc::new(LineGraph {
            rect: Cell::new(Rect::new(x, y, image.width(), image.height())),
            image: RefCell::new(image),
            background_color : color
        })
    }

    /// Draws a black line segment from "from" to "to."
    fn draw_path(&self, from : &Point, to : &Point) {
        let mut image = self.image.borrow_mut();
        image.line(from.x, from.y, to.x, to.y, LINE_COLOR);
    }

    /// Draws the little notches that line up to each data point.
    fn draw_notch(&self, x : i32) {
        let (_, y, _, height) = self.rect();
        let bot = Point { x : x, y : height + NOTCH_HEIGHT / 2};
        let top = Point { x : x, y : height - NOTCH_HEIGHT / 2};
        self.draw_path(&bot, &top);
    }

    /// Draws the box around the graph.
    fn draw_box(&self) {
        let (x, y, width, height) = self.rect();
        let top_left = Point { x : x, y : y};
        let top_right = Point {x : width,  y : y};
        let bot_left = Point {x : x, y : height};
        let bot_right = Point {x : width, y : height};
        self.draw_path(&top_left, &top_right);
        self.draw_path(&top_right, &bot_right);
        self.draw_path(&bot_right, &bot_left);
        self.draw_path(&bot_left, &top_left);
    }

    /// Plots the y-values on the graph.
    pub fn plot(&self, ys : Vec<i32>, ymax : i32) {
        self.reset();

        let (_, _, width, height) = self.rect();

        self.draw_box();

        let points = self.translate_ys(ys, ymax);
        for i in 0..(points.len() - 1) {
            self.draw_path(&points[i], &points[i + 1]);
        }

        for point in points {
            self.draw_notch(point.x);
        }

    }

    /// Returns the rectangle representing the graph.
    fn rect(&self) -> (i32, i32, i32, i32) {
        let rect = self.rect.get();
        (BORDER, BORDER, rect.width as i32 - BORDER, rect.height as i32 - BORDER)
    }

    /// Clears everything on the graph.
    fn reset(&self) {
        let (x, y, width, height) = self.rect();
        let mut image = self.image.borrow_mut();
        *image = orbimage::Image::from_color(width as u32 + BORDER as u32, height as u32 + BORDER as u32, self.background_color);
    }

    /// Converts the y-values to points
    fn translate_ys(&self, ys : Vec<i32>, ymax : i32) -> Vec<Point> {
        let (window_x, window_y, width, height) = self.rect();

        let mut points : Vec<Point> = Vec::new();
        let length = ys.len() as i32;

        for (i, y) in ys.into_iter().enumerate() {
            points.push( Point {
                x : window_x + (width / length) * (i as i32),
                y : ((height as f64) - (((y as f64) / (ymax as f64)) * ((height - window_y) as f64))) as i32,
            });
        }

        points
    }

}

impl Place for LineGraph {}

impl Widget for LineGraph {

    /// Returns the underlying rect.
    /// Taken from here: https://github.com/redox-os/orbtk/blob/master/src/widgets/image.rs
    fn rect(&self) -> &Cell<Rect> {
        &self.rect
    }

    /// Draws the underlying image.
    /// Taken from here: https://github.com/redox-os/orbtk/blob/master/src/widgets/image.rs
    fn draw(&self, renderer: &mut Renderer, _focused: bool) {
        let rect = self.rect.get();
        let image = self.image.borrow();
        renderer.image(rect.x, rect.y, image.width(), image.height(), image.data());
    }

    fn event(&self, _: Event, _: bool, _: &mut bool) -> bool {
        false
    }
}

/// Gets the current free, used, and total memory.
///
/// Code taken from here:
///  https://github.com/redox-os/coreutils/blob/master/src/bin/free.rs
#[cfg(target_os = "redox")]
fn get_memory_usage() -> (i32, i32, i32) {
    use syscall::data::StatVfs;

    let mut stat = StatVfs::default();
    {
        let fd = syscall::open("memory:", syscall::O_STAT).unwrap();
        syscall::fstatvfs(fd, &mut stat).unwrap();
        let _ = syscall::close(fd);
    }

    let size = stat.f_blocks * stat.f_bsize as u64;
    let used = (stat.f_blocks - stat.f_bfree) * stat.f_bsize as u64;
    let free = stat.f_bavail * stat.f_bsize as u64;
    (free as i32, used as i32, size as i32)
}

#[cfg(not(target_os = "redox"))]
fn get_memory_usage() -> (i32, i32, i32) {
    (5, 10, 15)
}
