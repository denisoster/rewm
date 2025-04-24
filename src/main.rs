use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::errors::ConnectionError;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;

#[derive(Debug, Clone, Copy)]
enum LayoutMode {
    Horizontal,
    Vertical,
}

struct WindowManager<C: Connection> {
    conn: C,
    screen_num: usize,
    layout: LayoutMode,
    windows: Vec<u32>,
}

impl<C: Connection> WindowManager<C> {
    fn new(conn: C, screen_num: usize) -> Self {
        WindowManager {
            conn,
            screen_num,
            layout: LayoutMode::Horizontal,
            windows: Vec::new(),
        }
    }

    fn arrange_windows(&mut self) -> Result<(), ConnectionError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let width = screen.width_in_pixels as u32;
        let height = screen.height_in_pixels as u32;

        if self.windows.len() >= 2 {
            match self.layout {
                LayoutMode::Horizontal => {
                    self.conn.configure_window(
                        self.windows[0],
                        &ConfigureWindowAux::new()
                            .x(0)
                            .y(0)
                            .width(width / 2)
                            .height(height),
                    )?;
                    self.conn.configure_window(
                        self.windows[1],
                        &ConfigureWindowAux::new()
                            .x((width / 2) as i32)
                            .y(0)
                            .width(width / 2)
                            .height(height),
                    )?;
                }
                LayoutMode::Vertical => {
                    self.conn.configure_window(
                        self.windows[0],
                        &ConfigureWindowAux::new()
                            .x(0)
                            .y(0)
                            .width(width)
                            .height(height / 2),
                    )?;
                    self.conn.configure_window(
                        self.windows[1],
                        &ConfigureWindowAux::new()
                            .x(0)
                            .y((height / 2) as i32)
                            .width(width)
                            .height(height / 2),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn toggle_layout(&mut self) -> Result<(), ConnectionError> {
        self.layout = match self.layout {
            LayoutMode::Horizontal => LayoutMode::Vertical,
            LayoutMode::Vertical => LayoutMode::Horizontal,
        };
        self.arrange_windows()?;
        Ok(())
    }

    fn setup_key_bindings(&self) -> Result<(), ConnectionError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let root = screen.root;

        self.conn.grab_key(
            true,
            root,
            ModMask::M4,
            65,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?;

        self.conn.grab_key(
            true,
            root,
            ModMask::CONTROL | ModMask::M4,
            24,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?;

        Ok(())
    }

    fn run(&mut self) -> Result<(), ConnectionError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let root = screen.root;

        self.conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new()
                .event_mask(EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY),
        )?;

        self.setup_key_bindings()?;

        println!("Оконный менеджер запущен. Режим: {:?}", self.layout);

        loop {
            self.conn.flush()?;
            let event = self.conn.wait_for_event()?;

            match event {
                Event::MapRequest(event) => {
                    println!("Получен запрос на отображение окна: {}", event.window);
                    self.conn.map_window(event.window)?;
                    self.windows.push(event.window);
                    if self.windows.len() > 2 {
                        self.windows.remove(0);
                    }
                    self.arrange_windows()?;
                }
                Event::KeyPress(event) => {
                    let keycode = event.detail;
                    let state = event.state;

                    if state == (ModMask::M4.bits() as u16).into() && keycode == 65 {
                        self.toggle_layout()?;
                        println!("Переключен режим на: {:?}", self.layout);
                    }
                    else if state == ((ModMask::CONTROL | ModMask::M4).bits() as u16).into() && keycode == 24 {
                        println!("Выход из оконного менеджера");
                        break;
                    }
                }
                Event::DestroyNotify(event) => {
                    if let Some(pos) = self.windows.iter().position(|&x| x == event.window) {
                        self.windows.remove(pos);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let apps = vec!["firefox", "alacritty"];
    for app in apps {
        Command::new(app)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        println!("Запущено приложение: {}", app);
        thread::sleep(Duration::from_secs(1));
    }

    let (conn, screen_num) = x11rb::connect(None)?;
    let mut wm = WindowManager::new(conn, screen_num);
    wm.run()?;

    Ok(())
}