use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::session::Session;

const DEFAULT_SCROLLBACK: usize = 1_000;

pub struct Terminal {
    writer: Box<dyn Write + Send>,
    parser: Arc<Mutex<vt100::Parser>>,
    alive: Arc<AtomicBool>,
    scrollback_size: Arc<AtomicUsize>,
    rows: u16,
    cols: u16,
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl Terminal {
    pub fn spawn(session: &Session, rows: u16, cols: u16) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(session.shell_bin());
        cmd.cwd(session.working_dir_path());
        for (k, v) in &session.env_vars {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");

        setup_shell_init(session, &mut cmd);

        let child = pair.slave.spawn_command(cmd)?;
        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, DEFAULT_SCROLLBACK)));
        let alive = Arc::new(AtomicBool::new(true));
        let scrollback_size = Arc::new(AtomicUsize::new(0));

        let parser_clone = Arc::clone(&parser);
        let alive_clone = Arc::clone(&alive);
        let scrollback_clone = Arc::clone(&scrollback_size);

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        alive_clone.store(false, Ordering::Relaxed);
                        break;
                    }
                    Ok(n) => {
                        match parser_clone.lock() {
                            Ok(mut p) => {
                                p.process(&buf[..n]);
                                p.set_scrollback(usize::MAX);
                                scrollback_clone.store(p.screen().scrollback(), Ordering::Relaxed);
                                p.set_scrollback(0);
                            }
                            Err(_) => {
                                alive_clone.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            writer,
            parser,
            alive,
            scrollback_size,
            rows,
            cols,
            master: pair.master,
            child: Some(child),
        })
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback_size.load(Ordering::Relaxed)
    }

    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(rows, cols);
        }
        self.rows = rows;
        self.cols = cols;
        Ok(())
    }

    pub fn with_screen<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&vt100::Screen) -> R,
    {
        self.parser.lock().ok().map(|p| f(p.screen()))
    }

    pub fn with_scrollback<F, R>(&self, offset: u16, f: F) -> Option<R>
    where
        F: FnOnce(&vt100::Screen) -> R,
    {
        self.parser.lock().ok().map(|mut p| {
            p.set_scrollback(offset as usize);
            let result = f(p.screen());
            p.set_scrollback(0);
            result
        })
    }

}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}

fn setup_shell_init(session: &Session, cmd: &mut CommandBuilder) {
    let init_file = match session.build_init_file() {
        Some(p) => p,
        None => return,
    };
    let shell = session.shell_bin();
    let rc_dir = std::env::temp_dir().join(format!("ntx_rc_{}", session.name));
    let _ = fs::create_dir_all(&rc_dir);

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

    if shell.contains("zsh") {
        let real_zshenv = home.join(".zshenv");
        let _ = fs::write(
            rc_dir.join(".zshenv"),
            format!(
                "[ -f \"{p}\" ] && source \"{p}\"\n",
                p = real_zshenv.display()
            ),
        );
        let real_zshrc = home.join(".zshrc");
        let _ = fs::write(
            rc_dir.join(".zshrc"),
            format!(
                "ZDOTDIR=\"$HOME\"\n[ -f \"{rc}\" ] && source \"{rc}\"\nsource \"{init}\"\n",
                rc = real_zshrc.display(),
                init = init_file.display(),
            ),
        );
        cmd.env("ZDOTDIR", rc_dir.to_string_lossy().as_ref());
    } else if shell.contains("bash") {
        let real_bashrc = home.join(".bashrc");
        let wrapper = rc_dir.join(".bashrc");
        let _ = fs::write(
            &wrapper,
            format!(
                "[ -f \"{rc}\" ] && source \"{rc}\"\nsource \"{init}\"\n",
                rc = real_bashrc.display(),
                init = init_file.display(),
            ),
        );
        cmd.args(["--rcfile", &wrapper.to_string_lossy()]);
    }
}
